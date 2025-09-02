// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use crate::{earth_distance, osm::profile::TurnRestriction, Edge, Graph, Node};

use super::{
    model::{self, FeatureType},
    FeatureReader, Options,
};

const MAX_NODE_ID: i64 = 0x0008_0000_0000_0000;

/// Helper object used for storing state related to converting [OSM features](super::model::Feature)
/// into a [Graph].
pub(super) struct GraphBuilder<'a> {
    g: &'a mut Graph,
    options: &'a Options<'a>,
    phantom_node_id_counter: i64,
    unused_nodes: HashSet<i64>,
    way_nodes: HashMap<i64, Vec<i64>>,
    ignore_bbox: bool,
}

impl<'a> GraphBuilder<'a> {
    /// Create a new, empty graph builder.
    pub(super) fn new(g: &'a mut Graph, options: &'a Options<'a>) -> Self {
        // Start adding phantom nodes at MAX_NODE_ID,
        // or the max node ID from the graph (in case phantom nodes were already added).
        let phantom_node_id_counter =
            MAX_NODE_ID.max(g.0.iter().map(|(&node_id, _)| node_id).max().unwrap_or(0));

        // TODO: Log invalid bounding boxes instead of discarding them
        let ignore_bbox =
            options.bbox.iter().all(|&x| x == 0.0) || options.bbox.iter().any(|x| !x.is_finite());

        Self {
            g,
            options,
            phantom_node_id_counter,
            unused_nodes: HashSet::default(),
            way_nodes: HashMap::default(),
            ignore_bbox,
        }
    }

    /// Add all features from the provided [FeatureReader].
    pub(super) fn add_features<F: FeatureReader>(
        &mut self,
        mut features: F,
    ) -> Result<(), F::Error> {
        while let Some(f) = features.next()? {
            self.add_feature(f);
        }
        self.cleanup();
        Ok(())
    }

    fn cleanup(&mut self) {
        self.unused_nodes
            .iter()
            .for_each(|&id| self.g.delete_node(id));
    }

    fn add_feature(&mut self, f: model::Feature) {
        match f {
            model::Feature::Node(n) => self.add_node(n),
            model::Feature::Way(w) => self.add_way(w),
            model::Feature::Relation(r) => self.add_relation(r),
        }
    }

    fn add_node(&mut self, n: Node) {
        // TODO: Log errors instead of silencing them

        if Self::is_valid_node_id(n.id) && self.is_in_bbox(n.lat, n.lon) {
            debug_assert_eq!(n.id, n.osm_id);
            self.g.set_node(n);
        }
    }

    fn is_valid_node_id(id: i64) -> bool {
        id != 0 && id < MAX_NODE_ID
    }

    fn is_in_bbox(&self, lat: f32, lon: f32) -> bool {
        if self.ignore_bbox {
            return true;
        }
        let [min_lon, min_lat, max_lon, max_lat] = self.options.bbox;
        lat >= min_lat && lat <= max_lat && lon >= min_lon && lon <= max_lon
    }

    fn add_way(&mut self, w: model::Way) {
        let penalty = self.get_way_penalty(&w);
        if penalty.is_infinite() {
            return;
        }

        let nodes = self.get_way_nodes(&w);
        if nodes.is_empty() {
            return;
        }

        let (forward, backward) = self.options.profile.way_direction(&w.tags);

        self.create_edges(&nodes, penalty, forward, backward);
        self.update_state_after_adding_way(w.id, nodes);
    }

    /// Gets the [penalty](crate::osm::profile::Penalty) applicable for the provided
    /// way and validates it. Returns [f32::INFINITY] or a valid (>= 1) penalty value.
    fn get_way_penalty(&self, w: &model::Way) -> f32 {
        let penalty = self.options.profile.way_penalty(&w.tags);
        if penalty.is_finite() && penalty >= 1.0 {
            penalty
        } else {
            f32::INFINITY // TODO: Log errors instead of silencing them
        }
    }

