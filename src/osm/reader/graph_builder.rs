// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use crate::Graph;

use super::{FeatureReader, Options};

/// Helper object used for storing state related to converting [OSM features](super::model::Feature)
/// into a [Graph].
pub(super) struct GraphBuilder<'a> {
    g: &'a mut Graph,
    options: &'a Options<'a>,
}

impl<'a> GraphBuilder<'a> {
    /// Create a new, empty graph builder.
    pub fn new(g: &'a mut Graph, options: &'a Options<'a>) -> Self {
        todo!()
    }

    /// Add all features from the provided [FeatureReader].
    pub fn add_features<F: FeatureReader>(&mut self, mut features: F) -> Result<(), F::Error> {
        todo!()
    }
}
