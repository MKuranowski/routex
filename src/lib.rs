// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

mod distance;
mod graph;

pub use distance::earth_distance;
pub use graph::Graph;

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
pub struct Node {
    pub id: i64,
    pub osm_id: i64,
    pub lat: f32,
    pub lon: f32,
}

/// Represents an outgoing (one-way) connection from a specific [Node].
///
/// `cost` must be greater than the crow-flies distance between the two nodes.
///
/// Due to implementation details, `to` might not exist in the [Graph].
/// Users must silently ignore such edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: i64,
    pub cost: f32,
}
