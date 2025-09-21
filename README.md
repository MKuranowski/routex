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
pub fn main() {
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
}
```

## C

The C interface is included in the <bindings/c/routex.h> header file.
`cargo build --release` compiles the static and shared library.
Compiled libraries are placed in `target/release`.

```c
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <routex.h>

void log_handler(void* f, int level, char const* target, char const* message) {
    (void)f; // unused
    char const* level_str = "";
    if (level >= 50) level_str = "CRITICAL";
    else if (level >= 40) level_str = "ERROR";
    else if (level >= 30) level_str = "WARNING";
    else if (level >= 20) level_str = "INFO";
    else if (level >= 10) level_str = "DEBUG";
    else level_str = "TRACE";
    fprintf(stderr, "[%s] %s: %s\n", level_str, target, message);
}


int main(void) {
    int status = 1;
    RoutexGraph* graph = NULL;
    RoutexRouteResult result = {0};

    // Set logging handler to show any errors
    routex_set_logging_callback(log_handler, NULL, NULL, 30);

    // Create a graph and load data into it
    graph = routex_graph_new();
    RoutexOsmOptions options = {
        .profile = ROUTEX_OSM_PROFILE_CAR,
        .file_format = RoutexOsmFormatUnknown,
        .bbox = {0},
    };
    if (!routex_graph_add_from_osm_file(graph, &options, "path/to/monaco.osm.pbf")) goto cleanup;

    // Find the start and end nodes
    RoutexNode start_node = routex_graph_find_nearest_node(graph, 43.7384, 7.4246);
    RoutexNode end_node = routex_graph_find_nearest_node(graph, 43.7478, 7.4323);

    // Find the route
    result = routex_find_route(graph, start_node.id, end_node.id, ROUTEX_DEFAULT_STEP_LIMIT);

    // Print the route or any error
    switch (result.type) {
    case RoutexRouteResultTypeOk:
        for (uint32_t i = 0; i < result.as_ok.len; ++i) {
            RoutexNode node = routex_graph_get_node(graph, result.as_ok.nodes[i]);
            printf("%f %f\n", node.lat, node.lon);
        }
        status = 0; // success
        break;

    case RoutexRouteResultTypeInvalidReference:
        fprintf(stderr, "[ERROR] find_route: invalid node reference to %d\n", result.as_invalid_reference.invalid_node_id);
        break;

    case RoutexRouteResultTypeStepLimitExceeded:
        fprintf(stderr, "[ERROR] find_route: step limit exceeded while searching for route\n");
        break;
    }

    // Free used memory
   cleanup:
    routex_route_result_delete(result);
    routex_graph_delete(graph);
    return status;
}
```

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
