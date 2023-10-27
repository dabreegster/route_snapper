use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use osm_to_route_snapper_v2::convert_osm;

#[derive(Parser)]
struct Args {
    /// Path to a .osm.pbf file to convert
    #[arg(long)]
    input_pbf: String,

    /// Output file to write
    #[arg(long, default_value = "snap.bin")]
    output: String,

    /// Omit road names from the output, saving some space.
    #[clap(long)]
    no_road_names: bool,
}

fn main() {
    let args = Args::parse();
    let snapper = convert_osm(
        // TODO Read to bytes first, so we can be WASM friendly? Even though we won't be doing PBF
        // from the web?
        args.input_pbf,
        !args.no_road_names,
    )
    .unwrap();

    let output = BufWriter::new(File::create(args.output).unwrap());
    bincode::serialize_into(output, &snapper).unwrap();
}
