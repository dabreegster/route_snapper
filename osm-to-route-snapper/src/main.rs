use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use osm_to_route_snapper::convert_osm;

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
    let snapper = convert_osm(
        std::fs::read_to_string(args.input_osm).unwrap(),
        args.boundary
            .map(|path| std::fs::read_to_string(path).unwrap()),
    );

    let output = BufWriter::new(File::create(args.output).unwrap());
    bincode::serialize_into(output, &snapper).unwrap();
}
