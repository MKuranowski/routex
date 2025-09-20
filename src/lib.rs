// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

//! Simple routing over [OpenStreetMap](https://www.openstreetmap.org/) data.
//!
//! It converts OSM data into a standard weighted directed graph representation,
//! and runs A* to find shortest paths between nodes. Interpretation of OSM data
//! is customizable via [profiles](crate::osm::Profile). Routex supports one-way streets,
//! access tags (on ways only) and turn restrictions.
//!
//! # Example
//!
//! ```no_run
//! let mut g = routex::Graph::new();
//! let osm_options = routex::osm::Options {
//!     profile: &routex::osm::CAR_PROFILE,
//!     file_format: routex::osm::FileFormat::Unknown,
//!     bbox: [0.0; 4],
//! };
//! routex::osm::add_features_from_file(
//!     &mut g,
//!     &osm_options,
//!     "path/to/monaco.osm.pbf",
//! ).expect("failed to load monaco.osm");
//!
//! let start_node = g.find_nearest_node(43.7384, 7.4246).unwrap();
//! let end_node = g.find_nearest_node(43.7478, 7.4323).unwrap();
//! let route = routex::find_route_without_turn_around(&g, start_node.id, end_node.id, routex::DEFAULT_STEP_LIMIT)
//!     .expect("failed to find route");
//!
//! println!("Route: {:?}", route);
//! ```

mod astar;
pub mod c;
mod distance;
mod graph;
mod kd;
pub mod osm;

pub use astar::{find_route, find_route_without_turn_around, AStarError, DEFAULT_STEP_LIMIT};
pub use distance::earth_distance;
pub use graph::Graph;
pub use kd::KDTree;

/// Represents an element of the [Graph].
///
/// Due to turn restriction processing, one OpenStreetMap node
/// may be represented by multiple Node instances. If that is the
/// case, a "canonical" node (not bound by any turn restrictions) will
/// have `id == osm_id`.
///
/// Nodes with `id == 0`, `osm_id == 0` or `osm_id >= 0x0008_0000_0000_0000`
/// are disallowed. Zero IDs are used by the C bindings to signify absence of nodes,
/// while large IDs are reserved for turn restriction processing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Node {
    pub id: i64,
    pub osm_id: i64,
    pub lat: f32,
    pub lon: f32,
}

impl Node {
    pub const ZERO: Self = Self {
        id: 0,
        osm_id: 0,
        lat: 0.0,
        lon: 0.0,
    };
}

/// Represents an outgoing (one-way) connection from a specific [Node].
///
/// `cost` must be greater than the crow-flies distance between the two nodes.
///
/// Due to implementation details, `to` might not exist in the [Graph].
/// Users must silently ignore such edges.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Edge {
    pub to: i64,
    pub cost: f32,
}
