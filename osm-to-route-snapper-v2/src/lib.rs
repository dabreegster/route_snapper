use std::collections::HashMap;

use anyhow::Result;
use geom::{GPSBounds, LonLat, PolyLine};
use osm_reader::{Element, WayID};

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

/// Convert input OSM PBF or XML data into a RouteSnapperMap, extracting all highway center-lines.
///
/// Does no clipping -- assumes the input has already been clipped to a boundary.
pub fn convert_osm(input_bytes: Vec<u8>, road_names: bool) -> Result<RouteSnapperMap> {
    println!("Scraping OSM data");
    let (nodes, ways) = scrape_elements(&input_bytes, road_names)?;
    println!(
        "Got {} nodes and {} ways. Splitting into edges",
        nodes.len(),
        ways.len(),
    );
    Ok(split_edges(nodes, ways))
}

struct Way {
    name: Option<String>,
    nodes: Vec<osm_reader::NodeID>,
}

fn scrape_elements(
    input_bytes: &[u8],
    road_names: bool,
) -> Result<(HashMap<osm_reader::NodeID, LonLat>, HashMap<WayID, Way>)> {
    // Scrape every node ID -> LonLat
    let mut nodes = HashMap::new();
    // Scrape every routable road
    let mut ways = HashMap::new();

    osm_reader::parse(input_bytes, |elem| match elem {
        Element::Node { id, lon, lat, .. } => {
            nodes.insert(id, LonLat::new(lon, lat));
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
    nodes: HashMap<osm_reader::NodeID, LonLat>,
    ways: HashMap<WayID, Way>,
) -> RouteSnapperMap {
    let mut gps_bounds = GPSBounds::new();
    for pt in nodes.values() {
        gps_bounds.update(*pt);
    }

    let mut map = RouteSnapperMap {
        gps_bounds,
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
            pts.push(nodes[&node].to_pt(&map.gps_bounds));
            // Edges start/end at intersections between two ways. The endpoints of the way also
            // count as intersections.
            let is_endpoint =
                idx == 0 || idx == num_nodes - 1 || *node_counter.get(&node).unwrap() > 1;
            if is_endpoint && pts.len() > 1 {
                let next_id = NodeID(node_id_lookup.len() as u32);
                let node1_id = *node_id_lookup.entry(node1).or_insert_with(|| {
                    map.nodes.push(pts[0]);
                    next_id
                });
                let next_id = NodeID(node_id_lookup.len() as u32);
                let node2_id = *node_id_lookup.entry(node).or_insert_with(|| {
                    map.nodes.push(*pts.last().unwrap());
                    next_id
                });
                let geometry = PolyLine::unchecked_new(std::mem::take(&mut pts));
                let length = geometry.length();
                map.edges.push(Edge {
                    node1: node1_id,
                    node2: node2_id,
                    geometry,
                    length,
                    name: way.name.clone(),
                });

                // Start the next edge
                node1 = node;
                pts.push(nodes[&node].to_pt(&map.gps_bounds));
            }
        }
    }

    println!(
        "{} nodes and {} edges total",
        map.nodes.len(),
        map.edges.len()
    );
    map
}
