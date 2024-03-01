use std::collections::HashMap;

use anyhow::Result;
use geo::{Coord, LineString};
use geojson::de::deserialize_geometry;
use serde::Deserialize;

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

/// Converts GeoJSON into a graph for use with the route snapper. The GeoJSON must only contain
/// LineStrings. Routing will only work if the LineStrings start and end at intersections between
/// other LineStrings. Each feature may have an optional `name` string property.
pub fn convert_geojson(input_string: String) -> Result<RouteSnapperMap> {
    let input: Vec<InputEdge> =
        geojson::de::deserialize_feature_collection_str_to_vec(&input_string)?;

    let mut map = RouteSnapperMap {
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let mut node_to_id: HashMap<(isize, isize), NodeID> = HashMap::new();

    for edge in input {
        let first_pt = *edge.geometry.coords().next().unwrap();
        let last_pt = *edge.geometry.coords().last().unwrap();

        for pt in [first_pt, last_pt] {
            let key = hashify_point(pt);
            if !node_to_id.contains_key(&key) {
                node_to_id.insert(key, NodeID(node_to_id.len() as u32));
                map.nodes.push(pt);
            }
        }

        map.edges.push(Edge {
            node1: node_to_id[&hashify_point(first_pt)],
            node2: node_to_id[&hashify_point(last_pt)],
            geometry: edge.geometry,
            length_meters: 0.0,
            name: edge.name,
        });
    }

    Ok(map)
}

#[derive(Deserialize)]
pub struct InputEdge {
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: LineString,
    name: Option<String>,
}

fn hashify_point(pt: Coord) -> (isize, isize) {
    ((pt.x * 1_000_000.0) as isize, (pt.y * 1_000_000.0) as isize)
}

#[cfg(target_arch = "wasm32")]
use std::sync::Once;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
static START: Once = Once::new();

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen()]
pub fn convert(input_string: String) -> Result<Vec<u8>, JsValue> {
    START.call_once(|| {
        console_error_panic_hook::set_once();
    });

    let snapper =
        convert_geojson(input_string).map_err(|err| JsValue::from_str(&err.to_string()))?;
    Ok(bincode::serialize(&snapper).unwrap())
}
