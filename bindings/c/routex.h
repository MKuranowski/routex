// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

#ifndef ROUTEX_H
#define ROUTEX_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Recommended A* step limit for routex_find_route() and routex_find_route_without_turn_around().
#define ROUTEX_DEFAULT_STEP_LIMIT 1000000

/**
 * An element of the @ref RoutexGraph.
 *
 * Due to turn restriction processing, one OpenStreetMap node
 * may be represented by multiple nodes in the graph. If that is the
 * case, a "canonical" node (not bound by any turn restrictions) will
 * have `id == osm_id`.
 *
 * Nodes with `id == 0` signify the absence of a node.
 */
typedef struct RoutexNode {
    int64_t id;
    int64_t osm_id;
    float lat;
    float lon;
} RoutexNode;

/**
 * Outgoing (one-way) connection from a @ref RoutexNode.
 *
 * `cost` must be greater than the crow-flies distance between the two nodes.
 */
typedef struct RoutexEdge {
    int64_t to;
    float cost;
} RoutexEdge;

/**
 * OpenStreetMap-based network representation as a set of @ref RoutexNode "RoutexNodes"
 * and @ref RoutexEdge "RoutexEdges" between them.
 */
typedef struct RoutexGraph RoutexGraph;

/**
 * Iterator over @ref RoutexNode "RoutexNodes" contained in a @ref RoutexGraph.
 */
typedef struct RoutexGraphIterator RoutexGraphIterator;

/**
 * Allocates a new, empty @ref RoutexGraph.
 *
 * Must be deallocated with routex_graph_delete().
 */
RoutexGraph* routex_graph_new(void);

/**
 * Deallocates a @ref RoutexGraph created by routex_graph_new(). The graph may be NULL.
 */
void routex_graph_delete(RoutexGraph* graph);

/**
 * Returns the number of @ref RoutexNode "RoutexNodes" in a Graph,
 * and (optionally) creates an iterator over them.
 *
 * The Graph must not be modified while any iterators are allocated.
 *
 * @param[in] g Graph to get the nodes of. May be NULL - in this case no nodes will be reported.
 * @param[out] it_ptr Optional (NULLable) destination of the returned opaque iterator. If not NULL, routex_graph_iterator_delete() must be called to deallocate the iterator.
 * @returns the number of nodes
 */
size_t routex_graph_get_nodes(RoutexGraph const* graph, RoutexGraphIterator** it_ptr);

/**
 * Advances a @ref RoutexGraphIterator "node iterator" and returns the next node.
 * A zero node (`id == 0`) will be returned to mark the end of iteration.
 *
 * The iterator may be NULL, in which case this function returns a zero node.
 */
RoutexNode routex_graph_iterator_next(RoutexGraphIterator* it);

/**
 * Deallocates a @ref RoutexGraphIterator created by routex_graph_get_nodes().
 *
 * May be called without exhausting the iterator, or with a NULL iterator.
 */
void routex_graph_iterator_delete(RoutexGraphIterator* it);

/**
 * Finds a node with the provided id. If no such node was found, returns a zero (`id == 0`) node.
 *
 * If the graph is NULL, returns a zero node.
 */
RoutexNode routex_graph_get_node(RoutexGraph const* graph, int64_t id);

/**
 * Creates or updates a @ref RoutexNode with the provided id.
 *
 * All outgoing and incoming edges are preserved, thus updating a @ref RoutexNode position
 * might result in violation of the @ref RoutexEdge invariant (and thus break route finding).
 * It **is discouraged** to update nodes, and it is the caller's responsibility not to break this invariant.
 *
 * When called with a NULL graph, this function does nothing and returns false.
 *
 * @returns true if an existing node was updated/overwritten, false otherwise
 */
bool routex_graph_set_node(RoutexGraph* graph, RoutexNode node);

/**
 * Deletes a @ref RoutexNode with the provided id.
 *
 * Outgoing edges are removed, but incoming edges are preserved (for performance reasons).
 * Thus, deleting a node and then reusing its id might result in violation of
 * @ref RoutexEdge cost invariant (breaking route finding) and **is therefore discouraged**.
 * It is the caller's responsibility not to break this invariant.
 *
 * When called with a NULL graph, this function does nothing and return false.
 *
 * @returns true if a node was actually deleted, false otherwise
 */
bool routex_graph_delete_node(RoutexGraph* graph, int64_t id);

