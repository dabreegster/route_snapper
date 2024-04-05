use std::collections::HashMap;

use anyhow::{bail, Result};
use geo::{Coord, CoordsIter, LineString};
use geojson::de::deserialize_geometry;
use serde::Deserialize;

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

/// Converts GeoJSON into a graph for use with the route snapper. See the user guide for
/// requirements about the GeoJSON file.
pub fn convert_geojson(input_string: String) -> Result<RouteSnapperMap> {
    let input: Vec<InputEdge> =
        geojson::de::deserialize_feature_collection_str_to_vec(&input_string)?;

    let mut map = RouteSnapperMap {
        nodes: Vec::new(),
        edges: Vec::new(),
        override_forward_costs: Vec::new(),
        override_backward_costs: Vec::new(),
    };

    // Count how many lines reference each point
    let mut point_counter: HashMap<(isize, isize), usize> = HashMap::new();
    for edge in &input {
        for pt in edge.geometry.coords() {
            *point_counter.entry(hashify_point(*pt)).or_insert(0) += 1;
        }
    }

    // Split each LineString into edges
    let mut node_id_lookup: HashMap<(isize, isize), NodeID> = HashMap::new();
    for edge in input {
        let mut point1 = *edge.geometry.coords().next().unwrap();
        let mut pts = Vec::new();

        let num_points = edge.geometry.coords_count();
        for (idx, pt) in edge.geometry.into_inner().into_iter().enumerate() {
            pts.push(pt);
            // Edges start/end at intersections between two LineStrings. The endpoints of the
            // LineString also count as intersections.
            let is_endpoint = idx == 0
                || idx == num_points - 1
                || *point_counter.get(&hashify_point(pt)).unwrap() > 1;
            if is_endpoint && pts.len() > 1 {
                let geometry = LineString::new(std::mem::take(&mut pts));

                let next_id = NodeID(node_id_lookup.len() as u32);
                let node1_id = *node_id_lookup
                    .entry(hashify_point(point1))
                    .or_insert_with(|| {
                        map.nodes.push(geometry.0[0]);
                        next_id
                    });
                let next_id = NodeID(node_id_lookup.len() as u32);
                let node2_id = *node_id_lookup.entry(hashify_point(pt)).or_insert_with(|| {
                    map.nodes.push(*geometry.0.last().unwrap());
                    next_id
                });
                map.edges.push(Edge {
                    node1: node1_id,
                    node2: node2_id,
                    geometry,
                    name: edge.name.clone(),

                    length_meters: 0.0,
                    forward_cost: None,
                    backward_cost: None,
                });
                map.override_forward_costs.push(edge.forward_cost);
                map.override_backward_costs.push(edge.backward_cost);
            }

            // Start the next edge
            point1 = pt;
            pts.push(pt);
        }
    }

    if map.override_forward_costs.iter().all(|x| x.is_none()) {
        bail!("No edges set forward_cost. The input is probably incorrect.");
    }
    if map.override_backward_costs.iter().all(|x| x.is_none()) {
        bail!("No edges set backward_cost. The input is probably incorrect.");
    }

    Ok(map)
}

#[derive(Deserialize)]
pub struct InputEdge {
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: LineString,
    name: Option<String>,
    forward_cost: Option<f64>,
    backward_cost: Option<f64>,
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
