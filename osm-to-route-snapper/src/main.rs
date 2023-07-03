use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

use abstutil::Timer;
use clap::Parser;
use geom::LonLat;

use route_snapper_graph::{Edge, NodeID, RouteSnapperMap};

#[derive(Parser)]
struct Args {
    /// Path to a .osm.xml file to convert
    #[arg(short, long)]
    input_osm: String,

    /// Path to GeoJSON file with the boundary to clip the input to
    #[arg(short, long)]
    boundary: Option<String>,

    /// Output file to write
    #[arg(short, long, default_value = "snap.bin")]
    output: String,
}

fn main() {
    let args = Args::parse();

    let mut timer = Timer::new("convert OSM to route snapper graph");

    let (streets, _) = streets_reader::osm_to_street_network(
        &std::fs::read_to_string(args.input_osm).unwrap(),
        args.boundary
            .map(|path| LonLat::read_geojson_polygon(&path).unwrap()),
        osm2streets::MapConfig::default(),
        &mut timer,
    )
    .unwrap();
    let snapper = streets_to_snapper(&streets);

    let output = BufWriter::new(File::create(args.output).unwrap());
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
