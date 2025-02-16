// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use crate::{earth_distance, Edge, Node};
use std::collections::btree_map::{BTreeMap, Entry};

/// Represents an OpenStreetMap network as a set of [Nodes](Node)
/// and [Edges](Edge) between them.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Graph(BTreeMap<i64, (Node, Vec<Edge>)>);

impl Graph {
    /// Returns the number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns an iterator over all [Nodes](Node) in the graph.
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.0.iter().map(|(_, (node, _))| node)
    }

    /// Retrieves a [Node] with the provided id.
    pub fn get_node(&self, id: i64) -> Option<Node> {
        self.0.get(&id).map(|&(node, _)| node)
    }

    /// Creates or updates a [Node] with `node.id`.
    ///
    /// All outgoing and incoming edges are preserved.
    /// Updating a [Node] position might result in violation of the
    /// [Edge] cost invariant (and thus break route finding) and
    /// is therefore disallowed.
    pub fn set_node(&mut self, node: Node) {
        assert_ne!(node.id, 0);

        match self.0.entry(node.id) {
            Entry::Vacant(e) => {
                e.insert((node, Vec::default()));
            }
            Entry::Occupied(mut e) => {
                debug_assert_eq!(e.get().0.id, node.id);
                e.get_mut().0 = node;
            }
        }
    }

    /// Deletes a [Node] with a given `id`.
    ///
    /// While all outgoing edges are removed, incoming edges are preserved
    /// (as this would require a walk over all nodes in the graph).
    /// Thus, deleting a node and then re-using its id might result in violation
    /// of the [Edge] cost invariant (and break route finding) is disallowed.
    pub fn delete_node(&mut self, id: i64) {
        self.0.remove(&id);
    }

    /// Finds the closest canonical (`id == osm_id`) [Node] to the given position.
    ///
    /// This function requires computing the distance to every [Node] in the graph,
    /// and is not suitable for large graphs.
    pub fn find_nearest_node(&self, lat: f32, lon: f32) -> Option<Node> {
        self.0
            .iter()
            .filter_map(|(_, &(nd, _))| {
                if nd.id == nd.osm_id {
                    Some((earth_distance(lat, lon, nd.lat, nd.lon), nd))
                } else {
                    None
                }
            })
            .min_by(|(a_dist, _), (b_dist, _)| a_dist.partial_cmp(b_dist).unwrap())
            .map(|(_, nd)| nd)
    }

    /// Gets all outgoing [Edges](Edge) from a node with a given id.
    pub fn get_edges(&self, from_id: i64) -> &[Edge] {
        self.0
            .get(&from_id)
            .map(|(_, e)| e.as_slice())
            .unwrap_or_default()
    }

    /// Gets the cost of an [Edge] from one node to another.
    /// If such an edge doesn't exist, returns [f32::INFINITY].
    pub fn get_edge(&self, from_id: i64, to_id: i64) -> f32 {
        self.0
            .get(&from_id)
            .map(|(_, e)| {
                e.iter().find_map(|edge| {
                    if edge.to == to_id {
                        Some(edge.cost)
                    } else {
                        None
                    }
                })
            })
            .flatten()
            .unwrap_or(f32::INFINITY)
    }

    /// Creates or updates an [Edge] from a node with a given id.
    pub fn set_edge(&mut self, from_id: i64, edge: Edge) {
        assert_ne!(from_id, 0);
        assert_ne!(edge.to, 0);

        if let Some((_, edges)) = self.0.get_mut(&from_id) {
            if let Some(candidate) = edges.iter_mut().find(|e| e.to == edge.to) {
                *candidate = edge;
            } else {
                edges.push(edge);
            }
        }
    }

    /// Removes an edge from one node to another.
    pub fn delete_edge(&mut self, from_id: i64, to_id: i64) {
        if let Some((_, edges)) = self.0.get_mut(&from_id) {
            if let Some(idx) =
                edges.iter().enumerate().find_map(
                    |(idx, edge)| {
                        if edge.to == to_id {
                            Some(idx)
                        } else {
                            None
                        }
                    },
                )
            {
                edges.swap_remove(idx);
            }
        }
    }
}
