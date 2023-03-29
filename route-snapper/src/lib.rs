use std::collections::{BTreeMap, HashSet};

use geom::{Distance, HashablePt2D, LonLat, PolyLine, Pt2D};
use petgraph::graphmap::DiGraphMap;
use rstar::primitives::GeomWithData;
use rstar::RTree;
use serde::Deserialize;
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

#[derive(Default, Deserialize)]
struct Config {
    avoid_doubling_back: bool,
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

#[derive(Clone, PartialEq)]
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
        // Panics shouldn't happen, but if they do, console.log them.
        console_error_panic_hook::set_once();

        web_sys::console::log_1(&format!("Got {} bytes, deserializing", map_bytes.len()).into());

        let mut map: RouteSnapperMap = bincode::deserialize(map_bytes).map_err(err_to_js)?;
        for edge in &mut map.edges {
            edge.length = edge.geometry.length();
        }

        web_sys::console::log_1(&"Finalizing JsRouteSnapper".into());

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
    #[wasm_bindgen(js_name = setConfig)]
    pub fn set_config(&mut self, input: JsValue) {
        match serde_wasm_bindgen::from_value(input) {
            Ok(config) => {
                self.router.config = config;
                self.route.recalculate_full_path(&self.router);
            }
            Err(err) => {
                web_sys::console::log_1(&format!("Bad input to setConfig: {}", err).into());
            }
        }
    }

    #[wasm_bindgen(js_name = toFinalFeature)]
    pub fn to_final_feature(&self) -> Option<String> {
        let pl = self.entire_line_string()?;
        let mut feature = geojson::Feature {
            bbox: None,
            geometry: Some(pl.to_geojson(Some(&self.router.map.gps_bounds))),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("length_meters", pl.length().inner_meters());
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
                // If we're trying to drag a point, don't show this preview
                if !self
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

        let obj = geom::geometries_with_properties_to_geojson(result);
        serde_json::to_string_pretty(&obj).unwrap()
    }

    #[wasm_bindgen(js_name = setSnapMode)]
    pub fn set_snap_mode(&mut self, snap_mode: bool) {
        self.snap_mode = snap_mode;
    }

    // True if something has changed
    #[wasm_bindgen(js_name = onMouseMove)]
    pub fn on_mouse_move(&mut self, lon: f64, lat: f64, circle_radius_meters: f64) -> bool {
        let pt = LonLat::new(lon, lat).to_pt(&self.router.map.gps_bounds);
        let circle_radius = Distance::meters(circle_radius_meters);

        if !self.snap_mode && !matches!(self.mode, Mode::Dragging { .. }) {
            self.mode = Mode::Freehand(pt);
            return true;
        }

        match self.mode {
            // If we were just in freehand mode and we released the key, go back to snapping
            Mode::Neutral | Mode::Freehand(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius) {
                    self.mode = Mode::Hovering(waypt);
                    return true;
                }
            }
            Mode::Hovering(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius) {
                    self.mode = Mode::Hovering(waypt);
                } else {
                    self.mode = Mode::Neutral;
                }
                return true;
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
                        return true;
                    }
                }
            }
        }

        false
    }

    #[wasm_bindgen(js_name = onClick)]
    pub fn on_click(&mut self) {
        if let Mode::Freehand(pt) = self.mode {
            self.route.add_waypoint(&self.router, Waypoint::Free(pt));
        }

        if let Mode::Hovering(hover) = self.mode {
            if let Some(idx) = self.route.waypoints.iter().position(|x| *x == hover) {
                // If we click on an existing waypoint and it's not the first or last, delete it
                if !self.route.waypoints.is_empty()
                    && idx != 0
                    && idx != self.route.waypoints.len() - 1
                {
                    self.route.waypoints.remove(idx);
                    self.route.recalculate_full_path(&self.router);
                }
            } else {
                self.route.add_waypoint(&self.router, hover);
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
    pub fn edit_existing(&mut self, raw_geojson: &str) -> Result<(), JsValue> {
        self.clear_state();

        let require_in_bounds = true;
        let mut results = PolyLine::from_geojson_bytes(
            raw_geojson.as_bytes(),
            &self.router.map.gps_bounds,
            require_in_bounds,
        )
        .map_err(err_to_js)?;
        if results.len() != 1 {
            return Err(JsValue::from_str(
                "editExisting didn't get exactly 1 LineString as input",
            ));
        }
        let pl = results.pop().unwrap().0;

        // Just start with the endpoints snapped to a node
        if let (Some(node1), Some(node2)) = (
            self.mouseover_node(pl.first_pt()),
            self.mouseover_node(pl.last_pt()),
        ) {
            self.route
                .add_waypoint(&self.router, Waypoint::Snapped(node1));
            self.route
                .add_waypoint(&self.router, Waypoint::Snapped(node2));
        } else {
            return Err(JsValue::from_str("endpoints didn't snap"));
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
            self.waypoints[way_idx] = new_waypt;
        } else {
            // Find the next waypoint after this one
            for entry in &self.full_path[full_idx..] {
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
