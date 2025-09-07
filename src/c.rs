// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use super::*;

use std::borrow::Cow;
use std::collections::btree_map;
use std::ffi::{c_char, CStr, OsStr};
use std::mem::{forget, ManuallyDrop};
use std::os::unix::ffi::OsStrExt;
use std::ptr::null_mut;
use std::slice;

type CGraphIterator<'a> = btree_map::Values<'a, i64, (Node, Vec<Edge>)>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_new() -> *mut Graph {
    Box::into_raw(Box::<Graph>::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_delete(ptr: *mut Graph) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_get_nodes(
    graph: *const Graph,
    iterator_ptr: *mut *mut CGraphIterator<'_>,
) -> usize {
    if let Some(graph) = graph.as_ref() {
        if !iterator_ptr.is_null() {
            *iterator_ptr = Box::into_raw(Box::new(graph.0.values()));
        }

        graph.len()
    } else {
        if !iterator_ptr.is_null() {
            *iterator_ptr = null_mut();
        }

        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_iterator_next(iterator: *mut CGraphIterator<'_>) -> Node {
    if let Some(iterator) = iterator.as_mut() {
        if let Some((node, _)) = iterator.next() {
            return *node;
        }
    }

    Node::ZERO
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_iterator_delete(iterator: *mut CGraphIterator<'_>) {
    if !iterator.is_null() {
        drop(Box::from_raw(iterator));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_get_node(graph: *const Graph, id: i64) -> Node {
    graph
        .as_ref()
        .and_then(|g| g.get_node(id))
        .unwrap_or(Node::ZERO)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_set_node(graph: *mut Graph, node: Node) -> bool {
    if let Some(graph) = graph.as_mut() {
        graph.set_node(node)
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_delete_node(graph: *mut Graph, id: i64) -> bool {
    if let Some(graph) = graph.as_mut() {
        graph.delete_node(id)
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_find_nearest_node(
    graph: *const Graph,
    lat: f32,
    lon: f32,
) -> Node {
    graph
        .as_ref()
        .and_then(|g| g.find_nearest_node(lat, lon))
        .unwrap_or(Node::ZERO)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_get_edges(
    graph: *const Graph,
    from_id: i64,
    out_edges: *mut *const Edge,
) -> usize {
    if let Some(graph) = graph.as_ref() {
        let edges = graph.get_edges(from_id);
        if !out_edges.is_null() {
            *out_edges = edges.as_ptr();
        }

        edges.len()
    } else {
        if !out_edges.is_null() {
            *out_edges = null_mut();
        }

        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_get_edge(
    graph: *const Graph,
    from_id: i64,
    to_id: i64,
) -> f32 {
    graph
        .as_ref()
        .map(|g| g.get_edge(from_id, to_id))
        .unwrap_or(f32::INFINITY)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_set_edge(
    graph: *mut Graph,
    from_id: i64,
    edge: Edge,
) -> bool {
    if let Some(graph) = graph.as_mut() {
        graph.set_edge(from_id, edge)
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_delete_edge(
    graph: *mut Graph,
    from_id: i64,
    to_id: i64,
) -> bool {
    if let Some(graph) = graph.as_mut() {
        graph.delete_edge(from_id, to_id)
    } else {
        false
    }
}

#[repr(C)]
struct COsmProfilePenalty {
    key: *const c_char,
    value: *const c_char,
    penalty: f32,
}

#[repr(C)]
pub struct COsmProfile {
    name: *const c_char,

    penalties: *const COsmProfilePenalty,
    penalties_len: usize,

    access: *const *const c_char,
    access_len: usize,

    disallow_motorroad: bool,
    disable_restrictions: bool,
}

impl COsmProfile {
    /// Builds a buffer containing all strings referenced by this Profile.
    ///
    /// The layout of the buffer is as follows:
    /// - 0: name
    /// - 1..=access_len: access
    /// - access_len + 1..=access_len + penalties_len * 2: penalty keys and values
    unsafe fn build_string_table(&self) -> Vec<Cow<'_, str>> {
        let mut table = Vec::with_capacity(self.penalties_len * 2 + self.access_len + 1);

        // table[0]: profile name
        table.push(CStr::from_ptr(self.name).to_string_lossy());

        // table[1..=access_len]: access tags
        table.extend(
            slice::from_raw_parts(self.access, self.access_len)
                .iter()
                .map(|&access_cstr_ptr| CStr::from_ptr(access_cstr_ptr).to_string_lossy()),
        );

        // table[access_len + 1..=access_len + penalties_len * 2]: penalty keys and values
        table.extend(
            slice::from_raw_parts(self.penalties, self.penalties_len)
                .iter()
                .flat_map(|penalty| {
                    [
                        CStr::from_ptr(penalty.key).to_string_lossy(),
                        CStr::from_ptr(penalty.value).to_string_lossy(),
                    ]
                }),
        );

        table
    }

    unsafe fn penalties_as_rust<'a>(
        &self,
        string_table: &'a [Cow<'_, str>],
    ) -> Vec<osm::Penalty<'a>> {
        let string_table_offset = 1 + self.access_len;
        slice::from_raw_parts(self.penalties, self.penalties_len)
            .iter()
            .enumerate()
            .map(|(i, penalty)| {
                let string_table_index = string_table_offset + i * 2;
                osm::Penalty {
                    key: &string_table[string_table_index],
                    value: &string_table[string_table_index + 1],
                    penalty: penalty.penalty,
                }
            })
            .collect()
    }

    fn access_as_rust<'a>(&self, string_table: &'a [Cow<'_, str>]) -> Vec<&'a str> {
        string_table[1..=self.access_len]
            .iter()
            .map(|s| s.as_ref())
            .collect()
    }

    fn as_rust<'a>(
        &self,
        name: &'a str,
        penalties: &'a [osm::Penalty<'a>],
        access: &'a [&'a str],
    ) -> osm::Profile<'a> {
        osm::Profile {
            name,
            penalties,
            access,
            disallow_motorroad: self.disallow_motorroad,
            disable_restrictions: self.disable_restrictions,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum COsmFormat {
    Unknown = 0,
    Xml = 1,
    XmlGz = 2,
    XmlBz2 = 3,
    Pbf = 4,
}

impl From<COsmFormat> for osm::FileFormat {
    fn from(value: COsmFormat) -> Self {
        match value {
            COsmFormat::Unknown => osm::FileFormat::Unknown,
            COsmFormat::Xml => osm::FileFormat::Xml,
            COsmFormat::XmlGz => osm::FileFormat::XmlGz,
            COsmFormat::XmlBz2 => osm::FileFormat::XmlBz2,
            COsmFormat::Pbf => osm::FileFormat::Pbf,
        }
    }
}

#[repr(C)]
pub struct COsmOptions {
    pub profile: *const COsmProfile,
    pub format: COsmFormat,
    pub bbox: [f32; 4],
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_add_from_osm_file(
    graph: *mut Graph,
    c_options: *const COsmOptions,
    c_filename: *const c_char,
) {
    if let (Some(graph), Some(c_options), c_filename) = (
        graph.as_mut(),
        c_options.as_ref(),
        CStr::from_ptr(c_filename),
    ) {
        let c_profile = c_options
            .profile
            .as_ref()
            .expect("RoutexOsmOptions.profile must not be NULL");
        let profile_strings = c_profile.build_string_table();
        let profile_penalties = c_profile.penalties_as_rust(&profile_strings);
        let profile_access = c_profile.access_as_rust(&profile_strings);
        let profile = c_profile.as_rust(&profile_strings[0], &profile_penalties, &profile_access);
        let options = osm::Options {
            profile: &profile,
            file_format: c_options.format.into(),
            bbox: c_options.bbox,
        };

        let filename = OsStr::from_bytes(c_filename.to_bytes());

        // TODO: Log errors instead of ignoring them
        let _ = osm::add_features_from_file(graph, &options, filename);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_add_from_osm_memory(
    graph: *mut Graph,
    c_options: *const COsmOptions,
    content: *const u8,
    content_len: usize,
) {
    if let (Some(graph), Some(c_options)) = (graph.as_mut(), c_options.as_ref()) {
        let c_profile = c_options
            .profile
            .as_ref()
            .expect("RoutexOsmOptions.profile must not be NULL");
        let profile_strings = c_profile.build_string_table();
        let profile_penalties = c_profile.penalties_as_rust(&profile_strings);
        let profile_access = c_profile.access_as_rust(&profile_strings);
        let profile = c_profile.as_rust(&profile_strings[0], &profile_penalties, &profile_access);
        let options = osm::Options {
            profile: &profile,
            file_format: c_options.format.into(),
            bbox: c_options.bbox,
        };

        let content = std::slice::from_raw_parts(content, content_len);

        // TODO: Log errors instead of ignoring them
        let _ = osm::add_features_from_buffer(graph, &options, content);
    }
}

#[repr(C)]
pub enum CRouteResultType {
    Ok = 0,
    InvalidReference = 1,
    StepLimitExceeded = 2,
}

#[repr(C)]
pub struct CRouteResultOk {
    pub nodes: *mut i64,
    pub len: u32,
    pub capacity: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct CRouteResultInvalidReference {
    pub invalid_node_id: i64,
}

#[repr(C)]
pub union CRouteResultInner {
    pub ok: ManuallyDrop<CRouteResultOk>,
    pub invalid_reference: CRouteResultInvalidReference,
    pub empty: (),
}

#[repr(C)]
pub struct CRouteResult {
    pub inner: CRouteResultInner,
    pub type_: CRouteResultType,
}

impl CRouteResult {
    fn ok(mut nodes: Vec<i64>) -> Self {
        let ptr = nodes.as_mut_ptr();
        let len = nodes.len().try_into().expect("route length overflow");
        let capacity = nodes
            .capacity()
            .try_into()
            .expect("route capacity overflow");
        forget(nodes);

        CRouteResult {
            inner: CRouteResultInner {
                ok: ManuallyDrop::new(CRouteResultOk {
                    nodes: ptr,
                    len,
                    capacity,
                }),
            },
            type_: CRouteResultType::Ok,
        }
    }

    fn invalid_reference(invalid_node_id: i64) -> Self {
        CRouteResult {
            inner: CRouteResultInner {
                invalid_reference: CRouteResultInvalidReference { invalid_node_id },
            },
            type_: CRouteResultType::InvalidReference,
        }
    }

    fn empty() -> Self {
        CRouteResult {
            inner: CRouteResultInner { empty: () },
            type_: CRouteResultType::StepLimitExceeded,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_graph_find_route(
    graph: *const Graph,
    from_id: i64,
    to_id: i64,
    max_steps: usize,
) -> CRouteResult {
    if let Some(graph) = graph.as_ref() {
        match find_route(graph, from_id, to_id, max_steps) {
            Ok(nodes) => CRouteResult::ok(nodes),
            Err(astar::AStarError::InvalidReference(ref_)) => CRouteResult::invalid_reference(ref_),
            Err(astar::AStarError::StepLimitExceeded) => CRouteResult::empty(),
        }
    } else {
        CRouteResult {
            inner: CRouteResultInner {
                ok: ManuallyDrop::new(CRouteResultOk {
                    nodes: null_mut(),
                    len: 0,
                    capacity: 0,
                }),
            },
            type_: CRouteResultType::Ok,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_find_route_without_turn_around(
    graph: *const Graph,
    from_id: i64,
    to_id: i64,
    max_steps: usize,
) -> CRouteResult {
    if let Some(graph) = graph.as_ref() {
        match find_route_without_turn_around(graph, from_id, to_id, max_steps) {
            Ok(nodes) => CRouteResult::ok(nodes),
            Err(astar::AStarError::InvalidReference(ref_)) => CRouteResult::invalid_reference(ref_),
            Err(astar::AStarError::StepLimitExceeded) => CRouteResult::empty(),
        }
    } else {
        CRouteResult {
            inner: CRouteResultInner {
                ok: ManuallyDrop::new(CRouteResultOk {
                    nodes: null_mut(),
                    len: 0,
                    capacity: 0,
                }),
            },
            type_: CRouteResultType::Ok,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_route_result_delete(result: CRouteResult) {
    match result.type_ {
        CRouteResultType::Ok => {
            let ok = ManuallyDrop::into_inner(result.inner.ok);
            if !ok.nodes.is_null() {
                drop(Vec::from_raw_parts(
                    ok.nodes,
                    ok.len as usize,
                    ok.capacity as usize,
                ));
            }
        }

        CRouteResultType::InvalidReference => {
            // Nothing to free
        }

        CRouteResultType::StepLimitExceeded => {
            // Nothing to free
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_kd_tree_new(graph: *const Graph) -> *mut KDTree {
    if let Some(graph) = graph.as_ref() {
        if let Some(kd) = KDTree::build_from_graph(graph) {
            return Box::into_raw(Box::new(kd));
        }
    }

    null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_kd_tree_delete(ptr: *mut KDTree) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_kd_tree_find_nearest_node(
    kd_tree: *const KDTree,
    lat: f32,
    lon: f32,
) -> Node {
    kd_tree
        .as_ref()
        .and_then(|kd| Some(kd.find_nearest_node(lat, lon)))
        .unwrap_or(Node::ZERO)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn routex_earth_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    earth_distance(lat1, lon1, lat2, lon2)
}
