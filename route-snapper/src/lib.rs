#[macro_use]
extern crate log;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};

use geojson::Feature;
use geom::{Distance, HashablePt2D, LonLat, PolyLine, Pt2D};
use petgraph::graphmap::DiGraphMap;
use rstar::primitives::GeomWithData;
use rstar::RTree;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use route_snapper_graph::{EdgeID, NodeID, RouteSnapperMap};

type Graph = DiGraphMap<NodeID, DirectedEdge>;

#[wasm_bindgen]
pub struct JsRouteSnapper {
    router: Router,
    snap_to_nodes: RTree<GeomWithData<[f64; 2], NodeID>>,
    route: Route,
    mode: Mode,
    // Is the shift key not held?
    snap_mode: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Config {
    /// With multiple intermediate waypoints, try to avoid routing on edges already used in a
    /// previous portion of the path. This is best-effort.
    avoid_doubling_back: bool,
    /// If false, the user can only drag waypoints after specifying the start and end of the route.
    /// If true, they can keep clicking to extend the end of the route.
    extend_route: bool,

    /// Generate a route that starts and ends in the same place. Has to be set using `setAreaMode`,
    /// but `getConfig` will show this.
    #[serde(skip_deserializing)]
    area_mode: bool,
}

struct Router {
    // TODO Blurring the line where state lives, all of this needs a re-work
    map: RouteSnapperMap,
    graph: Graph,
    config: Config,
}

// TODO It's impossible for a waypoint to be an Edge, but the code might be simpler if this and
// PathEntry are merged
#[derive(Clone, Copy, PartialEq, Debug)]
enum Waypoint {
    Snapped(NodeID),
    Free(Pt2D),
}

impl Waypoint {
    fn to_path_entry(self) -> PathEntry {
        match self {
            Waypoint::Snapped(x) => PathEntry::SnappedPoint(x),
            Waypoint::Free(x) => PathEntry::FreePoint(x),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathEntry {
    SnappedPoint(NodeID),
    FreePoint(Pt2D),
    Edge(DirectedEdge),
    // Note we don't need to represent a straight line between snapped or free points here. As we
    // build up the line-string, they'll happen anyway.
}

impl PathEntry {
    fn to_waypt(self) -> Option<Waypoint> {
        match self {
            PathEntry::SnappedPoint(x) => Some(Waypoint::Snapped(x)),
            PathEntry::FreePoint(x) => Some(Waypoint::Free(x)),
            PathEntry::Edge(_) => None,
        }
    }
}

#[derive(Clone)]
struct Route {
    // Something explicitly manipulated by the user
    waypoints: Vec<Waypoint>,

    // The full route, expanded. This can be calculated purely from waypoints.
    full_path: Vec<PathEntry>,
}

type Direction = bool;
const FORWARDS: Direction = true;
const BACKWARDS: Direction = false;

#[derive(Clone, Copy, PartialEq, Debug)]
struct DirectedEdge(EdgeID, Direction);

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Neutral,
    Hovering(Waypoint),
    // idx is into full_path
    Dragging { idx: usize, at: Waypoint },
    Freehand(Pt2D),
}

#[wasm_bindgen]
impl JsRouteSnapper {
    #[wasm_bindgen(constructor)]
    pub fn new(map_bytes: &[u8]) -> Result<JsRouteSnapper, JsValue> {
        if !cfg!(test) {
            console_log::init_with_level(log::Level::Info).unwrap();
            // Panics shouldn't happen, but if they do, console.log them.
            console_error_panic_hook::set_once();
        }

        info!("Got {} bytes, deserializing", map_bytes.len());

        let mut map: RouteSnapperMap = bincode::deserialize(map_bytes).map_err(err_to_js)?;
        for edge in &mut map.edges {
            edge.length = edge.geometry.length();
        }

        info!("Finalizing JsRouteSnapper");

        let mut graph: Graph = DiGraphMap::new();
        for (idx, e) in map.edges.iter().enumerate() {
            let id = EdgeID(idx as u32);
            graph.add_edge(e.node1, e.node2, DirectedEdge(id, FORWARDS));
            graph.add_edge(e.node2, e.node1, DirectedEdge(id, BACKWARDS));
        }

        let mut nodes = Vec::new();
        for (idx, pt) in map.nodes.iter().enumerate() {
            nodes.push(GeomWithData::new([pt.x(), pt.y()], NodeID(idx as u32)));
        }
        let snap_to_nodes = RTree::bulk_load(nodes);

        Ok(Self {
            router: Router {
                map,
                graph,
                config: Config::default(),
            },
            snap_to_nodes,
            route: Route::new(),
            mode: Mode::Neutral,
            snap_mode: true,
        })
    }

    /// Updates configuration and recalculates paths. The caller should redraw.
    #[wasm_bindgen(js_name = setRouteConfig)]
    pub fn set_route_config(&mut self, input: JsValue) {
        match serde_wasm_bindgen::from_value(input) {
            Ok(config) => {
                self.router.config = config;
                assert!(!self.router.config.area_mode);
                self.route.recalculate_full_path(&self.router);
            }
            Err(err) => {
                error!("Bad input to setRouteConfig: {err}");
            }
        }
    }

    /// Enables area mode, where the snapper produces polygons.
    #[wasm_bindgen(js_name = setAreaMode)]
    pub fn set_area_mode(&mut self) {
        self.router.config = Config {
            avoid_doubling_back: true,
            extend_route: true,
            area_mode: true,
        };
        self.route.recalculate_full_path(&self.router);
    }

    /// Gets the current configuration in JSON.
    #[wasm_bindgen(js_name = getConfig)]
    pub fn get_config(&mut self) -> String {
        serde_json::to_string_pretty(&self.router.config).unwrap()
    }

    #[wasm_bindgen(js_name = toFinalFeature)]
    pub fn to_final_feature(&self) -> Option<String> {
        let mut feature = if self.router.config.area_mode {
            if let Some(polygon) = self.into_polygon_area() {
                Feature::from(polygon)
            } else {
                return None;
            }
        } else {
            let pl = self.entire_line_string()?;
            let mut f = Feature::from(pl.to_geojson(Some(&self.router.map.gps_bounds)));
            f.set_property("length_meters", pl.length().inner_meters());
            f
        };

        // Set these on both LineStrings and Polygons
        let mut waypoints = Vec::new();
        for waypt in &self.route.waypoints {
            let gps = self
                .to_pt(*waypt)
                .to_pt2d()
                .to_gps(&self.router.map.gps_bounds);
            waypoints.push(
                serde_json::to_value(&RouteWaypoint {
                    lon: gps.x(),
                    lat: gps.y(),
                    snapped: matches!(waypt, Waypoint::Snapped(_)),
                })
                .unwrap(),
            );
        }
        feature.set_property("waypoints", serde_json::Value::Array(waypoints));

        Some(serde_json::to_string_pretty(&feature).unwrap())
    }

    #[wasm_bindgen(js_name = renderGeojson)]
    pub fn render_geojson(&self) -> String {
        let mut result = Vec::new();

        // Overlapping circles don't work, so override colors in here. Use these styles:
        //
        // 1) "hovered": Something under the cursor
        // 2) "important": A waypoint, or something being dragged
        // 3) "unimportant": A draggable node on the route
        let mut draw_circles = BTreeMap::new();

        // Draw the confirmed route
        if let Some(pl) = self.entire_line_string() {
            let geometry = pl.to_geojson(Some(&self.router.map.gps_bounds));
            result.push((geometry, serde_json::Map::new()));
        }
        for entry in &self.route.full_path {
            // Every free point is a waypoint, so just handle it below
            if let PathEntry::SnappedPoint(node) = entry {
                draw_circles.insert(self.router.map.node(*node).to_hashable(), "unimportant");
            }
        }
        for waypt in &self.route.waypoints {
            draw_circles.insert(self.to_pt(*waypt), "important");
        }

        // Draw the current operation
        if let Mode::Hovering(hover) = self.mode {
            draw_circles.insert(self.to_pt(hover), "hovered");

            if let (Some(last), Waypoint::Snapped(current)) = (self.route.waypoints.last(), hover) {
                // If we're trying to drag a point or it's a closed area, don't show this preview
                if !self.route.is_closed_area()
                    && !self
                        .route
                        .full_path
                        .contains(&PathEntry::SnappedPoint(current))
                {
                    match last {
                        Waypoint::Snapped(last) => {
                            if let Some(entries) =
                                self.router.pathfind(*last, current, &self.route.full_path)
                            {
                                for entry in entries {
                                    // Just preview the lines, not the circles
                                    if let PathEntry::Edge(dir_edge) = entry {
                                        let pl = PolyLine::unchecked_new(edge_geometry(
                                            &self.router.map,
                                            dir_edge,
                                        ));
                                        result.push((
                                            pl.to_geojson(Some(&self.router.map.gps_bounds)),
                                            serde_json::Map::new(),
                                        ));
                                    }
                                }
                            } else {
                                // It'll be a straight line
                                let pl = PolyLine::unchecked_new(vec![
                                    self.router.map.node(*last),
                                    self.router.map.node(current),
                                ]);
                                result.push((
                                    pl.to_geojson(Some(&self.router.map.gps_bounds)),
                                    serde_json::Map::new(),
                                ));
                            }
                        }
                        Waypoint::Free(pt) => {
                            let pl =
                                PolyLine::unchecked_new(vec![*pt, self.router.map.node(current)]);
                            result.push((
                                pl.to_geojson(Some(&self.router.map.gps_bounds)),
                                serde_json::Map::new(),
                            ));
                        }
                    }
                }
            }
        }
        if let Mode::Dragging { at, .. } = self.mode {
            draw_circles.insert(self.to_pt(at), "hovered");
        }
        if let Mode::Freehand(pt) = self.mode {
            draw_circles.insert(pt.to_hashable(), "hovered");

            if let Some(last) = self.route.waypoints.last() {
                let last_pt = match *last {
                    Waypoint::Snapped(node) => self.router.map.node(node),
                    Waypoint::Free(pt) => pt,
                };
                let pl = PolyLine::unchecked_new(vec![last_pt, pt]);
                result.push((
                    pl.to_geojson(Some(&self.router.map.gps_bounds)),
                    serde_json::Map::new(),
                ));
            }
        }

        // Partially overlapping circles cover each other up, so make sure the important ones are
        // drawn last
        let mut draw_circles: Vec<(HashablePt2D, &'static str)> =
            draw_circles.into_iter().collect();
        draw_circles.sort_by_key(|(_, style)| match *style {
            "hovered" => 3,
            "important" => 2,
            "unimportant" => 1,
            _ => unreachable!(),
        });

        for (pt, label) in draw_circles {
            let mut props = serde_json::Map::new();
            props.insert("type".to_string(), label.to_string().into());
            result.push((
                pt.to_pt2d().to_geojson(Some(&self.router.map.gps_bounds)),
                props,
            ));
        }

        // A polygon for the area
        if self.router.config.area_mode {
            if let Some(polygon) = self.into_polygon_area() {
                result.push((polygon, serde_json::Map::new()));
            }
        }

        let obj = geom::geometries_with_properties_to_geojson(result);
        serde_json::to_string_pretty(&obj).unwrap()
    }

    #[wasm_bindgen(js_name = setSnapMode)]
    pub fn set_snap_mode(&mut self, snap_mode: bool) {
        // No freehand in area mode yet
        if !snap_mode && self.router.config.area_mode {
            return;
        }
        self.snap_mode = snap_mode;
    }

    // True if something has changed
    #[wasm_bindgen(js_name = onMouseMove)]
    pub fn on_mouse_move(&mut self, lon: f64, lat: f64, circle_radius_meters: f64) -> bool {
        let pt = LonLat::new(lon, lat).to_pt(&self.router.map.gps_bounds);
        let circle_radius = Distance::meters(circle_radius_meters);

        if self.can_extend_route() && !self.snap_mode && !matches!(self.mode, Mode::Dragging { .. })
        {
            self.mode = Mode::Freehand(pt);
            return true;
        }

        let mut changed = false;
        match self.mode {
            // If we were just in freehand mode and we released the key, go back to snapping
            Mode::Neutral | Mode::Freehand(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius) {
                    self.mode = Mode::Hovering(waypt);
                    changed = true;
                }
            }
            Mode::Hovering(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius) {
                    self.mode = Mode::Hovering(waypt);
                } else {
                    self.mode = Mode::Neutral;
                }
                changed = true;
            }
            Mode::Dragging { idx, at } => {
                let new_waypt = match at {
                    Waypoint::Snapped(_) => self.mouseover_node(pt).map(Waypoint::Snapped),
                    Waypoint::Free(_) => Some(Waypoint::Free(pt)),
                };
                if let Some(new_waypt) = new_waypt {
                    if new_waypt != at {
                        let new_idx = self.route.move_waypoint(&self.router, idx, new_waypt);
                        self.mode = Mode::Dragging {
                            idx: new_idx,
                            at: new_waypt,
                        };
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.dont_hover_new_points();
        }

        changed
    }

    // Can we add new points to the end of the route right now?
    fn can_extend_route(&self) -> bool {
        self.route.waypoints.len() < 2 || self.router.config.extend_route
    }

    // If we shouldn't extend the route right now, then only allow hovering on a point already in
    // the route (for dragging it). Don't hover on any new points.
    fn dont_hover_new_points(&mut self) {
        if !self.can_extend_route() {
            if let Mode::Hovering(waypt) = self.mode {
                if !self.route.full_path.contains(&waypt.to_path_entry()) {
                    // We're not dragging
                    self.mode = Mode::Neutral;
                }
            }
        }
    }

    #[wasm_bindgen(js_name = onClick)]
    pub fn on_click(&mut self) {
        // TODO Allow freehand points for areas, once we can convert existing waypoints
        if !self.router.config.area_mode {
            if let Mode::Freehand(pt) = self.mode {
                self.route.add_waypoint(&self.router, Waypoint::Free(pt));
            }
        }

        if let Mode::Hovering(hover) = self.mode {
            if let Some(idx) = self.route.waypoints.iter().position(|x| *x == hover) {
                // Click an existing waypoint to delete it
                if self.route.is_closed_area() {
                    // Don't go below 2 waypoints (+1 because first=last)
                    // TODO Allow deleting the special first=last waypoint
                    if self.route.waypoints.len() > 3
                        && idx != 0
                        && idx != self.route.waypoints.len() - 1
                    {
                        self.route.waypoints.remove(idx);
                        self.route.recalculate_full_path(&self.router);
                    }
                } else {
                    // Don't delete the only waypoint
                    if self.route.waypoints.len() > 1 {
                        self.route.waypoints.remove(idx);
                        self.route.recalculate_full_path(&self.router);
                        // We're still in a hovering state on the point we just deleted. We may
                        // want to reset and stop hovering on this point.
                        self.dont_hover_new_points();
                    }
                }
            } else {
                // Clicking an intermediate point shouldn't do anything. It's usually a user error;
                // they meant to click and drag.
                if self.route.full_path.contains(&hover.to_path_entry()) {
                    return;
                }

                self.route.add_waypoint(&self.router, hover);
                if self.router.config.area_mode
                    && !self.route.is_closed_area()
                    && self.route.waypoints.len() == 3
                {
                    // Close off the area
                    self.route
                        .add_waypoint(&self.router, self.route.waypoints[0]);
                }
            }
        }
    }

    // True if we should hijack the drag controls
    #[wasm_bindgen(js_name = onDragStart)]
    pub fn on_drag_start(&mut self) -> bool {
        if let Mode::Hovering(at) = self.mode {
            if let Some(idx) = self
                .route
                .full_path
                .iter()
                .position(|x| *x == at.to_path_entry())
            {
                self.mode = Mode::Dragging { idx, at };
                return true;
            }
        }
        false
    }

    // True if we're done dragging
    #[wasm_bindgen(js_name = onMouseUp)]
    pub fn on_mouse_up(&mut self) -> bool {
        if let Mode::Dragging { at, .. } = self.mode {
            self.mode = Mode::Hovering(at);
            return true;
        }
        false
    }

    #[wasm_bindgen(js_name = clearState)]
    pub fn clear_state(&mut self) {
        self.route = Route::new();
        self.mode = Mode::Neutral;
    }

    #[wasm_bindgen(js_name = editExisting)]
    pub fn edit_existing(&mut self, raw_waypoints: JsValue) -> Result<(), JsValue> {
        self.clear_state();

        let waypoints: Vec<RouteWaypoint> = serde_wasm_bindgen::from_value(raw_waypoints)?;

        for waypt in waypoints {
            let pt = LonLat::new(waypt.lon, waypt.lat).to_pt(&self.router.map.gps_bounds);
            if waypt.snapped {
                if let Some(node) = self.mouseover_node(pt) {
                    self.route
                        .add_waypoint(&self.router, Waypoint::Snapped(node));
                } else {
                    return Err(JsValue::from_str("A waypoint didn't snap"));
                }
            } else {
                self.route.add_waypoint(&self.router, Waypoint::Free(pt));
            }
        }

        Ok(())
    }
}

impl JsRouteSnapper {
    // Snaps first to free-drawn points, then nodes
    fn mouseover_something(&self, pt: Pt2D, circle_radius: Distance) -> Option<Waypoint> {
        for waypt in &self.route.waypoints {
            if let Waypoint::Free(x) = waypt {
                if x.dist_to(pt) < circle_radius {
                    return Some(*waypt);
                }
            }
        }

        let node = self.mouseover_node(pt)?;

        // If we've closed off an area, don't snap to other nodes
        if self.route.is_closed_area()
            && !self
                .route
                .full_path
                .contains(&PathEntry::SnappedPoint(node))
        {
            return None;
        }

        Some(Waypoint::Snapped(node))
    }
    fn mouseover_node(&self, pt: Pt2D) -> Option<NodeID> {
        let pt = [pt.x(), pt.y()];
        let node = self.snap_to_nodes.nearest_neighbor(&pt)?;
        Some(node.data)
    }

    fn entire_line_string(&self) -> Option<PolyLine> {
        if self.route.full_path.is_empty() {
            return None;
        }
        let mut pts = Vec::new();

        for entry in &self.route.full_path {
            match entry {
                PathEntry::SnappedPoint(node) => {
                    // There may be an adjacent Edge that contributes geometry, but maybe not near
                    // free points. We'll dedupe later anyway.
                    pts.push(self.router.map.node(*node));
                }
                PathEntry::FreePoint(pt) => {
                    pts.push(*pt);
                }
                PathEntry::Edge(dir_edge) => {
                    pts.extend(edge_geometry(&self.router.map, *dir_edge));
                }
            }
        }

        pts.dedup();
        if pts.len() < 2 {
            return None;
        }
        Some(PolyLine::unchecked_new(pts))
    }

    fn into_polygon_area(&self) -> Option<geojson::Geometry> {
        if !self.route.is_closed_area() {
            return None;
        }
        let pl = self.entire_line_string()?;
        // We could put the points into Ring, but it's too strict about repeating points. Better to
        // just render something.
        let outer_ring = pl
            .into_points()
            .into_iter()
            .map(|pt| {
                let gps = pt.to_gps(&self.router.map.gps_bounds);
                vec![gps.x(), gps.y()]
            })
            .collect();
        Some(geojson::Geometry::from(geojson::Value::Polygon(vec![
            outer_ring,
        ])))
    }

    fn to_pt(&self, waypt: Waypoint) -> HashablePt2D {
        match waypt {
            Waypoint::Snapped(node) => self.router.map.node(node),
            Waypoint::Free(pt) => pt,
        }
        .to_hashable()
    }
}

impl Route {
    fn new() -> Route {
        Route {
            waypoints: Vec::new(),
            full_path: Vec::new(),
        }
    }

    fn add_waypoint(&mut self, router: &Router, waypt: Waypoint) {
        if self.waypoints.is_empty() {
            self.waypoints.push(waypt);
            assert!(self.full_path.is_empty());
            // TODO Do we need to have the one PathEntry?
        } else {
            self.waypoints.push(waypt);
            self.recalculate_full_path(router);
        }
    }

    // Returns the new full_path index
    fn move_waypoint(&mut self, router: &Router, full_idx: usize, new_waypt: Waypoint) -> usize {
        let old_waypt = self.full_path[full_idx].to_waypt().unwrap();

        // Edge case when we've placed just one point, then try to drag it
        if self.waypoints.len() == 1 {
            assert!(self.waypoints[0] == old_waypt);
            self.waypoints = vec![new_waypt];
            self.full_path.clear();
            return 0;
        }

        // Move an existing waypoint?
        if let Some(way_idx) = self.waypoints.iter().position(|x| *x == old_waypt) {
            if self.is_closed_area() && way_idx == 0 {
                // way_idx will never be the last; 0 will match first
                self.waypoints[0] = new_waypt;
                *self.waypoints.last_mut().unwrap() = new_waypt;
            } else {
                self.waypoints[way_idx] = new_waypt;
            }
        } else {
            // Find the next waypoint after this one
            for (idx_offset, entry) in self.full_path[full_idx..].iter().enumerate() {
                // Special case for areas: the first and last waypoints are equal. If we scan all
                // the way to the end of full_path, treat it as the last
                if self.is_closed_area() && full_idx + idx_offset == self.full_path.len() - 1 {
                    self.waypoints.insert(self.waypoints.len() - 1, new_waypt);
                    break;
                }

                if let Some(way_idx) = self
                    .waypoints
                    .iter()
                    .position(|x| x.to_path_entry() == *entry)
                {
                    // Insert a new waypoint before this
                    self.waypoints.insert(way_idx, new_waypt);
                    break;
                }
            }
        }

        self.recalculate_full_path(router);
        self.full_path
            .iter()
            .position(|x| x.to_waypt() == Some(new_waypt))
            .unwrap()
    }

    // It might be possible for callers to recalculate something smaller, but it's not worth the
    // complexity
    fn recalculate_full_path(&mut self, router: &Router) {
        self.full_path.clear();

        for pair in self.waypoints.windows(2) {
            // Always add every waypoint
            self.full_path.push(pair[0].to_path_entry());

            if let [Waypoint::Snapped(node1), Waypoint::Snapped(node2)] = pair {
                if let Some(entries) = router.pathfind(*node1, *node2, &self.full_path) {
                    // Don't repeat that snapped point
                    assert_eq!(self.full_path.pop(), Some(PathEntry::SnappedPoint(*node1)));
                    self.full_path.extend(entries);
                }
                // If the points are disconnected in the graph, just act like there's a freehand
                // line between them. It's better than breaking.
                // (We don't need to do anything here -- the other point will get added)
            }
        }
        // Always add the last if it's different
        if let Some(last) = self.waypoints.last() {
            let add = last.to_path_entry();
            if self.full_path.last() != Some(&add) {
                self.full_path.push(add);
            }
        }
    }

    fn is_closed_area(&self) -> bool {
        // TODO When area mode is false, somebody could make a linestring like this and mess things
        // up
        self.waypoints.len() >= 2 && self.waypoints[0] == *self.waypoints.last().unwrap()
    }
}

impl Router {
    // Returns a sequence of (SnappedPoint, Edge, SnappedPoint, Edge..., SnappedPoint)
    fn pathfind(
        &self,
        node1: NodeID,
        node2: NodeID,
        prev_path: &Vec<PathEntry>,
    ) -> Option<Vec<PathEntry>> {
        // Penalize visiting edges we've been to before, so that waypoints don't cause us to double
        // back
        // TODO Seems fast enough, but we could cache and build this up incrementally
        let mut avoid = HashSet::new();
        if self.config.avoid_doubling_back {
            for entry in prev_path {
                if let PathEntry::Edge(e) = entry {
                    avoid.insert(e.0);
                }
            }
        }

        let node2_pt = self.map.node(node2);

        let (_, path) = petgraph::algo::astar(
            &self.graph,
            node1,
            |i| i == node2,
            |(_, _, dir_edge)| {
                let penalty = if avoid.contains(&dir_edge.0) {
                    2.0
                } else {
                    1.0
                };
                penalty * self.map.edge(dir_edge.0).length
            },
            |i| self.map.node(i).dist_to(node2_pt),
        )?;

        let mut entries = Vec::new();
        for pair in path.windows(2) {
            entries.push(PathEntry::SnappedPoint(pair[0]));
            entries.push(PathEntry::Edge(
                *self.graph.edge_weight(pair[0], pair[1]).unwrap(),
            ));
        }
        entries.push(PathEntry::SnappedPoint(*path.last().unwrap()));
        assert!(entries[0] == PathEntry::SnappedPoint(node1));
        assert!(*entries.last().unwrap() == PathEntry::SnappedPoint(node2));
        Some(entries)
    }
}

fn edge_geometry(map: &RouteSnapperMap, dir_edge: DirectedEdge) -> Vec<Pt2D> {
    let mut pts = map.edge(dir_edge.0).geometry.clone().into_points();
    if dir_edge.1 == BACKWARDS {
        pts.reverse();
    }
    pts
}

fn err_to_js<E: std::fmt::Display>(err: E) -> JsValue {
    JsValue::from_str(&err.to_string())
}

// Encode a route's waypoints as GeoJSON properties, so we can later losslessly restore a route
#[derive(Serialize, Deserialize)]
struct RouteWaypoint {
    lon: f64,
    lat: f64,
    snapped: bool,
}
