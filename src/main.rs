use std::error::Error;
use std::path::{Path, PathBuf};

use clap::Parser;
use routex;

#[derive(Debug, thiserror::Error)]
#[error("{0}: {1}")]
struct GraphLoadError(PathBuf, #[source] routex::osm::Error);

#[derive(Parser)]
struct Cli {
    /// The path to the OSM file
    osm_file: PathBuf,

    /// Latitude of the start point
    start_lat: f32,

    /// Longitude of the start point
    start_lon: f32,

    /// Latitude of the end point
    end_lat: f32,

    /// Longitude of the end point
    end_lon: f32,
}

pub fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let g = load_graph(&cli.osm_file)?;

    let start = g
        .find_nearest_node(cli.start_lat, cli.start_lon)
        .expect("no node corresponding to the given start position");

    let end = g
        .find_nearest_node(cli.end_lat, cli.end_lon)
        .expect("no node corresponding to the given end position");

    let route =
        routex::find_route_without_turn_around(&g, start.id, end.id, routex::DEFAULT_STEP_LIMIT)?;

    println!("{{");
    println!("  \"type\": \"FeatureCollection\",");
    println!("  \"features\": [");
    println!("    {{");
    println!("      \"type\": \"Feature\",");
    println!("      \"properties\": {{}},");

    println!("      \"geometry\": {{");
    println!("        \"type\": \"LineString\",");
    println!("        \"coordinates\": [");

    let mut nodes = route
        .iter()
        .map(|&node_id| g.get_node(node_id).unwrap())
        .peekable();
    while let Some(node) = nodes.next() {
        let suffix = if nodes.peek().is_some() { "," } else { "" };
        println!("          [{}, {}]{}", node.lon, node.lat, suffix);
    }

    println!("        ]");
    println!("      }}");
    println!("    }}");
    println!("  ]");
    println!("}}");

    Ok(())
}

fn load_graph<P: AsRef<Path>>(path: P) -> Result<routex::Graph, GraphLoadError> {
    let mut g = routex::Graph::default();
    let options = routex::osm::Options {
        profile: &routex::osm::CAR_PROFILE,
        file_format: routex::osm::FileFormat::Xml,
        bbox: [0.0; 4],
    };
    match routex::osm::add_features_from_file(&mut g, &options, path.as_ref()) {
        Ok(()) => Ok(g),
        Err(e) => Err(GraphLoadError(PathBuf::from(path.as_ref()), e)),
    }
}
