use crate::*;

// The NodeIDs depend on the real southwark.bin graph! If the path between two nodes happens to
// include a third node, then a test may be confusing, because it could look like an intermediate
// point.
const WAYPT1: Waypoint = Waypoint::Snapped(NodeID(1));
const WAYPT2: Waypoint = Waypoint::Snapped(NodeID(2));
const WAYPT3: Waypoint = Waypoint::Snapped(NodeID(3));
const WAYPT4: Waypoint = Waypoint::Snapped(NodeID(4));
const WAYPT5: Waypoint = Waypoint::Snapped(NodeID(5));

// TODO Test freehand points with all of the variations of tests below

#[test]
fn test_route_delete() {
    let map_bytes = std::fs::read("../examples/southwark.bin").unwrap();
    let mut snapper = JsRouteSnapper::new(&map_bytes).unwrap();

    // Add a waypoint
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1]);

    // Click again to delete it -- shouldn't work, it's the only one
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1]);

    // Add a second
    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2]);

    // Delete the first waypoint
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT2]);
}

#[test]
fn test_route_extend() {
    let map_bytes = std::fs::read("../examples/southwark.bin").unwrap();
    let mut snapper = JsRouteSnapper::new(&map_bytes).unwrap();
    // setRouteConfig is a WASM API awkward to call; just set directly
    snapper.router.config.extend_route = true;

    // Add many waypoints
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1]);

    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2]);

    must_mouseover_waypt(&mut snapper, WAYPT3);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2, WAYPT3]);

    must_mouseover_waypt(&mut snapper, WAYPT4);
    snapper.on_click();
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT1, WAYPT2, WAYPT3, WAYPT4]
    );

    // TODO Drag
}

#[test]
fn test_route_extend_then_delete() {
    let map_bytes = std::fs::read("../examples/southwark.bin").unwrap();
    let mut snapper = JsRouteSnapper::new(&map_bytes).unwrap();
    // setRouteConfig is a WASM API awkward to call; just set directly
    snapper.router.config.extend_route = true;

    // Add many waypoints
    for waypt in [WAYPT1, WAYPT2, WAYPT3, WAYPT4] {
        must_mouseover_waypt(&mut snapper, waypt);
        snapper.on_click();
    }
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT1, WAYPT2, WAYPT3, WAYPT4]
    );

    // Clicking an existing waypoint will delete it
    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT3, WAYPT4]);
}

#[test]
fn test_route_dont_extend() {
    let map_bytes = std::fs::read("../examples/southwark.bin").unwrap();
    let mut snapper = JsRouteSnapper::new(&map_bytes).unwrap();
    // setRouteConfig is a WASM API awkward to call; just set directly
    snapper.router.config.extend_route = false;

    // The first two waypoints work normally
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1]);

    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2]);

    // But then we can't add another one
    optionally_mouseover_waypt(&mut snapper, WAYPT3);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2]);

    // Delete the first waypoint
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT2]);

    // Then add a different endpoint
    must_mouseover_waypt(&mut snapper, WAYPT4);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT2, WAYPT4]);

    // TODO Drag
}

#[test]
fn test_area() {
    let map_bytes = std::fs::read("../examples/southwark.bin").unwrap();
    let mut snapper = JsRouteSnapper::new(&map_bytes).unwrap();
    snapper.set_area_mode();

    // The first two points just make a line
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1]);

    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2]);

    // The third creates the polygon, and the first and last waypoints are now the same
    must_mouseover_waypt(&mut snapper, WAYPT3);
    snapper.on_click();
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT1, WAYPT2, WAYPT3, WAYPT1]
    );

    // Can't delete an intermediate waypoint if there aren't enough
    // TODO Actually not true; decide whether the behavior now is what we want or not
    /*must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(snapper.route.waypoints, vec![WAYPT1, WAYPT2, WAYPT3, WAYPT1]);*/

    // Drag something in between 1 and 2
    let intermediate = find_intermediate_point(&snapper, WAYPT1, WAYPT2);
    // Just make sure it's not one of the arbitrary IDs we chose. Manually figured out this ID.
    assert_eq!(intermediate, Waypoint::Snapped(NodeID(211)));
    drag(&mut snapper, intermediate, WAYPT4);
    // We should've introduced a new waypoint
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT1, WAYPT4, WAYPT2, WAYPT3, WAYPT1]
    );

    // Due to a current limitation, we can't delete the first/last waypoint
    must_mouseover_waypt(&mut snapper, WAYPT1);
    snapper.on_click();
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT1, WAYPT4, WAYPT2, WAYPT3, WAYPT1]
    );

    // If we modify the first point, the last stays in sync
    drag(&mut snapper, WAYPT1, WAYPT5);
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT5, WAYPT4, WAYPT2, WAYPT3, WAYPT5]
    );

    // We can delete an intermediate point
    must_mouseover_waypt(&mut snapper, WAYPT2);
    snapper.on_click();
    assert_eq!(
        snapper.route.waypoints,
        vec![WAYPT5, WAYPT4, WAYPT3, WAYPT5]
    );
}

// Simulate the mouse being somewhere
fn optionally_mouseover_waypt(snapper: &mut JsRouteSnapper, waypt: Waypoint) {
    let gps = snapper
        .to_pt(waypt)
        .to_pt2d()
        .to_gps(&snapper.router.map.gps_bounds);
    let circle_radius_meters = 1.0;
    snapper.on_mouse_move(gps.x(), gps.y(), circle_radius_meters);
}

// Simulate the mouse being somewhere, then check the tool is hovering on that waypoint
fn must_mouseover_waypt(snapper: &mut JsRouteSnapper, waypt: Waypoint) {
    optionally_mouseover_waypt(snapper, waypt);
    assert_eq!(snapper.mode, Mode::Hovering(waypt));
}

// After the route containts two waypoints, find some point in the middle of the two
fn find_intermediate_point(snapper: &JsRouteSnapper, pt1: Waypoint, pt2: Waypoint) -> Waypoint {
    // First get the full path without PathEntry::Edges
    let all_points: Vec<Waypoint> = snapper
        .route
        .full_path
        .iter()
        .flat_map(|path_entry| path_entry.to_waypt())
        .collect();

    let idx1 = all_points.iter().position(|x| *x == pt1).unwrap();
    let idx2 = all_points.iter().position(|x| *x == pt2).unwrap();
    assert!(idx1 < idx2);
    // Pick something in the middle
    let middle = idx1 + ((idx2 - idx1) as f64 / 2.0) as usize;
    assert_ne!(middle, idx1);
    all_points[middle]
}

fn drag(snapper: &mut JsRouteSnapper, from: Waypoint, to: Waypoint) {
    must_mouseover_waypt(snapper, from);
    snapper.on_drag_start();
    optionally_mouseover_waypt(snapper, to);
    snapper.on_mouse_up();
}