/**
 * Finds the closest canonical (`id == osm_id`) @ref RoutexNode to the given position.
 *
 * This function requires computing distance to every @ref RoutexNode in the @ref RoutexGraph,
 * and is not suitable for large graphs or for multiple searches. Use @ref RoutexKDTree
 * (routex_kd_tree_new()) for faster NN finding.
 *
 * If the graph is NULL or has no nodes, returns a zero (`id == 0`) node.
 */
RoutexNode routex_graph_find_nearest_node(RoutexGraph const* graph, float lat, float lon);

/**
 * Gets all outgoing @ref RoutexEdge "RoutexEdges" from a node with a given id.
 *
 * The Graph must not be modified while using the return edge array, as it might be reallocated.
 *
 * @param[in] graph Graph to get the edges from. May be NULL - in this case no edges will be reported.
 * @param[in] from_id ID of the source node.
 * @param[out] edges_ptr Optional (NULLable) destination for the pointer to the array of edges. When there are no edges, this might be set to NULL or to a dangling pointer - such pointer must not be used.
 * @returns the number of edges
 */
size_t routex_graph_get_edges(RoutexGraph const* graph, int64_t from_id, RoutexEdge const** edges_ptr);

/**
 * Gets the cost of a @ref RoutexEdge from one node to another.
 * Returns positive infinity when the provided edge can't be found, or when the graph is NULL.
 */
float routex_graph_get_edge(RoutexGraph const* graph, int64_t from_id, int64_t to_id);

/**
 * Creates or updates a @ref RoutexEdge from a node with a given id.
 *
 * The `cost` must not be smaller than the crow-flies distance between nodes,
 * as this would violate the A* invariant and break route finding. It is the caller's
 * responsibility to do so.
 *
 * When called with a NULL graph, this function does nothing and returns false.
 *
 * @returns true if an existing edge was updated, false otherwise
 */
bool routex_graph_set_edge(RoutexGraph* graph, int64_t from_id, RoutexEdge edge);

/**
 * Removes a @ref RoutexEdge from one node to another.
 * If no such edge exists (or the graph is NULL), does nothing.
 *
 * @returns true if an edge was removed, false otherwise
 */
bool routex_graph_delete_edge(RoutexGraph* graph, int64_t from_id, int64_t to_id);

/**
 * Numeric multiplier for OSM ways with specific keys and values.
 */
typedef struct RoutexOsmProfilePenalty {
    /// Key of an OSM way for which this penalty applies,
    /// used for @ref RoutexOsmProfilePenalty::value "value" comparison (e.g. "highway" or "railway").
    char const* key;

    /// Value under @ref RoutexOsmProfilePenalty::key "key" of an OSM way for which this penalty applies.
    /// E.g. "motorway", "residential" or "rail".
    char const* value;

    /// Multiplier of the length, to express preference for a specific way.
    /// Must be not less than one and a finite floating-point number.
    float penalty;
} RoutexOsmProfilePenalty;

/**
 * Describes how to convert OSM data into a @ref RoutexGraph.
 */
typedef struct RoutexOsmProfile {
    /// Human readable name of the routing profile,
    /// customary the most specific [access tag](https://wiki.openstreetmap.org/wiki/Key:access).
    ///
    /// This values is not used for actual OSM data interpretation,
    /// except when set to "foot", which adds the following logic:
    /// - `oneway` tags are ignored - only `oneway:foot` tags are considered, except on:
    ///    - `highway=footway`,
    ///    - `highway=path`,
    ///    - `highway=steps`,
    ///    - `highway=platform`
    ///    - `public_transport=platform`,
    ///    - `railway=platform`;
    /// - only `restriction:foot` turn restrictions are considered.
    char const* name;

    /// Array of tags which OSM ways can be used for routing.
    ///
    /// A way is matched against all @ref RoutexOsmProfilePenalty objects in order, and
    /// once an exact key and value match is found; the way is used for routing,
    /// and each connection between two nodes gets a resulting cost equal
    /// to the distance between nodes multiplied the penalty.
    ///
    /// All penalties must be normal and not less than zero.
    ///
    /// For example, if there are two penalties:
    /// 1. highway=motorway, penalty=1
    /// 2. highway=trunk, penalty=1.5
    ///
    /// This will result in:
    /// - a highway=motorway stretch of 100 meters will be used for routing with a cost of 100.
    /// - a highway=trunk motorway of 100 meters will be used for routing with a cost of 150.
    /// - a highway=motorway_link or highway=primary won't be used for routing, as they do not
    ///   match any @ref RoutexOsmProfilePenalty.
    RoutexOsmProfilePenalty const* penalties;

    /// Length of the @ref RoutexOsmProfile::penalties "penalties" array.
    size_t penalties_len;

    /// Array of OSM [access tags](https://wiki.openstreetmap.org/wiki/Key:access#Land-based_transportation)
    /// (in order from least to most specific) to consider when checking for road prohibitions.
    ///
    /// This array is used mainly used to follow the access tags, but also to follow mode-specific
    /// one-way and turn restrictions.
    char const** access;

    /// Length of the @ref RoutexOsmProfile::access "access" array.
    size_t access_len;

    /// Force no routing over [motorroad=yes](https://wiki.openstreetmap.org/wiki/Key:motorroad) ways.
    bool disallow_motorroad;

    /// Force ignoring of [turn restrictions](https://wiki.openstreetmap.org/wiki/Turn_restriction).
    bool disable_restrictions;
} RoutexOsmProfile;