    fn get_way_nodes(&self, w: &model::Way) -> Vec<i64> {
        // Remove references to unknown nodes
        let nodes: Vec<i64> = w
            .nodes
            .iter()
            .cloned()
            .filter(|&node_id| self.g.get_node(node_id).is_some())
            .collect();

        if nodes.len() < 2 {
            vec![] // TODO: Warn about too short ways
        } else {
            nodes
        }
    }

    fn create_edges(&mut self, nodes: &[i64], penalty: f32, forward: bool, backward: bool) {
        debug_assert!(nodes.len() >= 2);
        debug_assert!(penalty.is_finite() && penalty >= 1.0);
        debug_assert!(forward || backward);

        nodes.windows(2).for_each(|pair| {
            let left = self
                .g
                .get_node(pair[0])
                .expect("get_way_nodes should only return nodes which exist");

            let right = self
                .g
                .get_node(pair[1])
                .expect("get_way_nodes should only return nodes which exist");

            let cost = penalty * earth_distance(left.lat, left.lon, right.lat, right.lon);

            if forward {
                self.g.set_edge(left.id, Edge { to: right.id, cost });
            }
            if backward {
                self.g.set_edge(right.id, Edge { to: left.id, cost });
            }
        });
    }

    fn update_state_after_adding_way(&mut self, way_id: i64, nodes: Vec<i64>) {
        nodes.iter().for_each(|node_id| {
            self.unused_nodes.remove(node_id);
        });
        self.way_nodes.insert(way_id, nodes);
    }

    fn add_relation(&mut self, r: model::Relation) {
        let kind = self.options.profile.restriction_kind(&r.tags);
        if kind == TurnRestriction::Inapplicable {
            return;
        }

        let nodes = match self.get_restriction_nodes(&r) {
            Ok(nodes) => nodes,
            Err(_) => return, // TODO: Warn about invalid relations
        };

        _ = self.store_restriction(r.id, &nodes, kind); // TODO: Warn about failures
    }

    /// Returns the sequence of nodes representing a turn restriction.
    /// Only the last 2 nodes of the `from` and the first 2 nodes of the `to` members
    /// are taken into account.
    fn get_restriction_nodes(&self, r: &model::Relation) -> Result<Vec<i64>, InvalidRestriction> {
        let members = Self::get_ordered_restriction_members(r)?;
        let mut member_nodes = members
            .iter()
            .map(|&m| self.get_relation_member_nodes(m))
            .collect::<Result<Vec<_>, _>>()?;
        self.flatten_member_nodes(&mut member_nodes)
    }

