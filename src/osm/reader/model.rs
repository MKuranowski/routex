// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use crate::Node;
use std::collections::HashMap;

/// Represents an [OSM way](https://wiki.openstreetmap.org/wiki/Way).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Way {
    pub id: i64,
    pub nodes: Vec<i64>,
    pub tags: HashMap<String, String>,
}

/// Type of an [OSM feature/element](https://wiki.openstreetmap.org/wiki/Elements).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureType {
    Node,
    Way,
    Relation,
}

impl std::fmt::Display for FeatureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Way => write!(f, "way"),
            Self::Relation => write!(f, "relation"),
        }
    }
}

/// Represents a member of an [OSM relation](https://wiki.openstreetmap.org/wiki/Relation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationMember {
    pub type_: FeatureType,
    pub ref_: i64,
    pub role: String,
}

/// Represents an [OSM relation](https://wiki.openstreetmap.org/wiki/Relation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub id: i64,
    pub members: Vec<RelationMember>,
    pub tags: HashMap<String, String>,
}

/// Union over all possible [OSM features/elements](https://wiki.openstreetmap.org/wiki/Elements).
#[derive(Debug, Clone)]
pub enum Feature {
    Node(Node),
    Way(Way),
    Relation(Relation),
}
