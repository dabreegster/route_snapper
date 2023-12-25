#[macro_use]
extern crate log;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write;
use std::sync::Once;

use geo::{Coord, HaversineDistance, HaversineLength, LineString, Point, Polygon};
use geojson::{Feature, FeatureCollection, Geometry};
use petgraph::graphmap::DiGraphMap;
use rstar::primitives::GeomWithData;
use rstar::RTree;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use route_snapper_graph::{EdgeID, NodeID, RouteSnapperMap};

static START: Once = Once::new();

const MAX_PREVIOUS_STATES: usize = 100;

type Graph = DiGraphMap<NodeID, DirectedEdge>;

#[wasm_bindgen]
pub struct JsRouteSnapper {
    router: Router,
    snap_to_nodes: RTree<GeomWithData<[f64; 2], NodeID>>,
    route: Route,
    mode: Mode,
    snap_mode: bool,
    // Copies of route.waypoints are sufficient to represent state
    previous_states: Vec<Vec<Waypoint>>,
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
    Free(Coord),
}

impl Waypoint {
    fn to_path_entry(self) -> PathEntry {
        match self {
            Waypoint::Snapped(x) => PathEntry::SnappedPoint(x),
            Waypoint::Free(x) => PathEntry::FreePoint(x),
        }
    }