    /// Returns a list of turn restriction members in the order of from-via-...-via-to.
    /// Ensures there is exactly one `from` and `to``, and at least one `via` member.
    fn get_ordered_restriction_members<'r>(
        r: &'r model::Relation,
    ) -> Result<Vec<&'r model::RelationMember>, InvalidRestriction> {
        let mut from: Option<&'r model::RelationMember> = None;
        let mut to: Option<&'r model::RelationMember> = None;
        let mut order: Vec<&'r model::RelationMember> = vec![];

        for m in &r.members {
            match m.role.as_str() {
                "from" => {
                    if from.is_some() {
                        return Err(InvalidRestriction::MultipleFromMembers);
                    } else {
                        from = Some(m);
                    }
                }

                "via" => order.push(m),

                "to" => {
                    if to.is_some() {
                        return Err(InvalidRestriction::MultipleToMembers);
                    } else {
                        to = Some(m);
                    }
                }

                _ => {}
            }
        }

        match (from, to) {
            (Some(from), Some(to)) => {
                order.insert(0, from);
                order.push(to);
                Ok(order)
            }
            (None, _) => Err(InvalidRestriction::MissingFromMember),
            (_, None) => Err(InvalidRestriction::MissingToMember),
        }
    }

    /// Returns a list of nodes corresponding to the given restriction member.
    ///
    /// [FeatureType::Node] references are only permitted for `via` members,
    /// [FeatureType::Way] references are only permitted for all members, and
    /// [FeatureType::Relation] are not permitted.
    fn get_relation_member_nodes(
        &self,
        m: &model::RelationMember,
    ) -> Result<Vec<i64>, InvalidRestriction> {
        match (m.type_, m.role.as_str()) {
            (FeatureType::Node, "via") => {
                if self.g.get_node(m.ref_).is_some() {
                    Ok(vec![m.ref_])
                } else {
                    Err(InvalidRestriction::ReferenceToUnknownNode(m.ref_))
                }
            }

            (FeatureType::Way, _) => {
                if let Some(nodes) = self.way_nodes.get(&m.ref_) {
                    Ok(nodes.clone())
                } else {
                    Err(InvalidRestriction::ReferenceToUnknownWay(m.ref_))
                }
            }

            (_, _) => Err(InvalidRestriction::InvalidMemberType(
                m.role.clone(),
                m.type_,
            )),
        }
    }

    /// Turns a list of turn restriction members' nodes into a list of nodes of the restriction
    /// itself. Only the last two nodes of the first member, and the first two nodes of the
    /// last are considered.
    fn flatten_member_nodes(
        &self,
        members: &mut [Vec<i64>],
    ) -> Result<Vec<i64>, InvalidRestriction> {
        assert!(members.len() >= 2);
        let mut nodes = vec![];

        for idx in 0..members.len() {
            assert!(members[idx].len() > 0);
            let is_first = idx == 0;
            let is_last = idx == members.len() - 1;

            // Reverse members to ensure the restriction is continuous
            if is_first {
                // First member needs to be reversed if its first node
                // matches with the second member's first or last node.
                if members[idx].first() == members[1].first()
                    || members[idx].first() == members[1].last()
                {
                    // incorrect order, (B-A, B-C) or (B-A, C-B) case
                    members[idx].reverse();
                }
            } else {
                // Every non-first member needs to be reversed if its last node
                // matches the last member's last node
                if nodes.last() == members[idx].last() {
                    members[idx].reverse();
                }
            }

            // Check if the restriction is continuous
            if !is_first && nodes.last() != members[idx].first() {
                return Err(InvalidRestriction::Disjoint);
            }

            // Merge the nodes
            if is_first {
                // "from" member - only care about the last 2 nodes; A-B-C-D → C-D
                assert!(members[idx].len() >= 2);
                nodes.extend_from_slice(&members[idx][members[idx].len() - 2..]);
            } else if is_last {
                // "to" member - only care about the first 2 nodes,
                // but the first node was appended as the last node of the previous member,
                // thus only append the second node
                // A-B-C-D → A-B -("A" appended in previous step)→ B
                assert!(members[idx].len() >= 2);
                nodes.push(members[idx][1]);
            } else {
                nodes.extend_from_slice(&members[idx][1..]);
            }
        }

        Ok(nodes)
    }

    fn store_restriction(
        &mut self,
        _relation_id: i64,
        nodes: &[i64],
        kind: TurnRestriction,
    ) -> Result<(), InvalidRestriction> {
        // To store a turn restriction A-B-C-D-E, we replace all via nodes with phantom clones,
        // cloned with the outgoing edges A-B'-C'-D'-E, and replace the A-B edge by A-B'.
        // For prohibitory restrictions, the D'-E then needs to be removed.
        // For mandatory restrictions, all edges not following the mandated path need to be removed.
        // If the B' phantom edge need to be removed.

        let mut change = GraphChange::new(self);
        let cloned_nodes = if let Some(n) = change.restriction_as_cloned_nodes(self.g, nodes) {
            n
        } else {
            return Ok(()); // failed to apply the restriction - discard it
        };

        match kind {
            TurnRestriction::Mandatory => {
                for pair in cloned_nodes[1..].windows(2) {
                    // unsafe { assert_unchecked(pair.len() == 2) };
                    change.ensure_only_edge(self.g, pair[0], pair[1]);
                }
            },

            TurnRestriction::Prohibitory => {
                let a = cloned_nodes[cloned_nodes.len()-2];
                let b = cloned_nodes[cloned_nodes.len()-1];
                change.remove_edge(a, b);
            },

            TurnRestriction::Inapplicable => assert!(false, "GraphBuilder::store_restriction should not be called with TurnRestriction::Inapplicable")
        }

        change.apply(self);
        return Ok(());
    }
}

