use std::collections::HashMap;

use abstutil::Timer;
use geom::LonLat;

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

/// Convert input OSM XML data and a boundary GeoJSON string (with exactly one polygon) into a
/// RouteSnapperMap, which can then be serialized and used.
pub fn convert_osm(input_osm: String, boundary_geojson: Option<String>) -> RouteSnapperMap {
    let mut timer = Timer::new("convert OSM to route snapper graph");

    let (streets, _) = streets_reader::osm_to_street_network(
        &input_osm,
        boundary_geojson.map(|geojson| {
            let mut polygons = LonLat::parse_geojson_polygons(geojson).unwrap();
            if polygons.len() != 1 {
                panic!("boundary_geojson doesn't contain exactly one polygon");
            }
            polygons.pop().unwrap().0
        }),
        osm2streets::MapConfig::default(),
        &mut timer,
    )
    .unwrap();
    streets_to_snapper(&streets)
}

fn streets_to_snapper(streets: &osm2streets::StreetNetwork) -> RouteSnapperMap {
    let mut map = RouteSnapperMap {
        gps_bounds: streets.gps_bounds.clone(),
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let mut id_lookup = HashMap::new();
    for i in streets.intersections.values() {
        if i.roads.iter().all(|r| streets.roads[r].is_light_rail()) {
            continue;
        }

        // The intersection's calculated polygon might not match up with road reference lines.
        // Instead use an endpoint of any connecting road's reference line.
        let road = &streets.roads[&i.roads[0]];
        map.nodes.push(if road.src_i == i.id {
            road.reference_line.first_pt()
        } else {
            road.reference_line.last_pt()
        });

        id_lookup.insert(i.id, NodeID(id_lookup.len() as u32));
    }
    for r in streets.roads.values() {
        if r.is_light_rail() {
            continue;
        }
        map.edges.push(Edge {
            node1: id_lookup[&r.src_i],
            node2: id_lookup[&r.dst_i],
            geometry: r.reference_line.clone(),
            length: r.reference_line.length(),
        });
    }

    map
}

#[cfg(target_arch = "wasm32")]
use std::sync::Once;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
static START: Once = Once::new();

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen()]
pub fn convert(input_osm: String, boundary_geojson: String) -> Vec<u8> {
    START.call_once(|| {
        console_log::init_with_level(log::Level::Info).unwrap();
        console_error_panic_hook::set_once();
    });

    let snapper = convert_osm(input_osm, Some(boundary_geojson));
    bincode::serialize(&snapper).unwrap()
}
