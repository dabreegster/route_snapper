use geo::{line_measures::LengthMeasurable, Haversine};
use geojson::{Feature, Geometry};
use route_snapper_graph::RouteSnapperMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Pass in a snap.bin file");
        std::process::exit(1);
    }
    let bytes = std::fs::read(&args[1]).unwrap();
    let mut map: RouteSnapperMap = bincode::deserialize(&bytes).unwrap();

    // TODO Move this to route_snapper_graph
    if !map.override_forward_costs.is_empty() && map.override_forward_costs.len() != map.edges.len()
    {
        panic!("override_forward_costs length doesn't match edges length",);
    }
    if !map.override_backward_costs.is_empty()
        && map.override_backward_costs.len() != map.edges.len()
    {
        panic!("override_backward_costs length doesn't match edges length",);
    }

    for (idx, edge) in map.edges.iter_mut().enumerate() {
        edge.length_meters = edge.geometry.length(&Haversine);

        if map.override_forward_costs.is_empty() {
            edge.forward_cost = Some(edge.length_meters);
        } else {
            edge.forward_cost = map.override_forward_costs[idx];
        }

        if map.override_backward_costs.is_empty() {
            edge.backward_cost = Some(edge.length_meters);
        } else {
            edge.backward_cost = map.override_backward_costs[idx];
        }
    }

    // This is a copy of renderGraph from the WASM API. Browsers seem to have limits for how large
    // a dynamically-generated file they can download. Sharing the code for this method without
    // bloating dependencies isn't straightforward.
    let mut features = Vec::new();
    for (idx, edge) in map.edges.iter().enumerate() {
        let mut f = Feature::from(Geometry::from(&edge.geometry));
        f.set_property("edge_id", idx);
        f.set_property("node1", edge.node1.0);
        f.set_property("node2", edge.node2.0);
        f.set_property("length_meters", edge.length_meters);
        f.set_property("forward_cost", edge.forward_cost);
        f.set_property("backward_cost", edge.backward_cost);
        f.set_property("name", edge.name.clone());
        features.push(f);
    }
    for (idx, pt) in map.nodes.iter().enumerate() {
        let mut f = Feature::from(Geometry::from(geojson::Value::Point(vec![pt.x, pt.y])));
        f.set_property("node_id", idx);
        features.push(f);
    }
    let gj = geojson::GeoJson::from(features.into_iter().collect::<geojson::FeatureCollection>());
    std::fs::write("debug.geojson", serde_json::to_string_pretty(&gj).unwrap()).unwrap();
}
