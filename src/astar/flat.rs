// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::{BinaryHeap, HashMap};

use crate::{earth_distance, AStarError, Edge, Graph};

#[derive(Debug, Clone, Copy)]
struct FlatQueueItem {
    at: i64,
    cost: f32,
    score: f32,
}

impl PartialEq for FlatQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.score.eq(&other.score)
    }

    fn ne(&self, other: &Self) -> bool {
        self.score.ne(&other.score)
    }
}

impl PartialOrd for FlatQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // NOTE: We revert the order of comparison,
        // as lower scores are considered better ("higher"),
        // and Rust's BinaryHeap is a max-heap.
        other.score.partial_cmp(&self.score)
    }
}

impl Eq for FlatQueueItem {}

impl Ord for FlatQueueItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.partial_cmp(self).unwrap()
    }
}

fn reconstruct_flat_path(came_from: &HashMap<i64, i64>, mut last: i64) -> Vec<i64> {
    let mut path = vec![last];

    while let Some(&nd) = came_from.get(&last) {
        path.push(nd);
        last = nd;
    }

    path.reverse();
    return path;
}

/// Uses the [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm)
/// to find the shortest route between two nodes in the provided graph.
///
/// Returns an empty vector if there is no route between the two nodes.
///
/// For graphs with turn restrictions, use [find_route_without_turn_around](super::find_route_without_turn_around),
/// as this implementation will generate instructions with immediate turnarounds
/// (A-B-A) to circumvent any restrictions.
///
/// `step_limit` limits how many nodes may be expanded during the search
/// before returning [AStarError::StepLimitExceeded]. Concluding that no route exists requires
/// expanding all nodes accessible from the start, which is usually very time-consuming,
/// especially on large datasets (like the whole planet). The recommended value is
/// [DEFAULT_STEP_LIMIT](crate::DEFAULT_STEP_LIMIT).
pub fn find_route(
    g: &Graph,
    from_id: i64,
    to_id: i64,
    step_limit: usize,
) -> Result<Vec<i64>, AStarError> {
    assert_ne!(from_id, 0);
    assert_ne!(to_id, 0);

    let mut queue: BinaryHeap<FlatQueueItem> = BinaryHeap::default();
    let mut came_from: HashMap<i64, i64> = HashMap::default();
    let mut known_costs: HashMap<i64, f32> = HashMap::default();
    let mut steps: usize = 0;

    let to_node = g
        .get_node(to_id)
        .ok_or(AStarError::InvalidReference(to_id))?;

    {
        let from_node = g
            .get_node(from_id)
            .ok_or(AStarError::InvalidReference(from_id))?;

        let initial_distance =
            earth_distance(from_node.lat, from_node.lon, to_node.lat, to_node.lon);

        queue.push(FlatQueueItem {
            at: from_id,
            cost: 0.0,
            score: initial_distance,
        });
        known_costs.insert(from_id, 0.0);
    }

    while let Some(item) = queue.pop() {
        if item.at == to_id {
            return Ok(reconstruct_flat_path(&came_from, to_id));
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
        } in g.get_edges(item.at)
        {
            assert_ne!(neighbor_id, 0);

            // Check if the referred node exists
            if let Some(neighbor) = g.get_node(neighbor_id) {
                // Check if this is the cheapest way to the neighbor
                let neighbor_cost = item.cost + edge_cost;
                if neighbor_cost
                    > known_costs
                        .get(&neighbor_id)
                        .cloned()
                        .unwrap_or(f32::INFINITY)
                {
                    continue;
                }

                // Push the new item into the queue
                came_from.insert(neighbor_id, item.at);
                known_costs.insert(neighbor_id, neighbor_cost);
                queue.push(FlatQueueItem {
                    at: neighbor_id,
                    cost: neighbor_cost,
                    score: neighbor_cost
                        + earth_distance(neighbor.lat, neighbor.lon, to_node.lat, to_node.lon),
                });
            }
        }
    }

    return Ok(vec![]);
}
