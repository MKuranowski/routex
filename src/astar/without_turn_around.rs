// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::{BinaryHeap, HashMap};

use crate::{earth_distance, AStarError, Edge, Graph};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeAndBefore {
    node_id: i64,
    before_osm_id: i64,
}

#[derive(Debug, Clone, Copy)]
struct CameFromQueueItem {
    at: NodeAndBefore,
    osm_id: i64,
    cost: f32,
    score: f32,
}

impl PartialEq for CameFromQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.score.eq(&other.score)
    }

    fn ne(&self, other: &Self) -> bool {
        self.score.ne(&other.score)
    }
}

impl PartialOrd for CameFromQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // NOTE: We revert the order of comparison,
        // as lower scores are considered better ("higher"),
        // and Rust's BinaryHeap is a max-heap.
        other.score.partial_cmp(&self.score)
    }
}

impl Eq for CameFromQueueItem {}

impl Ord for CameFromQueueItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.partial_cmp(self).unwrap()
    }
}

fn reconstruct_came_from_path(
    came_from: &HashMap<NodeAndBefore, NodeAndBefore>,
    mut last: NodeAndBefore,
) -> Vec<i64> {
    let mut path = vec![last.node_id];

    while let Some(&nd) = came_from.get(&last) {
        path.push(nd.node_id);
        last = nd;
    }

    path.reverse();
    return path;
}

/// Uses the [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm)
/// to find the shortest route between two points in the provided graph.
///
/// Returns an empty list if there is no route between the two points.
///
/// For graphs without turn restrictions, use [find_route] as it runs faster.
/// This function has an extra dimension - it needs to not only consider the current node,
/// but also what was the previous node to prevent A-B-A immediate turnaround instructions.
///
/// `step_limit` limits how many nodes may be expanded during the search
/// before returning [AStarError::StepLimitExceeded]. Concluding that no route exists requires
/// expanding all nodes accessible from the start, which is usually very time-consuming,
/// especially on large datasets (like the whole planet). The recommended value is
/// [DEFAULT_STEP_LIMIT](crate::DEFAULT_STEP_LIMIT).
pub fn find_route_without_turn_around(
    g: &Graph,
    from_id: i64,
    to_id: i64,
    step_limit: usize,
) -> Result<Vec<i64>, AStarError> {
    assert_ne!(from_id, 0);
    assert_ne!(to_id, 0);

    let mut queue: BinaryHeap<CameFromQueueItem> = BinaryHeap::default();
    let mut came_from: HashMap<NodeAndBefore, NodeAndBefore> = HashMap::default();
    let mut known_costs: HashMap<NodeAndBefore, f32> = HashMap::default();
    let mut steps: usize = 0;

    let to_node = g
        .get_node(to_id)
        .ok_or(AStarError::InvalidReference(to_id))?;

    {
        let initial_at = NodeAndBefore {
            node_id: from_id,
            before_osm_id: 0,
        };

        let from_node = g
            .get_node(from_id)
            .ok_or(AStarError::InvalidReference(from_id))?;

        let initial_distance =
            earth_distance(from_node.lat, from_node.lon, to_node.lat, to_node.lon);

        queue.push(CameFromQueueItem {
            at: initial_at,
            osm_id: from_node.osm_id,
            cost: 0.0,
            score: initial_distance,
        });
        known_costs.insert(initial_at, 0.0);
    }

    while let Some(item) = queue.pop() {
        if item.at.node_id == to_id {
            return Ok(reconstruct_came_from_path(&came_from, item.at));
        }

        // Contrary to the wikipedia definition, we might keep multiple items in the queue for the same node.
        if item.cost > known_costs.get(&item.at).cloned().unwrap_or(f32::INFINITY) {
            continue;
        }

        steps += 1;
        if steps > step_limit {
            return Err(AStarError::StepLimitExceeded);
        }

        for &Edge {
            to: neighbor_id,
            cost: edge_cost,
        } in g.get_edges(item.at.node_id)
        {
            assert_ne!(neighbor_id, 0);

            // Check if the referred node exists
            if let Some(neighbor) = g.get_node(neighbor_id) {
                // Forbid turnarounds (A-B-A)
                if neighbor.osm_id == item.at.before_osm_id {
                    continue;
                }

                let neighbor_at = NodeAndBefore {
                    node_id: neighbor_id,
                    before_osm_id: item.osm_id,
                };

                // Check if this is the cheapest way to the neighbor
                let neighbor_cost = item.cost + edge_cost;
                if neighbor_cost
                    > known_costs
                        .get(&neighbor_at)
                        .cloned()
                        .unwrap_or(f32::INFINITY)
                {
                    continue;
                }

                // Push the new item into the queue
                came_from.insert(neighbor_at, item.at);
                known_costs.insert(neighbor_at, neighbor_cost);
                queue.push(CameFromQueueItem {
                    at: neighbor_at,
                    osm_id: neighbor.osm_id,
                    cost: neighbor_cost,
                    score: neighbor_cost
                        + earth_distance(neighbor.lat, neighbor.lon, to_node.lat, to_node.lon),
                });
            }
        }
    }

    return Ok(vec![]);
}