    fn to_color_name(self) -> &'static str {
        match self {
            Waypoint::Snapped(_) => "snapped-waypoint",
            Waypoint::Free(_) => "free-waypoint",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathEntry {
    SnappedPoint(NodeID),
    FreePoint(Coord),
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
    // TODO It'd be simpler if this was only hovering on an existing node. Make a new state for
    // appending a snapped point.
    Hovering(Waypoint),
    // idx is into full_path
    Dragging { idx: usize, at: Waypoint },
    // TODO Rename? This is appending a freehand
    Freehand(Coord),
}

#[wasm_bindgen]
impl JsRouteSnapper {
    #[wasm_bindgen(constructor)]
    pub fn new(map_bytes: &[u8]) -> Result<JsRouteSnapper, JsValue> {
        if !cfg!(test) {
            START.call_once(|| {
                console_log::init_with_level(log::Level::Info).unwrap();
            });
            // Panics shouldn't happen, but if they do, console.log them.
            console_error_panic_hook::set_once();
        }

        info!("Got {} bytes, deserializing", map_bytes.len());

        let mut map: RouteSnapperMap = bincode::deserialize(map_bytes).map_err(err_to_js)?;
        for edge in &mut map.edges {
            edge.length_meters = edge.geometry.haversine_length();
        }

        info!("Finalizing JsRouteSnapper");

        let mut graph: Graph = DiGraphMap::new();
        for (idx, e) in map.edges.iter().enumerate() {
            let id = EdgeID(idx as u32);
            graph.add_edge(e.node1, e.node2, DirectedEdge(id, FORWARDS));
            graph.add_edge(e.node2, e.node1, DirectedEdge(id, BACKWARDS));
        }

        // Euclidean distance on WGS84 coordinates works because we're just finding the closest
        // point to the cursor, and always in a pretty small area. Using GeodesicDistance as a
        // distance function is an alternative.
        let mut nodes = Vec::new();
        for (idx, pt) in map.nodes.iter().enumerate() {
            nodes.push(GeomWithData::new([pt.x, pt.y], NodeID(idx as u32)));
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
            previous_states: Vec::new(),
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
        self.snap_mode = true;
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
            let linestring = self.entire_line_string()?;
            let length = linestring.haversine_length();
            let mut f = Feature::from(Geometry::from(&linestring));
            f.set_property("length_meters", length);

            let from_name = self.name_waypoint(&self.route.waypoints[0]);
            let to_name = self.name_waypoint(self.route.waypoints.last().as_ref().unwrap());
            f.set_property("route_name", format!("Route from {from_name} to {to_name}"));

            f
        };

        // Set these on both LineStrings and Polygons
        let mut waypoints = Vec::new();
        for waypt in &self.route.waypoints {
            let pt = unhash_pt(self.to_pt(*waypt));
            waypoints.push(
                serde_json::to_value(&RouteWaypoint {
                    lon: trim_lon_lat(pt.x),
                    lat: trim_lon_lat(pt.y),
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
        // 1) "snapped-waypoint": A snapped waypoint
        // 2) "free-waypoint": A freehand waypoint
        // 3) "node": A draggable snapped node on the route, not a waypoint
        //
        // Store (style, optional intersection name)
        //
        // TODO Only draw each circle once, instead of overlapping.
        let mut draw_circles: BTreeMap<HashedPoint, (&'static str, Option<String>)> =
            BTreeMap::new();

        // Draw the confirmed route
        result.extend(self.line_string_broken_down());
        for entry in &self.route.full_path {
            // Every free point is a waypoint, so just handle it below
            if let PathEntry::SnappedPoint(node) = entry {
                draw_circles.insert(hash_pt(self.router.map.node(*node)), ("node", None));
            }
        }
        for waypt in &self.route.waypoints {
            draw_circles.insert(
                self.to_pt(*waypt),
                (waypt.to_color_name(), Some(self.name_waypoint(waypt))),
            );
        }

        // Draw the current operation
        if let Mode::Hovering(hover) = self.mode {
            draw_circles.insert(
                self.to_pt(hover),
                (hover.to_color_name(), Some(self.name_waypoint(&hover))),
            );

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
                                        let mut f =
                                            Feature::from(Geometry::from(&LineString::new(
                                                edge_geometry(&self.router.map, dir_edge),
                                            )));
                                        f.set_property("snapped", true);
                                        result.push(f);
                                    }
                                }
                            } else {
                                // It'll be a straight line
                                let mut f = Feature::from(Geometry::from(&LineString::new(vec![
                                    self.router.map.node(*last),
                                    self.router.map.node(current),
                                ])));
                                f.set_property("snapped", false);
                                result.push(f);
                            }
                        }
                        Waypoint::Free(pt) => {
                            let mut f = Feature::from(Geometry::from(&LineString::new(vec![
                                *pt,
                                self.router.map.node(current),
                            ])));
                            f.set_property("snapped", false);
                            result.push(f);
                        }
                    }
                }
            }
        }
        if let Mode::Dragging { at, .. } = self.mode {
            draw_circles.insert(
                self.to_pt(at),
                (at.to_color_name(), Some(self.name_waypoint(&at))),
            );
        }
        if let Mode::Freehand(pt) = self.mode {
            draw_circles.insert(hash_pt(pt), ("free-waypoint", None));

            if let Some(last) = self.route.waypoints.last() {
                let last_pt = match *last {
                    Waypoint::Snapped(node) => self.router.map.node(node),
                    Waypoint::Free(pt) => pt,
                };
                let mut f = Feature::from(Geometry::from(&LineString::new(vec![last_pt, pt])));
                f.set_property("snapped", false);
                result.push(f);
            }
        }

        // Partially overlapping circles cover each other up, so make sure the important ones are
        // drawn last
        let mut draw_circles: Vec<(HashedPoint, &'static str, Option<String>)> = draw_circles
            .into_iter()
            .map(|(key, (v1, v2))| (key, v1, v2))
            .collect();
        draw_circles.sort_by_key(|(_, style, _)| match *style {
            "snapped-waypoint" => 3,
            "free-waypoint" => 2,
            "node" => 1,
            _ => unreachable!(),
        });

        let hovering_pt = match self.mode {
            Mode::Neutral => None,
            Mode::Hovering(pt) => Some(self.to_pt(pt)),
            Mode::Dragging { at, .. } => Some(self.to_pt(at)),
            Mode::Freehand(pt) => Some(hash_pt(pt)),
        };

        for (pt, label, maybe_name) in draw_circles {
            let mut f = Feature::from(Geometry::from(&Point::from(unhash_pt(pt))));
            f.set_property("type", label.to_string());
            if hovering_pt == Some(pt) {
                f.set_property("hovered", true);
            }
            if let Some(name) = maybe_name {
                // Skip freehand points
                if name != "???" {
                    f.set_property("name", name);
                }
            }
            result.push(f);
        }

        // A polygon for the area
        if self.router.config.area_mode {
            if let Some(polygon) = self.into_polygon_area() {
                result.push(Feature::from(polygon));
            }
        }

        let cursor = match self.mode {
            Mode::Neutral => "inherit",
            Mode::Hovering(_) => "pointer",
            Mode::Dragging { .. } => "grabbing",
            Mode::Freehand(_) => "crosshair",
        };
        let fc = FeatureCollection {
            features: result,
            bbox: None,
            foreign_members: Some(
                serde_json::json!({
                    "cursor": cursor,
                    "snap_mode": self.snap_mode,
                    "undo_length": self.previous_states.len(),
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };
        serde_json::to_string_pretty(&fc).unwrap()
    }

    #[wasm_bindgen(js_name = toggleSnapMode)]
    pub fn toggle_snap_mode(&mut self) {
        // Can't do this in area mode yet
        if self.router.config.area_mode {
            return;
        }
        self.snap_mode = !self.snap_mode;

        // Based on the current mode, immediately change something
        match self.mode {
            Mode::Neutral => {}
            Mode::Hovering(waypt) => {
                // Are we appending a snapped node?
                // TODO This repeats logic from on_click to figure out if this is a new node. Split
                // Mode into more cases instead.
                if !self.route.full_path.contains(&waypt.to_path_entry()) {
                    self.mode = Mode::Freehand(unhash_pt(self.to_pt(waypt)));
                }
            }
            Mode::Dragging { at, idx } => {
                let new_waypt = match at {
                    Waypoint::Snapped(node) => Waypoint::Free(self.router.map.node(node)),
                    Waypoint::Free(pt) => {
                        if let Some(node) = self.mouseover_node(pt) {
                            Waypoint::Snapped(node)
                        } else {
                            // TODO Couldn't convert a free point to snapped! What do we do now?
                            self.snap_mode = false;
                            return;
                        }
                    }
                };
                // Don't keep every single update during a drag
                let new_idx = self.route.move_waypoint(&self.router, idx, new_waypt);
                self.mode = Mode::Dragging {
                    idx: new_idx,
                    at: new_waypt,
                };
            }
            Mode::Freehand(pt) => {
                if let Some(node) = self.mouseover_node(pt) {
                    self.mode = Mode::Hovering(Waypoint::Snapped(node));
                } else {
                    // TODO Couldn't convert a free point to snapped! What do we do now?
                    self.snap_mode = false;
                    return;
                }
            }
        }
    }

    // True if something has changed
    #[wasm_bindgen(js_name = onMouseMove)]
    pub fn on_mouse_move(&mut self, lon: f64, lat: f64, circle_radius_meters: f64) -> bool {
        let pt = Coord { x: lon, y: lat };

        if self.can_extend_route() && !self.snap_mode && !matches!(self.mode, Mode::Dragging { .. })
        {
            self.mode = Mode::Freehand(pt);
            return true;
        }

        let mut changed = false;
        match self.mode {
            // If we were just in freehand mode and we released the key, go back to snapping
            Mode::Neutral | Mode::Freehand(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius_meters) {
                    self.mode = Mode::Hovering(waypt);
                    changed = true;
                }
            }
            Mode::Hovering(_) => {
                if let Some(waypt) = self.mouseover_something(pt, circle_radius_meters) {
                    self.mode = Mode::Hovering(waypt);
                } else {
                    self.mode = Mode::Neutral;
                }
                changed = true;
            }
            Mode::Dragging { idx, at } => {
                // Keep the same snapped/free type here. Toggling will change this current
                // waypoint.
                let new_waypt = match at {
                    Waypoint::Snapped(_) => self.mouseover_node(pt).map(Waypoint::Snapped),
                    Waypoint::Free(_) => Some(Waypoint::Free(pt)),
                };
                if let Some(new_waypt) = new_waypt {
                    if new_waypt != at {
                        // Don't keep every single update during a drag
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
                self.before_update();
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
                        self.before_update();
                        self.route.waypoints.remove(idx);
                        self.route.recalculate_full_path(&self.router);
                    }
                } else {
                    // Don't delete the only waypoint
                    if self.route.waypoints.len() > 1 {
                        self.before_update();
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

                self.before_update();
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
                // TODO Only do this for the first actual bit of drag?
                self.before_update();
                self.mode = Mode::Dragging { idx, at };
                self.snap_mode = matches!(at, Waypoint::Snapped(_));
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
        self.snap_mode = true;
        self.previous_states.clear();
    }

    #[wasm_bindgen(js_name = editExisting)]
    pub fn edit_existing(&mut self, raw_waypoints: JsValue) -> Result<(), JsValue> {
        self.clear_state();

        let waypoints: Vec<RouteWaypoint> = serde_wasm_bindgen::from_value(raw_waypoints)?;

        for waypt in waypoints {
            let pt = Coord {
                x: waypt.lon,
                y: waypt.lat,
            };
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

    /// Render the graph as GeoJSON points and line-strings, for debugging.
    #[wasm_bindgen(js_name = debugRenderGraph)]
    pub fn debug_render_graph(&self) -> String {
        let mut features = Vec::new();
        for pt in &self.router.map.nodes {
            features.push(Feature::from(Geometry::from(&Point::from(*pt))));
        }
        for edge in &self.router.map.edges {
            features.push(Feature::from(Geometry::from(&edge.geometry)));
        }
        let gj =
            geojson::GeoJson::from(features.into_iter().collect::<geojson::FeatureCollection>());
        serde_json::to_string_pretty(&gj).unwrap()
    }

    #[wasm_bindgen(js_name = routeNameForWaypoints)]
    pub fn route_name_for_waypoints(&self, raw_waypoints: JsValue) -> Result<String, JsValue> {
        let waypoints: Vec<RouteWaypoint> = serde_wasm_bindgen::from_value(raw_waypoints)?;
        let from_name = self.name_for_waypoint(&waypoints[0])?;
        let to_name = self.name_for_waypoint(waypoints.last().unwrap())?;
        Ok(format!("Route from {from_name} to {to_name}"))
    }

    #[wasm_bindgen(js_name = addSnappedWaypoint)]
    pub fn add_snapped_waypoint(&mut self, lon: f64, lat: f64) {
        // Not supported yet
        if self.router.config.area_mode {
            return;
        }
        let pt = Coord { x: lon, y: lat };
        if let Some(node) = self.mouseover_node(pt) {
            self.before_update();
            self.route
                .add_waypoint(&self.router, Waypoint::Snapped(node));
        }
    }

    #[wasm_bindgen()]
    pub fn undo(&mut self) {
        if let Mode::Dragging { .. } = self.mode {
            // Too confusing
            return;
        }
        if let Some(state) = self.previous_states.pop() {
            self.route.waypoints = state;
            self.route.recalculate_full_path(&self.router);
        }
    }

    fn name_for_waypoint(&self, waypoint: &RouteWaypoint) -> Result<String, JsValue> {
        if !waypoint.snapped {
            return Ok("???".to_string());
        }

        let pt = Coord {
            x: waypoint.lon,
            y: waypoint.lat,
        };
        if let Some(node) = self.mouseover_node(pt) {
            Ok(self.name_waypoint(&Waypoint::Snapped(node)))
        } else {
            return Err(JsValue::from_str("A waypoint didn't snap"));
        }
    }

    fn before_update(&mut self) {
        self.previous_states.push(self.route.waypoints.clone());
        // TODO Different data structure to make this more efficient
        if self.previous_states.len() > MAX_PREVIOUS_STATES {
            self.previous_states.remove(0);
        }
    }
}

impl JsRouteSnapper {
    // Snaps first to free-drawn points, then nodes
    fn mouseover_something(&self, pt: Coord, circle_radius_meters: f64) -> Option<Waypoint> {
        for waypt in &self.route.waypoints {
            if let Waypoint::Free(x) = waypt {
                if Point::from(*x).haversine_distance(&Point::from(pt)) < circle_radius_meters {
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
    fn mouseover_node(&self, pt: Coord) -> Option<NodeID> {
        let pt = [pt.x, pt.y];
        let node = self.snap_to_nodes.nearest_neighbor(&pt)?;
        Some(node.data)
    }

    fn entire_line_string(&self) -> Option<LineString> {
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
        Some(LineString::new(pts))
    }

    // Returns the entire_line_string, but broken into pieces with a snapped=true/false property.
    fn line_string_broken_down(&self) -> Vec<Feature> {
        let mut result = Vec::new();
        if self.route.full_path.is_empty() {
            return result;
        }
        let mut add_result = |mut pts: Vec<Coord>, snapped: bool| {
            pts.dedup();
            if pts.len() >= 2 {
                let mut f = Feature::from(Geometry::from(&LineString::new(pts)));
                f.set_property("snapped", snapped);
                result.push(f);
            }
        };

        let mut prev_snapped = !matches!(self.route.full_path[0], PathEntry::FreePoint(_));
        let mut pts = Vec::new();

        for entry in &self.route.full_path {
            let pt = match entry {
                PathEntry::SnappedPoint(node) => self.router.map.node(*node),
                PathEntry::FreePoint(pt) => *pt,
                PathEntry::Edge(dir_edge) => {
                    pts.extend(edge_geometry(&self.router.map, *dir_edge));
                    continue;
                }
            };
            let snapped = !matches!(entry, PathEntry::FreePoint(_));

            if prev_snapped == snapped {
                pts.push(pt);
            } else if prev_snapped {
                // Starting freehand
                let last_pt = *pts.last().unwrap();
                add_result(std::mem::take(&mut pts), true);
                prev_snapped = false;
                pts = vec![last_pt, pt];
            } else {
                // Starting snapped
                pts.push(pt);
                add_result(std::mem::take(&mut pts), false);
                prev_snapped = true;
                pts = vec![pt];
            }
        }

        // Handle the last transition
        add_result(std::mem::take(&mut pts), prev_snapped);

        result
    }

    fn into_polygon_area(&self) -> Option<Geometry> {
        if !self.route.is_closed_area() {
            return None;
        }
        let exterior = self.entire_line_string()?;
        Some(geojson::Geometry::from(&Polygon::new(exterior, Vec::new())))
    }

    fn to_pt(&self, waypt: Waypoint) -> HashedPoint {
        match waypt {
            Waypoint::Snapped(node) => hash_pt(self.router.map.node(node)),
            Waypoint::Free(pt) => hash_pt(pt),
        }
    }

    fn name_waypoint(&self, waypt: &Waypoint) -> String {
        match waypt {
            Waypoint::Snapped(node) => {
                let edge_names = self
                    .router
                    .graph
                    .edges(*node)
                    .map(|(_, _, edge)| {
                        self.router.map.edges[edge.0 .0 as usize]
                            .name
                            .clone()
                            .unwrap_or_else(|| "???".to_string())
                    })
                    .collect::<BTreeSet<_>>();
                plain_list_names(edge_names)
            }
            Waypoint::Free(_) => "???".to_string(),
        }
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
                penalty * self.map.edge(dir_edge.0).length_meters
            },
            |i| Point::from(self.map.node(i)).haversine_distance(&Point::from(node2_pt)),
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

fn edge_geometry(map: &RouteSnapperMap, dir_edge: DirectedEdge) -> Vec<Coord> {
    let mut pts = map.edge(dir_edge.0).geometry.clone().into_inner();
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

// Per https://datatracker.ietf.org/doc/html/rfc7946#section-11.2, 6 decimal places (10cm) is
// plenty of precision
fn trim_lon_lat(x: f64) -> f64 {
    (x * 10e6).round() / 10e6
}

fn plain_list_names(names: BTreeSet<String>) -> String {
    let mut s = String::new();
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    write!(s, " and ").unwrap();
                } else {
                    write!(s, ", and ").unwrap();
                }
            } else {
                write!(s, ", ").unwrap();
            }
        }
        write!(s, "{}", n).unwrap();
    }
    s
}

// TODO Hack, make render_geojson do something simpler
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
struct HashedPoint(i64, i64);
fn hash_pt(pt: Coord) -> HashedPoint {
    HashedPoint((pt.x * 10e6) as i64, (pt.y * 10e6) as i64)
}
fn unhash_pt(pt: HashedPoint) -> Coord {
    Coord {
        x: pt.0 as f64 / 10e6,
        y: pt.1 as f64 / 10e6,
    }
}