#[derive(Debug)]
enum InvalidRestriction {
    Disjoint,
    MultipleFromMembers,
    MultipleToMembers,
    MissingFromMember,
    MissingToMember,
    ReferenceToUnknownNode(i64),
    ReferenceToUnknownWay(i64),
    InvalidMemberType(String, FeatureType),
}

impl std::fmt::Display for InvalidRestriction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disjoint => write!(f, "disjoint turn restriction"),
            Self::MultipleFromMembers => write!(f, "multiple 'from' members"),
            Self::MultipleToMembers => write!(f, "multiple 'to' members"),
            Self::MissingFromMember => write!(f, "missing 'from' member"),
            Self::MissingToMember => write!(f, "missing 'to' member"),
            Self::ReferenceToUnknownNode(node_id) => {
                write!(f, "reference to unknown node {node_id}")
            }
            Self::ReferenceToUnknownWay(way_id) => {
                write!(f, "reference to unknown way {way_id}")
            }
            Self::InvalidMemberType(role, type_) => {
                write!(f, "member with role {role} can't be of type {type_}")
            }
        }
    }
}

impl std::error::Error for InvalidRestriction {}

struct GraphChange {
    /// Map of nodes to clone (including their outgoing edges),
    /// mapping new node ids to old node ids.
    new_nodes: HashMap<i64, i64>,

    /// Set of edges to remove from the graph.
    /// Applied after [new_nodes], but before [edges_to_add].
    edges_to_remove: HashSet<(i64, i64)>,

    /// New edges to add to the graph.
    /// Applied after [new_nodes] and after [edges_to_remove].
    edges_to_add: HashMap<i64, HashMap<i64, f32>>,

    /// New value for [GraphBuilder::phantom_node_id_counter].
    phantom_node_id_counter: i64,
}

impl GraphChange {
    fn new(b: &GraphBuilder<'_>) -> Self {
        Self {
            new_nodes: HashMap::default(),
            edges_to_remove: HashSet::default(),
            edges_to_add: HashMap::default(),
            phantom_node_id_counter: b.phantom_node_id_counter,
        }
    }

    /// Turns a A-B-C-D-E list of OSM nodes into a A-B'-C'-D'-E list by cloning
    /// all middle nodes. Cloned nodes (including E') may be re-used, if a B' node and A-B' edge
    /// already exists.
    ///
    /// Returns ``None`` if osm_nodes represents a disjoined sequence, and in this case
    /// the _GraphChange **must** be discarded, as it may contain garbage changes.
    fn restriction_as_cloned_nodes(&mut self, g: &Graph, nodes: &[i64]) -> Option<Vec<i64>> {
        assert!(nodes.len() >= 3);

        let mut cloned_nodes = vec![nodes[0]];
        for i in 1..nodes.len() {
            let previous_node_id = nodes[i - 1];
            let osm_id = nodes[i];
            let candidate_node_id = self.get_to_node_id_by_edge(g, previous_node_id, osm_id)?;

            let is_cloned = osm_id == candidate_node_id;
            let is_last = Some(osm_id) == nodes.last().cloned();

            // We need to make a clone of `osm_id` if we don't have a cloned node already and it's not the last node
            let node_id = if !is_cloned && !is_last {
                let node_id = self.clone_node(candidate_node_id);

                // Relink previous_node_id -> osm_id to previous_node_id -> node_id
                let cost = self.get_edge_cost(g, previous_node_id, osm_id);
                self.edges_to_remove.insert((previous_node_id, osm_id));
                self.edges_to_add
                    .entry(previous_node_id)
                    .or_insert_with(HashMap::new)
                    .insert(node_id, cost);
                node_id
            } else {
                candidate_node_id
            };

            cloned_nodes.push(node_id);
        }

        Some(cloned_nodes)
    }

