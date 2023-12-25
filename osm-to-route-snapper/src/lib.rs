use std::collections::HashMap;

use anyhow::Result;
use geo::{
    BooleanOps, Contains, Coord, HaversineLength, Intersects, LineString, MultiLineString, Polygon,
};
use log::info;
use osm_reader::{Element, WayID};

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

/// Convert input OSM PBF or XML data into a RouteSnapperMap, extracting all highway center-lines.
/// If a boundary polygon is specified, clips roads to this boundary.
pub fn convert_osm(
    input_bytes: Vec<u8>,
    boundary_gj: Option<String>,
    road_names: bool,
) -> Result<RouteSnapperMap> {
    info!("Scraping OSM data");
    let (nodes, ways) = scrape_elements(&input_bytes, road_names)?;
    info!(
        "Got {} nodes and {} ways. Splitting into edges",
        nodes.len(),
        ways.len(),
    );

    let mut boundary = None;
    if let Some(gj_string) = boundary_gj {
        let gj: geojson::Feature = gj_string.parse()?;
        let boundary_geo: Polygon = gj.try_into()?;
        boundary = Some(boundary_geo);
    }

    let mut map = split_edges(nodes, ways, boundary.as_ref());
    if let Some(boundary) = boundary {
        clip(&mut map, boundary);
    }
    Ok(map)
}

struct Way {
    name: Option<String>,
    nodes: Vec<osm_reader::NodeID>,
}

fn scrape_elements(
    input_bytes: &[u8],
    road_names: bool,
) -> Result<(HashMap<osm_reader::NodeID, Coord>, HashMap<WayID, Way>)> {
    // Scrape every node ID -> Coord
    let mut nodes = HashMap::new();
    // Scrape every routable road
    let mut ways = HashMap::new();

    osm_reader::parse(input_bytes, |elem| match elem {
        Element::Node { id, lon, lat, .. } => {
            nodes.insert(id, Coord { x: lon, y: lat });
        }
        Element::Way { id, node_ids, tags } => {
            if tags.contains_key("highway") {
                // TODO When the name is missing, we could fallback on other OSM tags. See
                // map_model::Road::get_name in A/B Street.
                let name = if road_names {
                    tags.get("name").map(|x| x.to_string())
                } else {
                    None
                };
                ways.insert(
                    id,
                    Way {
                        name,
                        nodes: node_ids,
                    },
                );
            }
        }
        Element::Relation { .. } => {}
    })?;

    Ok((nodes, ways))
}

fn split_edges(
    nodes: HashMap<osm_reader::NodeID, Coord>,
    ways: HashMap<WayID, Way>,
    boundary: Option<&Polygon>,
) -> RouteSnapperMap {
    let mut map = RouteSnapperMap {
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    // Count how many ways reference each node
    let mut node_counter: HashMap<osm_reader::NodeID, usize> = HashMap::new();
    for way in ways.values() {
        for node in &way.nodes {
            *node_counter.entry(*node).or_insert(0) += 1;
        }
    }

    // Split each way into edges
    let mut node_id_lookup = HashMap::new();
    for way in ways.into_values() {
        let mut node1 = way.nodes[0];
        let mut pts = Vec::new();

        let num_nodes = way.nodes.len();
        for (idx, node) in way.nodes.into_iter().enumerate() {
            pts.push(nodes[&node]);
            // Edges start/end at intersections between two ways. The endpoints of the way also
            // count as intersections.
            let is_endpoint =
                idx == 0 || idx == num_nodes - 1 || *node_counter.get(&node).unwrap() > 1;
            if is_endpoint && pts.len() > 1 {
                let geometry = LineString::new(std::mem::take(&mut pts));
                let mut add_road = true;
                if let Some(boundary) = boundary {
                    // If this road doesn't intersect the boundary at all, skip it
                    if !boundary.contains(&geometry) && !boundary.exterior().intersects(&geometry) {
                        add_road = false;
                    }
                }

                if add_road {
                    let next_id = NodeID(node_id_lookup.len() as u32);
                    let node1_id = *node_id_lookup.entry(node1).or_insert_with(|| {
                        map.nodes.push(geometry.0[0]);
                        next_id
                    });
                    let next_id = NodeID(node_id_lookup.len() as u32);
                    let node2_id = *node_id_lookup.entry(node).or_insert_with(|| {
                        map.nodes.push(*geometry.0.last().unwrap());
                        next_id
                    });
                    let length_meters = geometry.haversine_length();
                    map.edges.push(Edge {
                        node1: node1_id,
                        node2: node2_id,
                        geometry,
                        length_meters,
                        name: way.name.clone(),
                    });
                }

                // Start the next edge
                node1 = node;
                pts.push(nodes[&node]);
            }
        }
    }

    info!(
        "{} nodes and {} edges total",
        map.nodes.len(),
        map.edges.len()
    );
    map
}

fn clip(map: &mut RouteSnapperMap, boundary: Polygon) {
    // If we have edges totally out-of-bounds, that's harder to clean up

    for edge in &mut map.edges {
        if boundary.exterior().intersects(&edge.geometry) {
            let invert = false;
            let mut multi_line_string =
                boundary.clip(&MultiLineString::from(edge.geometry.clone()), invert);
            // If we have multiple pieces, that's hard to deal with
            info!(
                "Shortening {:?} from {} to {}",
                edge.name,
                edge.geometry.haversine_length(),
                multi_line_string.0[0].haversine_length()
            );
            edge.geometry = multi_line_string.0.remove(0);
            // Fix both nodes; if there's no change, doesn't matter
            map.nodes[edge.node1.0 as usize] = *edge.geometry.coords().next().unwrap();
            map.nodes[edge.node2.0 as usize] = *edge.geometry.coords().next_back().unwrap();
        }
    }
}

#[cfg(target_arch = "wasm32")]
use std::sync::Once;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
static START: Once = Once::new();

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen()]
pub fn convert(input_bytes: Vec<u8>, boundary_geojson: String) -> Result<Vec<u8>, JsValue> {
    START.call_once(|| {
        console_log::init_with_level(log::Level::Info).unwrap();
        console_error_panic_hook::set_once();
    });

    let road_names = true;
    let snapper = convert_osm(input_bytes, Some(boundary_geojson), road_names)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    Ok(bincode::serialize(&snapper).unwrap())
}