/**
 * Format of the input OSM file.
 */
typedef enum RoutexOsmFormat {
    /// Unknown format - guess the format based on the content
    RoutexOsmFormatUnknown = 0,

    /// Force uncompressed [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    RoutexOsmFormatXml = 1,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [gzip](https://en.wikipedia.org/wiki/Gzip) compression
    RoutexOsmFormatXmlGz = 2,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [bzip2](https://en.wikipedia.org/wiki/Bzip2) compression
    RoutexOsmFormatXmlBz2 = 3,

    /// Force [OSM PBF](https://wiki.openstreetmap.org/wiki/PBF_Format)
    RoutexOsmFormatPbf = 4,
} RoutexOsmFormat;

/**
 * Controls for interpreting OSM data as a routing @ref RoutexGraph.
 */
typedef struct RoutexOsmOptions {
    /// How OSM features should be interpreted, see @ref RoutexOsmProfile.
    RoutexOsmProfile const* profile;

    /// Format of the input OSM data, see @ref RoutexOsmFormat.
    RoutexOsmFormat file_format;

    /// Filter features by a specific bounding box. In order: left (min lon), bottom (min lat),
    /// right (max lon), top (max lat). Ignored if all values are set to zero.
    float bbox[4];
} RoutexOsmOptions;

/**
 * Parses OSM data from the provided file and adds it to the provided graph.
 *
 * @param graph Graph to which the OSM data will be added. If NULL, this function does nothing and returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param filename Path to the OSM file to be parsed. Must not be NULL.
 * @returns true if an error occurred, false otherwise
 */
bool routex_graph_add_from_osm_file(RoutexGraph* graph, RoutexOsmOptions const* options, char const* filename);

/**
 * Parses OSM data from the provided buffer and adds it to the provided graph.
 *
 * @param graph Graph to which the OSM data will be added. If NULL, this function does nothing and returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param content Pointer to the buffer containing OSM data. Must be not be NULL, even if content_len == 0.
 * @param content_len Length of the buffer in bytes.
 * @returns true if an error occurred, false otherwise
 */
bool routex_graph_add_from_osm_memory(RoutexGraph* graph, RoutexOsmOptions const* options, unsigned char const* content, size_t content_len);

/**
 * High-level route search status, also used as the tag for the anonymous union in @ref RoutexRouteResult.
 */
typedef enum RoutexRouteResultType {
    /// The search was successful.
    RoutexRouteResultTypeOk = 0,

    /// `from` or `to` nodes do not exist in the graph.
    RoutexRouteResultTypeInvalidReference = 1,

    /// Search exceeded its step limit. Either the nodes are really far apart, or no route exists.
    ///
    /// Concluding that no route exists requires traversing the whole graph, which can result in a denial-of-service.
    /// The step limit protects against resource exhaustion.
    RoutexRouteResultTypeStepLimitExceeded = 2,
} RoutexRouteResultType;

typedef struct RoutexRouteResult {
    union {
        /**
         * A list of node, returned as a result of successful route search.
         *
         * Valid if and only if `type` is set to @ref RoutexRouteResultTypeOk.
         */
        struct {
            /// Sequence of nodes of the route.
            /// If `len == 0`, this might be set to NULL or to a dangling pointer - such pointer must not be used.
            int64_t* nodes;

            /// Length of the route.
            uint32_t len;

            /// Capacity of the `nodes` array; used for internal bookkeeping.
            uint32_t capacity;
        } as_ok;

        /**
         * The `from` or `to` parameter of find_route()/find_route_without_turn_around() does not
         * exist in the graph.
         *
         * Valid if and only if `type` is set to @ref RoutexRouteResultTypeInvalidReference.
         */
        struct {
            /// ID of the non-existing node
            int64_t invalid_node_id;
        } as_invalid_reference;
    };

    /**
     * Indicates the overall outcome of routex_find_route()/find_route_without_turn_around():
     * - @ref RoutexRouteResultTypeOk - the search was successful, use `as_ok`.
     * - @ref RoutexRouteResultTypeInvalidReference - `from` or `to` do not exist in the graph, use `as_invalid_reference`.
     * - @ref RoutexRouteResultTypeStepLimitExceeded - search exceeded its step limit. Either the nodes are really far apart, or no route exists.
     *    Concluding that no route exists requires traversing the whole graph, which can result in a denial-of-service.
     *    The step limit protects against resource exhaustion.
     */
    RoutexRouteResultType type;
} RoutexRouteResult;

/**
 * Finds the shortest route between two nodes using the [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm).
 * in the provided graph.
 *
 * The returned result must be destroyed by calling routex_route_result_delete().
 *
 * Returns an @ref RoutexRouteResultTypeOk "ok result" with an empty vector if no route exists.
 *
 * For graphs with turn restrictions, use routex_find_route_without_turn_around(), as this implementation
 * will generate unrealistic instructions with immediate turnarounds (A-B-A) to circumvent any restrictions.
 *
 * The `step_limit` parameter limits how many nodes can be expanded during search before returning
 * @ref RoutexRouteResultTypeStepLimitExceeded "step limit exceeded". Concluding that no route exists
 * requires expanding all nodes accessible from the start, which is usually very time consuming,
 * especially on large datasets. Recommended value is @ref ROUTEX_DEFAULT_STEP_LIMIT.
 */
RoutexRouteResult routex_find_route(RoutexGraph const* graph, int64_t from, int64_t to, size_t step_limit);

/**
 * Finds the shortest route between two nodes using the [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm).
 * in the provided graph.
 *
 * The returned result must be destroyed by calling routex_route_result_delete().
 *
 * Returns an @ref RoutexRouteResultTypeOk "ok result" with an empty vector if no route exists.
 *
 * For graphs without turn restrictions, use routex_find_route(), as it runs faster.
 * This function has an extra dimension - it needs to not only consider the current node,
 * but also what was the previous node to prevent immediate turnaround (A-B-A) instructions.
 *
 * The `step_limit` parameter limits how many nodes can be expanded during search before returning
 * @ref RoutexRouteResultTypeStepLimitExceeded "step limit exceeded". Concluding that no route exists
 * requires expanding all nodes accessible from the start, which is usually very time consuming,
 * especially on large datasets. Recommended value is @ref ROUTEX_DEFAULT_STEP_LIMIT.
 */
RoutexRouteResult routex_find_route_without_turn_around(RoutexGraph const* graph, int64_t from, int64_t to, size_t step_limit);

/**
 * Deallocates a @ref RoutexRouteResult created by routex_find_route() or routex_find_route_without_turn_around().
 */
void routex_route_result_delete(RoutexRouteResult);

/**
 * A [k-d tree data structure](https://en.wikipedia.org/wiki/K-d_tree) which can be used to
 * speed up nearest-neighbor search for large datasets.
 *
 * Practice shows that routex_graph_find_nearest_node() takes significantly more time than
 * routex_find_route() when generating multiple routes with routex. A k-d tree helps with that,
 * trading CPU time for memory usage.
 */
typedef struct RoutexKDTree RoutexKDTree;

/**
 * Builds a @ref RoutexKDTree with all canonical (`id == osm_id`) @ref RoutexNode "RoutexNodes"
 * contained in the provided @ref RoutexGraph.
 *
 * Must be deallocated with routex_kd_tree_delete().
 *
 * Returns NULL if the graph has no nodes.
 */
RoutexKDTree* routex_kd_tree_new(RoutexGraph const*);

/**
 * Deallocates a @ref RoutexKDTree created by routex_kd_tree_new(). The k-d tree may be NULL.
 */
void routex_kd_tree_delete(RoutexKDTree*);

/**
 * Finds the closest node to the provided position and returns its id.
 * If there are no nodes or the k-d tree is NULL, returns 0.
 */
int64_t routex_kd_tree_find_nearest_node(RoutexKDTree const* kd_tree, float lat, float lon);

/**
 * Calculates the great-circle distance between two positions using the [haversine formula](https://en.wikipedia.org/wiki/Haversine_formula).
 * Returns the result in kilometers.
 */
float routex_earth_distance(float lat1, float lon1, float lat2, float lon2);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // ROUTEX_H
