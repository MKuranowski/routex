// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

/// Recommended number of allowed node expansions in [find_route](crate::find_route) and
/// [find_route_without_turn_around](crate::find_route_without_turn_around)
/// before [AStarError::StepLimitExceeded] is returned.
pub const DEFAULT_STEP_LIMIT: usize = 1_000_000;

/// Error conditions which may occur during [find_route](crate::find_route) or
/// [find_route_without_turn_around](crate::find_route_without_turn_around).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AStarError {
    /// The start or end nodes don't exist in a graph.
    InvalidReference(i64),

    /// Route search has exceeded its limit of steps.
    /// Either the nodes are really far apart, or no route exists.
    ///
    /// Concluding that no route exists requires traversing the whole graph,
    /// which can result in a denial-of-service. The step limit protects
    /// against resource exhaustion.
    StepLimitExceeded,
}

impl std::fmt::Display for AStarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidReference(node_id) => write!(f, "invalid node: {}", node_id),
            Self::StepLimitExceeded => write!(f, "step limit exceeded"),
        }
    }
}

impl std::error::Error for AStarError {}