    fn clone_node(&mut self, src: i64) -> i64 {
        self.phantom_node_id_counter += 1;
        let dst = self.phantom_node_id_counter;
        self.new_nodes.insert(dst, src);
        dst
    }

    /// Finds the first edge from `from_node_id` to a node with `osm_id == to_osm_id` and returns
    /// the corresponding node id.
    fn get_to_node_id_by_edge(&self, g: &Graph, from_node_id: i64, to_osm_id: i64) -> Option<i64> {
        let from_osm_id = self
            .new_nodes
            .get(&from_node_id)
            .cloned()
            .unwrap_or(from_node_id);

        g.get_edges(from_osm_id)
            .iter()
            .map(|e| (e.to, g.get_node(e.to).map(|n| n.osm_id)))
            .find(|&(_, osm_id)| osm_id == Some(to_osm_id))
            .map(|(node_id, _)| node_id)
    }

    /// Gets the cost of the edge from `from` to `to`, allowing `from` to be a cloned node.
    fn get_edge_cost(&self, g: &Graph, from: i64, to: i64) -> f32 {
        if let Some(overridden_cost) = self
            .edges_to_add
            .get(&from)
            .and_then(|edges| edges.get(&to))
        {
            *overridden_cost
        } else {
            let original_from = self.new_nodes.get(&from).cloned().unwrap_or(from);
            g.get_edge(original_from, to)
        }
    }

    /// Ensures the only edge from a (possibly-cloned) `from` node is to `to`
    fn ensure_only_edge(&mut self, g: &Graph, from: i64, to: i64) {
        // If adding edges, ensure the only new edge from `from` is to `to`
        if let Some(edges_to_add) = self.edges_to_add.get_mut(&from) {
            edges_to_add.retain(|&i, _| i == to);
        }

        // If `from` is cloned, ensure only the from-to edge is cloned
        let original_from = self.new_nodes.get(&from).cloned().unwrap_or(from);
        for edge in g.get_edges(original_from) {
            self.edges_to_remove.insert((from, edge.to));
        }
    }

    /// Ensure the edge from `from` to `to` does not exist
    fn remove_edge(&mut self, from: i64, to: i64) {
        self.edges_to_remove.insert((from, to));
    }

    fn apply(&self, b: &mut GraphBuilder<'_>) {
        b.phantom_node_id_counter = self.phantom_node_id_counter;
        self.apply_clone_nodes(b.g);
        self.apply_remove_edges(b.g);
        self.apply_add_edges(b.g);
    }

    fn apply_clone_nodes(&self, g: &mut Graph) {
        for (&new_id, &old_id) in &self.new_nodes {
            let old_node = g
                .get_node(old_id)
                .expect("GraphChange::apply can only be called with valid changes");

            g.set_node(Node {
                id: new_id,
                osm_id: old_node.osm_id,
                lat: old_node.lat,
                lon: old_node.lon,
            });

            g.clone_edges(new_id, old_id);
        }
    }

    fn apply_remove_edges(&self, g: &mut Graph) {
        for &(from, to) in &self.edges_to_remove {
            g.delete_edge(from, to);
        }
    }

    fn apply_add_edges(&self, g: &mut Graph) {
        for (&from, edges) in &self.edges_to_add {
            for (&to, &cost) in edges {
                g.set_edge(from, Edge { to, cost });
            }
        }
    }
}
