// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

mod error;
mod flat;
mod without_turn_around;

pub use error::{AStarError, DEFAULT_STEP_LIMIT};
pub use flat::find_route;
pub use without_turn_around::find_route_without_turn_around;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph, Node};

    #[inline]
    fn simple_graph_fixture() -> Graph {
        //   200   200   200
        // 1─────2─────3─────4
        //       └─────5─────┘
        //         100    100
        Graph::from_iter(
            [
                Node {
                    id: 1,
                    osm_id: 1,
                    lat: 0.01,
                    lon: 0.01,
                },
                Node {
                    id: 2,
                    osm_id: 2,
                    lat: 0.02,
                    lon: 0.01,
                },
                Node {
                    id: 3,
                    osm_id: 3,
                    lat: 0.03,
                    lon: 0.01,
                },
                Node {
                    id: 4,
                    osm_id: 4,
                    lat: 0.04,
                    lon: 0.01,
                },
                Node {
                    id: 5,
                    osm_id: 5,
                    lat: 0.03,
                    lon: 0.00,
                },
            ],
            [
                (1, 2, 200.0),
                (2, 1, 200.0),
                (2, 3, 200.0),
                (2, 5, 100.0),
                (3, 2, 200.0),
                (3, 4, 200.0),
                (4, 3, 200.0),
                (4, 5, 100.0),
                (5, 2, 100.0),
                (5, 4, 100.0),
            ],
        )
    }

    #[test]
    fn simple() {
        let g = simple_graph_fixture();
        assert_eq!(find_route(&g, 1, 4, 100), Ok(vec![1_i64, 2, 5, 4]));
    }

    #[test]
    fn simple_without_turn_around() {
        let g = simple_graph_fixture();
        assert_eq!(
            find_route_without_turn_around(&g, 1, 4, 100),
            Ok(vec![1_i64, 2, 5, 4])
        );
    }

    #[test]
    fn step_limit() {
        let g = simple_graph_fixture();
        assert_eq!(find_route(&g, 1, 4, 2), Err(AStarError::StepLimitExceeded));
    }

    #[test]
    fn step_limit_without_turn_around() {
        let g = simple_graph_fixture();
        assert_eq!(
            find_route_without_turn_around(&g, 1, 4, 2),
            Err(AStarError::StepLimitExceeded)
        );
    }

    #[inline]
    fn shortest_not_optimal_fixture() -> Graph {
        //    500   100
        //  7─────8─────9
        //  │     │     │
        //  │400  │300  │100
        //  │ 200 │ 400 │
        //  4─────5─────6
        //  │     │     │
        //  │600  │500  │100
        //  │ 100 │ 200 │
        //  1─────2─────3
        Graph::from_iter(
            [
                Node {
                    id: 1,
                    osm_id: 1,
                    lat: 0.00,
                    lon: 0.00,
                },
                Node {
                    id: 2,
                    osm_id: 2,
                    lat: 0.01,
                    lon: 0.00,
                },
                Node {
                    id: 3,
                    osm_id: 3,
                    lat: 0.02,
                    lon: 0.00,
                },
                Node {
                    id: 4,
                    osm_id: 4,
                    lat: 0.00,
                    lon: 0.01,
                },
                Node {
                    id: 5,
                    osm_id: 5,
                    lat: 0.01,
                    lon: 0.01,
                },
                Node {
                    id: 6,
                    osm_id: 6,
                    lat: 0.02,
                    lon: 0.01,
                },
                Node {
                    id: 7,
                    osm_id: 7,
                    lat: 0.00,
                    lon: 0.02,
                },
                Node {
                    id: 8,
                    osm_id: 8,
                    lat: 0.01,
                    lon: 0.02,
                },
                Node {
                    id: 9,
                    osm_id: 9,
                    lat: 0.02,
                    lon: 0.02,
                },
            ],
            [
                (1, 2, 100.0),
                (1, 4, 600.0),
                (2, 1, 100.0),
                (2, 3, 200.0),
                (2, 5, 500.0),
                (3, 2, 200.0),
                (3, 6, 100.0),
                (4, 1, 600.0),
                (4, 5, 200.0),
                (4, 7, 400.0),
                (5, 2, 500.0),
                (5, 4, 200.0),
                (5, 6, 400.0),
                (5, 8, 300.0),
                (6, 3, 100.0),
                (6, 5, 400.0),
                (6, 9, 100.0),
                (7, 4, 400.0),
                (7, 8, 500.0),
                (8, 5, 300.0),
                (8, 7, 500.0),
                (8, 9, 100.0),
                (9, 6, 100.0),
                (9, 8, 100.0),
            ],
        )
    }

    #[test]
    fn shortest_not_optimal() {
        let g = shortest_not_optimal_fixture();
        assert_eq!(find_route(&g, 1, 8, 100), Ok(vec![1_i64, 2, 3, 6, 9, 8]));
    }

    #[test]
    fn shortest_not_optimal_without_turn_around() {
        let g = shortest_not_optimal_fixture();
        assert_eq!(
            find_route_without_turn_around(&g, 1, 8, 100),
            Ok(vec![1_i64, 2, 3, 6, 9, 8])
        );
    }

    #[inline]
    fn turn_restriction_fixture() -> Graph {
        // 1
        // │
        // │10
        // │ 10
        // 2─────4
        // │     │
        // │10   │100
        // │ 10  │
        // 3─────5
        // mandatory 1-2-4
        Graph::from_iter(
            [
                Node {
                    id: 1,
                    osm_id: 1,
                    lat: 0.00,
                    lon: 0.02,
                },
                Node {
                    id: 2,
                    osm_id: 2,
                    lat: 0.00,
                    lon: 0.01,
                },
                Node {
                    id: 20,
                    osm_id: 2,
                    lat: 0.00,
                    lon: 0.01,
                },
                Node {
                    id: 3,
                    osm_id: 3,
                    lat: 0.00,
                    lon: 0.00,
                },
                Node {
                    id: 4,
                    osm_id: 4,
                    lat: 0.01,
                    lon: 0.01,
                },
                Node {
                    id: 5,
                    osm_id: 5,
                    lat: 0.01,
                    lon: 0.00,
                },
            ],
            [
                (1, 20, 10.0),
                (2, 1, 10.0),
                (2, 3, 10.0),
                (2, 4, 10.0),
                (20, 4, 10.0),
                (3, 2, 10.0),
                (3, 5, 10.0),
                (4, 2, 10.0),
                (4, 5, 100.0),
                (5, 3, 10.0),
                (5, 4, 100.0),
            ],
        )
    }

    #[test]
    fn turn_restriction() {
        let g = turn_restriction_fixture();
        assert_eq!(find_route(&g, 1, 3, 100), Ok(vec![1_i64, 20, 4, 2, 3]));
    }

    #[test]
    fn turn_restriction_without_turn_around() {
        let g = turn_restriction_fixture();
        assert_eq!(
            find_route_without_turn_around(&g, 1, 3, 100),
            Ok(vec![1_i64, 20, 4, 5, 3])
        );
    }
}
