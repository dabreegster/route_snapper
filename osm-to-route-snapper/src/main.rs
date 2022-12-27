use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

use abstutil::Timer;
use geom::LonLat;

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

fn main() {
    let mut timer = Timer::new("convert OSM to route snapper graph");

    let osm_filename = std::env::args().nth(1).expect("no osm filename provided");
    let geojson_filename = std::env::args()
        .nth(2)
        .expect("no geojson filename provided");
    let (streets, _) = streets_reader::osm_to_street_network(
        &std::fs::read_to_string(osm_filename).unwrap(),
        Some(LonLat::read_geojson_polygon(&geojson_filename).unwrap()),
        osm2streets::MapConfig::default(),
        &mut timer,
    )
    .unwrap();
    let snapper = streets_to_snapper(&streets);

    let output = BufWriter::new(
        File::create(format!("{}.bin", abstutil::basename(geojson_filename))).unwrap(),
    );
    bincode::serialize_into(output, &snapper).unwrap();
}

fn streets_to_snapper(streets: &osm2streets::StreetNetwork) -> RouteSnapperMap {
    let mut map = RouteSnapperMap {
        gps_bounds: streets.gps_bounds.clone(),
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    let mut id_lookup = HashMap::new();
    for i in streets.intersections.values() {
        map.nodes.push(i.point);
        id_lookup.insert(i.id, NodeID(id_lookup.len() as u32));
    }
    for r in streets.roads.values() {
        map.edges.push(Edge {
            node1: id_lookup[&r.src_i],
            node2: id_lookup[&r.dst_i],
            geometry: r.reference_line.clone(),
            length: r.reference_line.length(),
        });
    }

    map
}
