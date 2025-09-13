// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use crate::osm::profile::TurnRestriction;
use crate::osm::reader::FeatureReader;
use crate::{earth_distance, Edge, Graph, Node};

use super::model::FeatureType;
use super::{model, Options};

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

        let ignore_bbox = !is_bbox_applicable(options.bbox);

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
    pub(super) fn add_features<F: FeatureReader>(&mut self, features: F) -> Result<(), F::Error> {
        for f in features {
            self.add_feature(f?);
        }
        self.cleanup();
        Ok(())
    }

    fn cleanup(&mut self) {
        self.unused_nodes.iter().for_each(|&id| {
            self.g.delete_node(id);
        });
    }

    fn add_feature(&mut self, f: model::Feature) {
        match f {
            model::Feature::Node(n) => self.add_node(n),
            model::Feature::Way(w) => self.add_way(w),
            model::Feature::Relation(r) => self.add_relation(r),
        }
    }

    fn add_node(&mut self, n: Node) {
        debug_assert_eq!(n.id, n.osm_id);

        // Node already exists - ignore
        if self.g.get_node(n.id).is_some() {
            return;
        }

        // Node id invalid - ignore & warn
        if !Self::is_valid_node_id(n.id) {
            log::warn!(target: "routex.osm", "node with invalid id {} - ignoring", n.id);
            return;
        }

        // Node outside of bbox - ignore
        if !self.is_in_bbox(n.lat, n.lon) {
            return;
        }

        // Save node
        self.g.set_node(n);
        self.unused_nodes.insert(n.id);
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
        if !penalty.is_finite() {
            f32::INFINITY // Way not routable
        } else if penalty < 1.0 {
            log::error!(target: "routex", "profile has invalid penalty {} - assuming non-routable", penalty);
            f32::INFINITY
        } else {
            penalty
        }
    }

    fn get_way_nodes(&self, w: &model::Way) -> Vec<i64> {
        // Check if way has enough nodes
        if w.nodes.len() < 2 {
            log::warn!(target: "routex.osm", "way {} has less than 2 nodes - ignoring", w.id);
            return vec![];
        }

        // Filter out invalid nodes
        // NOTE: We don't warn about invalid references, as they may have been deliberately
        //       filtered out by the bbox. We're not an osm validator.
        let nodes: Vec<i64> = w
            .nodes
            .iter()
            .cloned()
            .filter(|&node_id| self.g.get_node(node_id).is_some())
            .collect();

        if nodes.len() < 2 {
            vec![]
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
        match self.add_relation_inner(&r) {
            Ok(()) => {}
            Err(e) => e.log(r.id),
        }
    }

    fn add_relation_inner(&mut self, r: &model::Relation) -> Result<(), InvalidRestriction> {
        let kind = self.options.profile.restriction_kind(&r.tags);
        if kind == TurnRestriction::Inapplicable {
            return Ok(());
        }

        let nodes = self.get_restriction_nodes(&r)?;
        self.store_restriction(&nodes, kind)?;
        Ok(())
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

#[derive(Debug, thiserror::Error)]
enum InvalidRestriction {
    #[error("disjoint turn restriction")]
    Disjoint,

    #[error("multiple 'from' members")]
    MultipleFromMembers,

    #[error("multiple 'to' members")]
    MultipleToMembers,

    #[error("missing 'from' member")]
    MissingFromMember,

    #[error("missing 'to' member")]
    MissingToMember,

    #[error("reference to unknown node {0}")]
    ReferenceToUnknownNode(i64),

    #[error("reference to unknown way {0}")]
    ReferenceToUnknownWay(i64),

    #[error("member with role {0} can't be of type {1}")]
    InvalidMemberType(String, FeatureType),
}

impl InvalidRestriction {
    fn log(&self, relation_id: i64) {
        match self {
            // NOTE: We don't warn about invalid references, as they may have been deliberately
            //       filtered out by the bbox. We're not an osm validator.
            Self::ReferenceToUnknownNode(_) => {}
            Self::ReferenceToUnknownWay(_) => {}

            _ => {
                log::warn!(target: "routex.osm", "relation {} - {} - ignoring", relation_id, self);
            }
        }
    }
}

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
            let previous_node_id = cloned_nodes[i - 1];
            let osm_id = nodes[i];
            let candidate_node_id = self.get_to_node_id_by_edge(g, previous_node_id, osm_id)?;

            let is_cloned = osm_id != candidate_node_id;
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
            if edge.to != to {
                self.edges_to_remove.insert((from, edge.to));
            }
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

fn is_bbox_applicable(bbox: [f32; 4]) -> bool {
    // All elements 0 - no bbox
    if bbox.iter().all(|&x| x == 0.0) {
        return false;
    }

    // Some elements non-finite - invalid bbox
    if bbox.iter().any(|x| !x.is_finite()) {
        log::error!(target: "routex", "bounding box contains non-finite elements - ignoring");
        return false;
    }

    // Check min-max pairs
    let [left, bottom, right, top] = bbox;
    if left >= right {
        log::error!(target: "routex", "bounding box has zero areas - left >= right - ignoring");
        false
    } else if bottom >= top {
        log::error!(target: "routex", "bounding box has zero areas - bottom >= top - ignoring");
        false
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{FileFormat, CAR_PROFILE};
    use super::super::model::{FeatureType, Relation, RelationMember, Way};
    use super::*;

    macro_rules! tags {
        {} => { HashMap::default() };
        {$( $k:literal : $v:literal ),+} => {
            HashMap::from_iter([ $( ($k.to_string(), $v.to_string()) ),+ ])
        };
    }

    macro_rules! n {
        ($id:expr, $lat:expr, $lon:expr) => {
            Node {
                id: $id,
                osm_id: $id,
                lat: $lat,
                lon: $lon,
            }
        };

        ($id:expr, $osm_id:expr, $lat:expr, $lon:expr) => {
            Node {
                id: $id,
                osm_id: $osm_id,
                lat: $lat,
                lon: $lon,
            }
        };
    }

    macro_rules! w {
        ($id:expr, $nodes:expr) => {
            Way {
                id: $id,
                nodes: $nodes,
                tags: HashMap::default(),
            }
        };

        ($id:expr, $nodes:expr, $tags:expr) => {
            Way {
                id: $id,
                nodes: $nodes,
                tags: $tags,
            }
        };
    }

    macro_rules! m {
        ($type_:expr, $ref_:expr, $role:expr) => {
            RelationMember {
                type_: $type_,
                ref_: $ref_,
                role: $role.to_string(),
            }
        };
    }

    macro_rules! r {
        ($id:expr, $members:expr) => {
            Relation {
                id: $id,
                members: $members,
                tags: HashMap::default(),
            }
        };

        ($id:expr, $members:expr, $tags:expr) => {
            Relation {
                id: $id,
                members: $members,
                tags: $tags,
            }
        };
    }

    macro_rules! e {
        ($to:expr, $cost:expr) => {
            Edge {
                to: $to,
                cost: $cost,
            }
        };
    }

    macro_rules! assert_edge {
        ($graph:expr, $from:expr, $to:expr) => {
            assert!($graph.get_edge($from, $to).is_finite());
        };
    }

    macro_rules! assert_no_edge {
        ($graph:expr, $from:expr, $to:expr) => {
            assert!($graph.get_edge($from, $to).is_infinite());
        };
    }

    const DEFAULT_OPTIONS: Options<'static> = Options {
        profile: &CAR_PROFILE,
        file_format: FileFormat::Xml,
        bbox: [0.0; 4],
    };

    mod graph_builder {
        use super::*;

        #[test]
        fn test_add_node() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.0, 0.0));

                assert!(b.unused_nodes.contains(&1));
            }

            assert_eq!(
                g.get_node(1),
                Some(Node {
                    id: 1,
                    osm_id: 1,
                    lat: 0.0,
                    lon: 0.0
                })
            );
        }

        #[test]
        fn test_add_node_duplicate() {
            let mut g = Graph::default();
            g.set_node(n!(1, 0.0, 0.0));

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.1, -4.2));
            }

            assert_eq!(g.get_node(1), Some(n!(1, 0.0, 0.0)));
        }

        #[test]
        fn test_add_node_big_osm_id() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(MAX_NODE_ID, 0.0, 0.0));
            }

            assert_eq!(g.get_node(MAX_NODE_ID), None);
        }

        #[test]
        fn test_add_way() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 1.0, 0.0));
                b.add_node(n!(3, 0.0, 1.0));
                b.add_way(w!(1, vec![1, 2, 3], tags!("highway": "primary")));

                assert!(b.unused_nodes.is_empty());
                assert_eq!(b.way_nodes.get(&1), Some(&vec![1, 2, 3]));
            }

            assert_edge!(g, 1, 2);
            assert_edge!(g, 2, 3);
            assert_no_edge!(g, 1, 3);
            assert_edge!(g, 3, 2);
            assert_edge!(g, 2, 1);
            assert_no_edge!(g, 3, 1);
        }

        #[test]
        fn test_add_way_one_way() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 1.0, 0.0));
                b.add_node(n!(3, 0.0, 1.0));
                b.add_way(w!(
                    1,
                    vec![1, 2, 3],
                    tags!("highway": "primary", "oneway": "yes")
                ));

                assert!(b.unused_nodes.is_empty());
                assert_eq!(b.way_nodes.get(&1), Some(&vec![1, 2, 3]));
            }

            assert_edge!(g, 1, 2);
            assert_edge!(g, 2, 3);
            assert_no_edge!(g, 1, 3);
            assert_no_edge!(g, 3, 2);
            assert_no_edge!(g, 2, 1);
            assert_no_edge!(g, 3, 1);
        }

        #[test]
        fn test_add_way_not_routable() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_way(w!(
                    1,
                    vec![1, 2, 3],
                    tags!("highway": "primary", "access": "no")
                ));

                assert!(b.unused_nodes.contains(&1));
                assert!(b.unused_nodes.contains(&2));
                assert!(b.unused_nodes.contains(&3));
                assert!(!b.way_nodes.contains_key(&10));
            }

            assert_no_edge!(g, 1, 2);
            assert_no_edge!(g, 2, 3);
            assert_no_edge!(g, 1, 3);
            assert_no_edge!(g, 3, 2);
            assert_no_edge!(g, 2, 1);
            assert_no_edge!(g, 3, 1);
        }

        #[test]
        fn test_add_relation_prohibitory() {
            //     4
            //     │
            // 1───2───3
            // no_left_turn: 1->2->4

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);

            assert_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);
        }

        #[test]
        fn test_add_relation_prohibitory_not_applicable() {
            //     4
            //     ↓
            // 1───2───3
            // no_left_turn: 1->2->4

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(
                    12,
                    vec![4, 2],
                    tags!("highway": "primary", "oneway": "yes")
                ));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));

                assert_eq!(b.phantom_node_id_counter, 100);
            }

            assert!(g.get_node(101).is_none());

            assert_edge!(g, 1, 2);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_no_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);
        }

        #[test]
        fn test_add_relation_two_prohibitory() {
            //     4
            //     │
            // 1───2───3
            // no_left_turn: 1->2->4
            // no_right_turn: 4->2->1

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));
                b.add_relation(r!(
                    21,
                    vec![
                        m!(FeatureType::Way, 12, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 10, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_right_turn")
                ));
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);

            assert_no_edge!(g, 4, 2);
            assert_edge!(g, 4, 102);

            assert_edge!(g, 101, 1);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);

            assert_no_edge!(g, 102, 1);
            assert_no_edge!(g, 102, 2);
            assert_edge!(g, 102, 3);
            assert_edge!(g, 102, 4);
        }

        #[test]
        fn test_add_relation_two_prohibitory_with_same_activator() {
            //     4
            //     │
            // 1───2───3
            //     │
            //     5
            // no_left_turn: 1->2->4
            // no_right_turn: 1->2->5

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_node(n!(5, 0.1, -0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_way(w!(13, vec![2, 5], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 13, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_right_turn")
                ));

                assert_eq!(b.phantom_node_id_counter, 101);
            }

            assert_eq!(g.len(), 6);
            assert!(g.get_node(1).is_some());
            assert!(g.get_node(2).is_some());
            assert!(g.get_node(3).is_some());
            assert!(g.get_node(4).is_some());
            assert!(g.get_node(5).is_some());
            assert!(g.get_node(101).is_some());

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);
            assert_edge!(g, 2, 5);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);
            assert_edge!(g, 5, 2);

            assert_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);
            assert_no_edge!(g, 101, 5);
        }

        #[test]
        fn test_add_relation_mandatory() {
            //     4
            //     │
            // 1───2───3
            // only_straight_on: 1->2->3

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_straight_on")
                ));
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);

            assert_no_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);
        }

        #[test]
        fn test_add_relation_mandatory_not_applicable() {
            //     4
            //     ↓
            // 1───2───3
            // only_left_turn: 1->2->4

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(
                    12,
                    vec![4, 2],
                    tags!("highway": "primary", "oneway": "yes")
                ));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_left_turn")
                ));

                assert_eq!(b.phantom_node_id_counter, 100);
            }

            assert!(g.get_node(101).is_none());

            assert_edge!(g, 1, 2);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_no_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);
        }

        #[test]
        fn test_add_relation_two_mandatory() {
            //     4
            //     │
            // 1───2───3
            // only_straight_on: 1->2->3
            // only_left_turn: 4->2->3

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_straight_on")
                ));
                b.add_relation(r!(
                    21,
                    vec![
                        m!(FeatureType::Way, 12, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_left_turn")
                ));

                assert_eq!(b.phantom_node_id_counter, 102);
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);

            assert_no_edge!(g, 4, 2);
            assert_edge!(g, 4, 102);

            assert_no_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);

            assert_no_edge!(g, 102, 1);
            assert_no_edge!(g, 102, 2);
            assert_edge!(g, 102, 3);
            assert_no_edge!(g, 102, 4);
        }

        #[test]
        fn test_add_relation_two_conflicting_mandatory() {
            //     4
            //     │
            // 1───2───3
            // only_straight_on: 1->2->3 (applied)
            // only_left_turn: 1->2->4 (ignored)

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_straight_on")
                ));
                b.add_relation(r!(
                    21,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_left_turn")
                ));

                assert_eq!(b.phantom_node_id_counter, 101);
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);

            assert_no_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);
        }

        #[test]
        fn test_add_relation_mandatory_and_prohibitory_with_same_activator() {
            //     4
            //     │
            // 1───2───3
            // no_left_turn: 1->2->4
            // only_straight_on: 1->2->3

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.1, 0.1));
                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![2, 4], tags!("highway": "primary")));
                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 12, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));
                b.add_relation(r!(
                    21,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_straight_on")
                ));
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 4);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 4, 2);

            assert_no_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 2);
            assert_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 4);
        }

        #[test]
        fn test_add_relation_contained_within_another() {
            //     5   6
            //     │   │
            // 1───2───3───4
            // no_left_turn: 1->2->3->6
            // only_straight_on: 1->2->3

            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 100;

                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.3, 0.0));
                b.add_node(n!(5, 0.1, 0.1));
                b.add_node(n!(6, 0.2, 0.1));

                b.add_way(w!(10, vec![1, 2], tags!("highway": "primary")));
                b.add_way(w!(11, vec![2, 3], tags!("highway": "primary")));
                b.add_way(w!(12, vec![3, 4], tags!("highway": "primary")));
                b.add_way(w!(13, vec![2, 5], tags!("highway": "primary")));
                b.add_way(w!(14, vec![3, 6], tags!("highway": "primary")));

                b.add_relation(r!(
                    20,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Way, 11, "via"),
                        m!(FeatureType::Way, 14, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "no_left_turn")
                ));
                b.add_relation(r!(
                    21,
                    vec![
                        m!(FeatureType::Way, 10, "from"),
                        m!(FeatureType::Node, 2, "via"),
                        m!(FeatureType::Way, 11, "to"),
                    ],
                    tags!("type": "restriction", "restriction": "only_straight_on")
                ));

                assert_eq!(b.phantom_node_id_counter, 102);
            }

            assert_no_edge!(g, 1, 2);
            assert_edge!(g, 1, 101);

            assert_edge!(g, 101, 102);
            assert_no_edge!(g, 101, 1);
            assert_no_edge!(g, 101, 3);
            assert_no_edge!(g, 101, 5);

            assert_edge!(g, 102, 2);
            assert_edge!(g, 102, 4);
            assert_no_edge!(g, 102, 6);

            assert_edge!(g, 2, 1);
            assert_edge!(g, 2, 3);
            assert_edge!(g, 2, 5);

            assert_edge!(g, 3, 2);
            assert_edge!(g, 3, 4);
            assert_edge!(g, 3, 6);

            assert_edge!(g, 4, 3);
            assert_edge!(g, 5, 2);
            assert_edge!(g, 6, 3);
        }

        #[test]
        fn test_cleanup() {
            let mut g = Graph::default();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.add_node(n!(1, 0.0, 0.0));
                b.add_node(n!(2, 0.1, 0.0));
                b.add_node(n!(3, 0.2, 0.0));
                b.add_node(n!(4, 0.2, 0.1));
                b.add_node(n!(5, 0.2, 0.1));
                b.add_way(w!(10, vec![1, 2, 3], tags!("highway": "primary")));

                assert_eq!(b.g.len(), 5);
                assert!(b.g.get_node(1).is_some());
                assert!(b.g.get_node(2).is_some());
                assert!(b.g.get_node(3).is_some());
                assert!(b.g.get_node(4).is_some());
                assert!(b.g.get_node(5).is_some());

                assert_eq!(b.unused_nodes.len(), 2);
                assert!(b.unused_nodes.contains(&4));
                assert!(b.unused_nodes.contains(&5));

                b.cleanup();
            }

            assert_eq!(g.len(), 3);
            assert!(g.get_node(1).is_some());
            assert!(g.get_node(2).is_some());
            assert!(g.get_node(3).is_some());
        }
    }

    mod graph_change {
        use super::*;

        #[inline]
        fn fixture_graph() -> Graph {
            //  (200) (200) (200)
            // 1─────2─────3─────4
            //       └─────5─────┘
            //        (100) (100)

            let mut g = Graph::default();
            g.set_node(n!(1, 0.0, 0.0));
            g.set_node(n!(2, 0.1, 0.0));
            g.set_node(n!(3, 0.2, 0.0));
            g.set_node(n!(4, 0.3, 0.0));
            g.set_node(n!(5, 0.2, 0.1));
            g.set_edge(1, e!(2, 200.0));
            g.set_edge(2, e!(1, 200.0));
            g.set_edge(2, e!(3, 200.0));
            g.set_edge(2, e!(5, 100.0));
            g.set_edge(3, e!(2, 200.0));
            g.set_edge(3, e!(4, 200.0));
            g.set_edge(4, e!(3, 200.0));
            g.set_edge(4, e!(5, 100.0));
            g.set_edge(5, e!(2, 100.0));
            g.set_edge(5, e!(4, 100.0));
            g
        }

        #[test]
        fn test_restriction_as_cloned_nodes() {
            let mut g = fixture_graph();
            let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
            b.phantom_node_id_counter = 10;

            let mut c = GraphChange::new(&b);
            let cloned = c.restriction_as_cloned_nodes(&b.g, &[1, 2, 5]);

            assert_eq!(cloned, Some(vec![1, 11, 5]));

            assert_eq!(c.new_nodes.len(), 1);
            assert_eq!(c.new_nodes.get(&11).cloned(), Some(2));

            assert_eq!(c.edges_to_add.len(), 1);
            assert_eq!(c.edges_to_add[&1].len(), 1);
            assert_eq!(c.edges_to_add[&1][&11], 200.0);

            assert_eq!(c.edges_to_remove.len(), 1);
            assert!(c.edges_to_remove.contains(&(1, 2)));

            assert_eq!(c.phantom_node_id_counter, 11);
        }

        #[test]
        fn test_restriction_as_cloned_nodes_reuses_cloned_nodes() {
            let mut g = fixture_graph();
            g.set_node(n!(11, 2, 0.1, 0.0));
            g.delete_edge(1, 2);
            g.set_edge(1, e!(11, 200.0));
            g.set_edge(11, e!(1, 200.0));
            g.set_edge(11, e!(3, 200.0));
            g.set_edge(11, e!(5, 100.0));

            let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
            b.phantom_node_id_counter = 11;

            let mut c = GraphChange::new(&b);
            let cloned = c.restriction_as_cloned_nodes(&b.g, &[1, 2, 3]);

            assert_eq!(cloned, Some(vec![1, 11, 3]));

            assert_eq!(c.new_nodes.len(), 0);
            assert_eq!(c.edges_to_add.len(), 0);
            assert_eq!(c.edges_to_remove.len(), 0);
            assert_eq!(c.phantom_node_id_counter, 11);
        }

        #[test]
        fn test_restriction_as_cloned_nodes_reuses_last_nodes() {
            let mut g = fixture_graph();
            g.set_node(n!(11, 2, 0.1, 0.0));
            g.set_node(n!(12, 3, 0.2, 0.0));
            g.delete_edge(1, 2);
            g.set_edge(1, e!(11, 200.0));
            g.set_edge(11, e!(1, 200.0));
            g.set_edge(11, e!(12, 200.0));
            g.set_edge(11, e!(5, 100.0));
            g.set_edge(12, e!(2, 200.0));
            g.set_edge(12, e!(4, 200.0));

            let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
            b.phantom_node_id_counter = 12;

            let mut c = GraphChange::new(&b);
            let cloned = c.restriction_as_cloned_nodes(&b.g, &[1, 2, 3]);

            assert_eq!(cloned, Some(vec![1, 11, 12]));

            assert_eq!(c.new_nodes.len(), 0);
            assert_eq!(c.edges_to_add.len(), 0);
            assert_eq!(c.edges_to_remove.len(), 0);
            assert_eq!(c.phantom_node_id_counter, 12);
        }

        #[test]
        fn test_restriction_as_cloned_nodes_missing_edge() {
            let mut g = fixture_graph();
            let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
            b.phantom_node_id_counter = 10;

            let mut c = GraphChange::new(&b);
            assert_eq!(c.restriction_as_cloned_nodes(&b.g, &[1, 2, 6]), None);
        }

        #[test]
        fn test_apply() {
            let mut g = fixture_graph();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 10;

                let mut c = GraphChange::new(&b);
                c.new_nodes.insert(11, 2);
                c.edges_to_add.insert(1, HashMap::from([(11, 200.0)]));
                c.edges_to_remove.insert((1, 2));
                c.edges_to_remove.insert((11, 5));
                c.phantom_node_id_counter = 11;
                c.apply(&mut b);

                assert_eq!(b.phantom_node_id_counter, 11);
            }

            assert_eq!(g.len(), 6);
            assert_eq!(g.get_node(1), Some(n!(1, 0.0, 0.0)));
            assert_eq!(g.get_node(2), Some(n!(2, 0.1, 0.0)));
            assert_eq!(g.get_node(3), Some(n!(3, 0.2, 0.0)));
            assert_eq!(g.get_node(4), Some(n!(4, 0.3, 0.0)));
            assert_eq!(g.get_node(5), Some(n!(5, 0.2, 0.1)));
            assert_eq!(g.get_node(11), Some(n!(11, 2, 0.1, 0.0)));

            assert_eq!(g.get_edges(1), &[e!(11, 200.0)]);
            assert_eq!(g.get_edges(2), &[e!(1, 200.0), e!(3, 200.0), e!(5, 100.0)]);
            assert_eq!(g.get_edges(3), &[e!(2, 200.0), e!(4, 200.0)]);
            assert_eq!(g.get_edges(4), &[e!(3, 200.0), e!(5, 100.0)]);
            assert_eq!(g.get_edges(5), &[e!(2, 100.0), e!(4, 100.0)]);
            assert_eq!(g.get_edges(11), &[e!(1, 200.0), e!(3, 200.0)]);
        }

        #[test]
        fn test_ensure_only_edge() {
            let mut g = fixture_graph();

            {
                let mut b = GraphBuilder::new(&mut g, &DEFAULT_OPTIONS);
                b.phantom_node_id_counter = 10;

                let mut c = GraphChange::new(&b);
                let cloned = c.restriction_as_cloned_nodes(&b.g, &[1, 2, 3, 4]).unwrap();
                assert_eq!(cloned, &[1, 11, 12, 4]);
                c.ensure_only_edge(&b.g, 11, 12);
                c.ensure_only_edge(&b.g, 12, 4);
                c.apply(&mut b);
            }

            assert_eq!(g.len(), 7);
            assert_eq!(g.get_node(1), Some(n!(1, 0.0, 0.0)));
            assert_eq!(g.get_node(2), Some(n!(2, 0.1, 0.0)));
            assert_eq!(g.get_node(3), Some(n!(3, 0.2, 0.0)));
            assert_eq!(g.get_node(4), Some(n!(4, 0.3, 0.0)));
            assert_eq!(g.get_node(5), Some(n!(5, 0.2, 0.1)));
            assert_eq!(g.get_node(11), Some(n!(11, 2, 0.1, 0.0)));
            assert_eq!(g.get_node(12), Some(n!(12, 3, 0.2, 0.0)));

            assert_eq!(g.get_edges(1), &[e!(11, 200.0)]);
            assert_eq!(g.get_edges(2), &[e!(1, 200.0), e!(3, 200.0), e!(5, 100.0)]);
            assert_eq!(g.get_edges(3), &[e!(2, 200.0), e!(4, 200.0)]);
            assert_eq!(g.get_edges(4), &[e!(3, 200.0), e!(5, 100.0)]);
            assert_eq!(g.get_edges(5), &[e!(2, 100.0), e!(4, 100.0)]);
            assert_eq!(g.get_edges(11), &[e!(12, 200.0)]);
            assert_eq!(g.get_edges(12), &[e!(4, 200.0)]);
        }
    }

    #[test]
    fn test_is_bbox_applicable() {
        assert_eq!(is_bbox_applicable([0.0, 0.0, 0.0, 0.0]), false);
        assert_eq!(is_bbox_applicable([0.0, 0.0, 1.0, 1.0]), true);
        assert_eq!(is_bbox_applicable([-1.0, -1.0, 1.0, 1.0]), true);

        assert_eq!(is_bbox_applicable([0.0, f32::NAN, 1.0, 1.0]), false);
        assert_eq!(is_bbox_applicable([0.0, 0.0, 1.0, -f32::INFINITY]), false);

        assert_eq!(is_bbox_applicable([0.0, 2.0, 1.0, 1.0]), false);
        assert_eq!(is_bbox_applicable([2.0, 0.0, 1.0, 1.0]), false);
    }
}
