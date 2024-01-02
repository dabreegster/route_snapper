use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use osm_to_route_snapper::convert_osm;

#[derive(Parser)]
struct Args {
    /// Path to a .osm.pbf or .xml file to convert
    #[arg(long)]
    input: String,

    /// Path to GeoJSON file with the boundary to clip the input to
    #[arg(short, long)]
    boundary: Option<String>,

    /// Output file to write
    #[arg(long, default_value = "snap.bin")]
    output: String,

    /// Omit road names from the output, saving some space.
    #[clap(long)]
    no_road_names: bool,
}

fn main() {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    let args = Args::parse();
    let snapper = convert_osm(
        std::fs::read(&args.input).unwrap(),
        args.boundary
            .map(|path| std::fs::read_to_string(path).unwrap()),
        !args.no_road_names,
    )
    .unwrap();

    let output = BufWriter::new(File::create(args.output).unwrap());
    bincode::serialize_into(output, &snapper).unwrap();
}
