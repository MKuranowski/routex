// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use crate::{earth_distance, Edge, Node};
use std::collections::btree_map::{BTreeMap, Entry};

/// Represents an OpenStreetMap network as a set of [Nodes](Node)
/// and [Edges](Edge) between them.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Graph(pub BTreeMap<i64, (Node, Vec<Edge>)>);

impl Graph {
    /// Returns the number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns an iterator over all [Nodes](Node) in the graph.
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.0.iter().map(|(_, (node, _))| node)
    }

    /// Creates a Graph from 2 separate iterators over [Nodes](Node)
    /// and [Edges](Edge).
    ///
    /// Example:
    ///
    /// ```rs
    /// let g = Graph::from_iter(
    ///     [Node { ... }, Node { ... }],
    ///     [(1, 2, 10.0), (2, 1, 10.0)],
    /// );
    /// ```
    pub fn from_iter<N, E>(nodes: N, edges: E) -> Self
    where
        N: IntoIterator<Item = Node>,
        E: IntoIterator<Item = (i64, i64, f32)>,
    {
        let mut g = Graph(BTreeMap::from_iter(
            nodes.into_iter().map(|n| (n.id, (n, vec![]))),
        ));

        edges.into_iter().for_each(|(from, to, cost)| {
            g.set_edge(from, Edge { to: to, cost });
        });

        return g;
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
    ///
    /// Returns `true` if an existing node was updated/overwritten,
    /// `false` if a new node was created.
    pub fn set_node(&mut self, node: Node) -> bool {
        assert_ne!(node.id, 0);

        match self.0.entry(node.id) {
            Entry::Vacant(e) => {
                e.insert((node, Vec::default()));
                false
            }

            Entry::Occupied(mut e) => {
                debug_assert_eq!(e.get().0.id, node.id);
                e.get_mut().0 = node;
                true
            }
        }
    }

    /// Deletes a [Node] with a given `id`.
    ///
    /// While all outgoing edges are removed, incoming edges are preserved
    /// (as this would require a walk over all nodes in the graph).
    /// Thus, deleting a node and then re-using its id might result in violation
    /// of the [Edge] cost invariant (and break route finding) is disallowed.
    ///
    /// Returns `true` if a node was deleted, `false` if no such node existed.
    pub fn delete_node(&mut self, id: i64) -> bool {
        self.0.remove(&id).is_some()
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
    ///
    /// Returns `true` if an existing edge was updated, `false` if a new edge was created.
    ///
    /// If `from_id` or `edge.to` doesn't exist in the graph, does nothing and returns `false`.
    pub fn set_edge(&mut self, from_id: i64, edge: Edge) -> bool {
        assert_ne!(from_id, 0);
        assert_ne!(edge.to, 0);

        if !self.0.contains_key(&edge.to) {
            return false;
        }

        if let Some((_, edges)) = self.0.get_mut(&from_id) {
            if let Some(candidate) = edges.iter_mut().find(|e| e.to == edge.to) {
                *candidate = edge;
                return true;
            } else {
                edges.push(edge);
            }
        }

        false
    }

    /// Removes an edge from one node to another. If no such edge exists, does nothing.
    ///
    /// Returns `true` if an edge was removed, `false` otherwise.
    pub fn delete_edge(&mut self, from_id: i64, to_id: i64) -> bool {
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
                return true;
            }
        }

        false
    }

    /// Replaces all edges from `dst` by cloning all edges outgoing from `src`.
    pub(crate) fn clone_edges(&mut self, dst: i64, src: i64) {
        // Don't clone if dst doesn't exist
        if !self.0.contains_key(&dst) {
            return;
        }

        // Get a clone of source edges
        let edges = if let Some((_, src_edges)) = self.0.get(&src) {
            src_edges.clone()
        } else {
            Vec::new()
        };

        // Overwrite dst edges
        if let Some((_, dst_edges)) = self.0.get_mut(&dst) {
            *dst_edges = edges;
        }
    }
}
