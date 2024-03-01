use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use geojson_to_route_snapper::convert_geojson;

#[derive(Parser)]
struct Args {
    /// Path to a .geojson file to convert
    #[arg(long)]
    input: String,

    /// Output file to write
    #[arg(long, default_value = "snap.bin")]
    output: String,
}

fn main() {
    let args = Args::parse();
    let snapper = convert_geojson(std::fs::read_to_string(&args.input).unwrap()).unwrap();

    let output = BufWriter::new(File::create(args.output).unwrap());
    bincode::serialize_into(output, &snapper).unwrap();
}
