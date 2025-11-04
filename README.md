# routx

Simple routing over [OpenStreetMap](https://www.openstreetmap.org/) data.

It converts OSM data into a standard weighted directed graph representation,
and runs A* to find shortest paths between nodes. Interpretation of OSM data
is customizable via [profiles](crate::osm::Profile). Routx supports one-way streets,
access tags (on ways only) and turn restrictions.

## TODOs

- [x] Rust library
- [x] C bindings
    - [x] Graphs
    - [x] K-D Tree
    - [x] OSM
    - [x] Common Profiles
    - [x] Logging
- [x] C++ bindings
- [ ] Python bindings
- [ ] CLI program

## Usage

routx is written in [Rust](https://www.rust-lang.org/) and uses [Cargo](https://doc.rust-lang.org/cargo/) for dependency management and compilation.

### Rust

Add dependency with `cargo add routx`.

```rust
pub fn main() {
    let mut g = routx::Graph::new();
    let osm_options = routx::osm::Options {
        profile: &routx::osm::CAR_PROFILE,
        file_format: routx::osm::FileFormat::Unknown,
        bbox: [0.0; 4],
    };
    routx::osm::add_features_from_file(
        &mut g,
        &osm_options,
        "path/to/monaco.osm.pbf",
    ).expect("failed to load monaco.osm");

    let start_node = g.find_nearest_node(43.7384, 7.4246).unwrap();
    let end_node = g.find_nearest_node(43.7478, 7.4323).unwrap();
    let route = routx::find_route_without_turn_around(&g, start_node.id, end_node.id, routx::DEFAULT_STEP_LIMIT)
        .expect("failed to find route");

    println!("Route: {:?}", route);
}
```

### C/C++

The C interface is included in the <bindings/include/routx.h> header file.
The C++ OOP interface builds on top of that and is included in the <bindings/include/routx.hpp> header.
C++20 is required as the bindings make use of [std::span](https://en.cppreference.com/w/cpp/container/span.html).

`cargo build --release` compiles the static and shared library. Compiled libraries are placed in `target/release`.

For prototyping it might be easier to simply download a compiled static library and headers
from [GitHub Releases](https://github.com/mkuranowski/routx/releases) and simply `cc -o main main.c routx.a`.

A Meson wrapper (which simply calls into cargo) is provided to make it easier for C/C++
projects to use routx with the help of [meson subprojects](https://mesonbuild.com/Subprojects.html)
and [meson wraps](https://mesonbuild.com/Wrap-dependency-system-manual.html).
Add the wrap file from below as `subprojects/routx.wrap` and get the [dependency object](https://mesonbuild.com/Reference-manual_returned_dep.html)
with `routx_dep = dependency('routx', fallback: ['routx', 'routx_dep'])`.

In principle, any other build system that can execute `cargo` and copy the files can be used.
Consult the manual for your build system on how to do that.

<details>
<summary>Example C program</summary>

```c
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <routx.h>

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
    RoutxGraph* graph = NULL;
    RoutxRouteResult result = {0};

    // Set logging handler to show any errors
    routx_set_logging_callback(log_handler, NULL, NULL, 30);

    // Create a graph and load data into it
    graph = routx_graph_new();
    RoutxOsmOptions options = {
        .profile = ROUTX_OSM_PROFILE_CAR,
        .file_format = RoutxOsmFormatUnknown,
        .bbox = {0},
    };
    if (!routx_graph_add_from_osm_file(graph, &options, "path/to/monaco.osm.pbf")) goto cleanup;

    // Find the start and end nodes
    RoutxNode start_node = routx_graph_find_nearest_node(graph, 43.7384, 7.4246);
    RoutxNode end_node = routx_graph_find_nearest_node(graph, 43.7478, 7.4323);

    // Find the route
    result = routx_find_route(graph, start_node.id, end_node.id, ROUTX_DEFAULT_STEP_LIMIT);

    // Print the route or any error
    switch (result.type) {
    case RoutxRouteResultTypeOk:
        for (uint32_t i = 0; i < result.as_ok.len; ++i) {
            RoutxNode node = routx_graph_get_node(graph, result.as_ok.nodes[i]);
            printf("%f %f\n", node.lat, node.lon);
        }
        status = 0; // success
        break;

    case RoutxRouteResultTypeInvalidReference:
        fprintf(stderr, "[ERROR] find_route: invalid node reference to %d\n", result.as_invalid_reference.invalid_node_id);
        break;

    case RoutxRouteResultTypeStepLimitExceeded:
        fprintf(stderr, "[ERROR] find_route: step limit exceeded while searching for route\n");
        break;
    }

    // Free used memory
   cleanup:
    routx_route_result_delete(result);
    routx_graph_delete(graph);
    return status;
}
```
</details>

<details>
<summary>Example C++ program</summary>

```cpp
#include <routx.hpp>
#include <iostream>
#include <cstdint>

void log_handler([[maybe_unused]] void* f, int level, char const* target, char const* message) {
    char const* level_str = "";
    if (level >= 50) level_str = "CRITICAL";
    else if (level >= 40) level_str = "ERROR";
    else if (level >= 30) level_str = "WARNING";
    else if (level >= 20) level_str = "INFO";
    else if (level >= 10) level_str = "DEBUG";
    else level_str = "TRACE";
    std::cerr << '[' << level_str << "] " << target << ": " << message << std::endl;
}


int main(void) {
    // Set logging handler to show any errors
    routx::set_logging_callback(log_handler, nullptr, nullptr, 30);

    // Create a graph and load data into it
    routx::Graph g = {};
    routx::osm::Options options = {
        .profile = routx::osm::ProfileCar,
        .file_format = routx::osm::Format::RoutxOsmFormatUnknown,
        .bbox = {0},
    };
    g.add_from_osm_file(&options, "path/to/monaco.osm.pbf");

    // Find the start and end nodes
    routx::Node start_node = g.find_nearest_node(43.7384, 7.4246);
    routx::Node end_node = g.find_nearest_node(43.7478, 7.4323);

    // Find the route
    routx::Route route = g.find_route_without_turn_around(start_node.id, end_node.id);

    // Print the route
    for (int64_t node_id : route) {
        routx::Node node = g.get_node(node_id);
        std::cout << node.lat << ' ' << node.lon << '\n';
    }
}
```

</details>

<details>
<summary>Meson wrap file</summary>

```ini
[wrap-git]
url = https://github.com/mkuranowski/routx.git
revision = v1.0.1
depth = 1

[provides]
dependency_names = routx
```

</details>

### Python

> ⚠️🏗️ Python bindings are work-in-progress

## Cross-Compiling

It's recommended to use [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) and [cargo-xwin](https://github.com/rust-cross/cargo-xwin).

The Meson wrapper project also supports cross-compilation, through the use of 2 external properties:
- `cargo_build_command` - the command to use in place of `cargo build`. Example values include `cargo zigbuild`,
    `cargo xwin build` or `cross build`. It's processed using Python's [shlex.split](https://docs.python.org/3/library/shlex.html#shlex.split),
    and the first argument is assumed to be an executable, searched using [shutil.which](https://docs.python.org/3/library/shutil.html#shutil.which).
- `cargo_build_target` - if set, append a `--target ${cargo_build_target}` to the argument list when executing
    `${cargo_build_command}`. Causes the wrapper to search for the built library in `target/${cargo_build_target}/`
    instead of the usual `target/`.

## Release Checklist

Note that routx is supposed to use [semantic versioning](https://semver.org/).

1. Make sure the working directory is clean, all the tests and formatting checks pass.
2. Bump version numbers in Cargo.toml, meson.build and README.md. Commit that change and
    tag it with `vX.Y.Z`. Push that tag along the latest `main` to GitHub.
3. `cargo publish`
4. Cross-compile static and dynamic versions of the library for most common platforms (`./cross_compile.py`).
    Attach them as artifacts to a new GitHub release, along with the routx.h and routx.hpp headers.
5. Notify and prepare any out-of-tree bindings.

## License

routx is made available under the MIT license.
