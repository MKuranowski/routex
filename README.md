# routex

Simple routing over [OpenStreetMap](https://www.openstreetmap.org/) data.

It converts OSM data into a standard weighted directed graph representation,
and runs A* to find shortest paths between nodes. Interpretation of OSM data
is customizable via [profiles](crate::osm::Profile). Routex supports one-way streets,
access tags (on ways only) and turn restrictions.

## Usage

routex is written in [Rust](https://www.rust-lang.org/) and uses [Cargo](https://doc.rust-lang.org/cargo/) for dependency management and compilation.

### Rust

Add dependency with `cargo add routex`.

```rust
let mut g = routex::Graph::new();
let osm_options = routex::osm::Options {
    profile: &routex::osm::CAR_PROFILE,
    file_format: routex::osm::FileFormat::Unknown,
    bbox: [0.0; 4],
};
routex::osm::add_features_from_file(
    &mut g,
    &osm_options,
    "path/to/monaco.osm.pbf",
).expect("failed to load monaco.osm");

let start_node = g.find_nearest_node(43.7384, 7.4246).unwrap();
let end_node = g.find_nearest_node(43.7478, 7.4323).unwrap();
let route = routex::find_route_without_turn_around(&g, start_node.id, end_node.id, routex::DEFAULT_STEP_LIMIT)
    .expect("failed to find route");

println!("Route: {:?}", route);
```

## C

> ⚠️🏗️ C bindings are work-in-progress

The C interface is included in the <bindings/c/routex.h> header file.
`cargo build --release` compiles the static and shared library.
Compiled libraries are placed in `target/release`.

## C++

> ⚠️🏗️ C++ bindings are work-in-progress

### Python

> ⚠️🏗️ Python bindings are work-in-progress

## TODOs

- [x] Rust library
- [x] C bindings
    - [x] Graphs
    - [x] K-D Tree
    - [x] OSM
    - [x] Common Profiles
    - [x] Logging
- [ ] C++ bindings
- [ ] Python bindings
- [ ] CLI program

## License

routex is made available under the MIT license.
