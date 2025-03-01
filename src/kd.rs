// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use crate::{earth_distance, Node};

/// KDTree implements the [k-d tree data structure](https://en.wikipedia.org/wiki/K-d_tree),
/// which can be used to speed up nearest-neighbor search for large datasets. Practice shows
/// that [crate::Graph::find_nearest_node] takes significantly more time than
/// [crate::find_route] when generating multiple routes with `routex`. A k-d tree
/// can help with that, trading memory usage for CPU time.
///
/// This implementation assumes euclidean geometry, even though the default distance function
/// used is [earth_distance]. This results in undefined behavior when points
/// are close to the ante meridian (180°/-180° longitude) or poles (90°/-90° latitude),
/// or when the data spans multiple continents.
#[derive(Debug, Clone)]
pub struct KDTree {
    pivot: Node,
    left: Option<Box<KDTree>>,
    right: Option<Box<KDTree>>,
}

impl KDTree {
    /// Finds the closest canonical (`id == osm_id`) [Node] to the given position.
    pub fn find_nearest_node(&self, lat: f32, lon: f32) -> Node {
        self.find_nearest_node_impl(lat, lon, false).0
    }

    fn find_nearest_node_impl(&self, lat: f32, lon: f32, lon_divides: bool) -> (Node, f32) {
        // Start by assuming that pivot is the closest
        let mut best = self.pivot;
        let mut best_dist = earth_distance(lat, lon, best.lat, best.lon);

        // Select which branch to recurse into first
        let first_left = if lon_divides {
            lon < best.lon
        } else {
            lat < best.lat
        };
        let (first, second) = if first_left {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        };

        // Recurse into the first branch
        if let Some(ref branch) = first {
            let (alt, alt_dist) = branch.find_nearest_node_impl(lat, lon, !lon_divides);
            if alt_dist < best_dist {
                best = alt;
                best_dist = alt_dist;
            }
        }

        // (Optionally) recurse into the second branch
        if let Some(ref branch) = second {
            // A closer node is possible in the second branch if and only if
            // the splitting axis is closer than the current best candidate.
            let (axis_lat, axis_lon) = if lon_divides {
                (lat, self.pivot.lon)
            } else {
                (self.pivot.lat, lon)
            };
            let dist_to_axis = earth_distance(lat, lon, axis_lat, axis_lon);

            if dist_to_axis < best_dist {
                let (alt, alt_dist) = branch.find_nearest_node_impl(lat, lon, !lon_divides);
                if alt_dist < best_dist {
                    best = alt;
                    best_dist = alt_dist;
                }
            }
        }

        return (best, best_dist);
    }

    /// Builds a k-d tree from an iterable of [Nodes](Node).
    /// Non-canonical (`id != osm_id`) nodes are skipped when building the tree.
    pub fn from_iter<I: IntoIterator<Item = Node>>(nodes: I) -> Option<Self> {
        let mut nodes = nodes
            .into_iter()
            .filter(|n| n.id == n.osm_id)
            .collect::<Vec<_>>();
        Self::build(nodes.as_mut_slice())
    }

    /// Builds a k-d tree from a mutable slice of [Nodes](Node). Nodes will be reordered
    /// in the slice to facility building the tree.
    ///
    /// The provided slice must only contain canonical (`id == osm_id`) nodes;
    /// this is checked with a `debug_assert!`.
    pub fn build(nodes: &mut [Node]) -> Option<Self> {
        debug_assert!(nodes.iter().all(|n| n.id == n.osm_id));
        Self::build_impl(nodes, false)
    }

    fn build_impl(nodes: &mut [Node], lon_divides: bool) -> Option<Self> {
        match nodes.len() {
            0 => None,
            1 => Some(Self {
                pivot: nodes[0],
                left: None,
                right: None,
            }),
            _ => {
                if lon_divides {
                    nodes.sort_by(|a, b| a.lon.partial_cmp(&b.lon).unwrap());
                } else {
                    nodes.sort_by(|a, b| a.lat.partial_cmp(&b.lat).unwrap());
                }
                let median = nodes.len() / 2;
                let pivot = nodes[median];
                let (left, right_and_pivot) = nodes.split_at_mut(median);
                let right = &mut right_and_pivot[1..];
                Some(Self {
                    pivot,
                    left: box_option(Self::build_impl(left, !lon_divides)),
                    right: box_option(Self::build_impl(right, !lon_divides)),
                })
            }
        }
    }
}

#[inline]
fn box_option<T>(o: Option<T>) -> Option<Box<T>> {
    o.map(|thing| Box::new(thing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kd_tree() {
        let tree = KDTree::build(&mut [
            Node {
                id: 1,
                osm_id: 1,
                lat: 0.01,
                lon: 0.01,
            },
            Node {
                id: 2,
                osm_id: 2,
                lat: 0.01,
                lon: 0.05,
            },
            Node {
                id: 3,
                osm_id: 3,
                lat: 0.03,
                lon: 0.09,
            },
            Node {
                id: 4,
                osm_id: 4,
                lat: 0.04,
                lon: 0.03,
            },
            Node {
                id: 5,
                osm_id: 5,
                lat: 0.04,
                lon: 0.07,
            },
            Node {
                id: 6,
                osm_id: 6,
                lat: 0.07,
                lon: 0.03,
            },
            Node {
                id: 7,
                osm_id: 7,
                lat: 0.07,
                lon: 0.01,
            },
            Node {
                id: 8,
                osm_id: 8,
                lat: 0.08,
                lon: 0.05,
            },
            Node {
                id: 9,
                osm_id: 9,
                lat: 0.08,
                lon: 0.09,
            },
        ])
        .expect("k-d tree from non-empty slice must not be empty");

        assert_eq!(tree.find_nearest_node(0.02, 0.02).id, 1);
        assert_eq!(tree.find_nearest_node(0.05, 0.03).id, 4);
        assert_eq!(tree.find_nearest_node(0.05, 0.08).id, 5);
        assert_eq!(tree.find_nearest_node(0.09, 0.06).id, 8);
    }
}
