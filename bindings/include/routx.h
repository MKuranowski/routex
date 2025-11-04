// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

#ifndef ROUTX_H
#define ROUTX_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Recommended A* step limit for routx_find_route() and routx_find_route_without_turn_around().
#define ROUTX_DEFAULT_STEP_LIMIT 1000000

/**
 * Sets a logging handler for the library.
 *
 * The `callback` function will be called whenever the library wants to log something.
 * Routx makes two types of logs:
 *
 * - warnings with target=`routx::osm`, informing about issues with OSM data;
 *
 * - errors with target=`routx`, informing about input issues or failures within the library.
 *
 * Dependencies of routx technically could also make any logging calls, but they don't
 * seem to do so.
 *
 * The logging levels numbers generally correspond to [Python Logging
 * Levels](https://docs.python.org/3/library/logging.html#logging-levels), that is:
 *
 * - 50 for critical failures (not used in Rust's log crate),
 *
 * - 40 for errors,
 *
 * - 30 for warnings,
 *
 * - 20 for info,
 *
 * - 10 for debug,
 *
 * - 5 for trace.
 *
 * This function leaks a small amount of memory allocated for the logging adapter.
 * It is advisable to only call this function once.
 *
 * @param callback function to call on a logging message, or NULL to disable logging.
 *     `arg` parameter is passed through as-is, `level` represents message severity (described
 * above), `target` describes briefly who made the logging call (e.g. `routx`), and `message` is
 * the actual log message.
 * @param flush_callback function to call when the library wants to flush any buffered log
 * messages, or NULL if no flushing is needed. `arg` parameter is passed through as-is. Currently
 * not used.
 * @param arg user-provided argument passed to `callback` and `flush_callback` calls
 * @param min_level minimum logging level to report. Messages with a lower level will be ignored.
 *    Recommended value is 30 to only see warnings and errors.
 */
void routx_set_logging_callback(void (*callback)(void* arg, int level, char const* target,
                                                 char const* message),
                                void (*flush_callback)(void* arg), void* arg, int min_level);

/**
 * An element of the @ref RoutxGraph.
 *
 * Due to turn restriction processing, one OpenStreetMap node
 * may be represented by multiple nodes in the graph. If that is the
 * case, a "canonical" node (not bound by any turn restrictions) will
 * have `id == osm_id`.
 *
 * Nodes with `id == 0` signify the absence of a node.
 */
typedef struct RoutxNode {
    int64_t id;
    int64_t osm_id;
    float lat;
    float lon;
} RoutxNode;

/**
 * Outgoing (one-way) connection from a @ref RoutxNode.
 *
 * `cost` must be greater than the crow-flies distance between the two nodes.
 */
typedef struct RoutxEdge {
    int64_t to;
    float cost;
} RoutxEdge;

/**
 * OpenStreetMap-based network representation as a set of @ref RoutxNode "RoutxNodes"
 * and @ref RoutxEdge "RoutxEdges" between them.
 */
typedef struct RoutxGraph RoutxGraph;

/**
 * Iterator over @ref RoutxNode "RoutxNodes" contained in a @ref RoutxGraph.
 */
typedef struct RoutxGraphIterator RoutxGraphIterator;

/**
 * Allocates a new, empty @ref RoutxGraph.
 *
 * Must be deallocated with routx_graph_delete().
 */
RoutxGraph* routx_graph_new(void);

/**
 * Deallocates a @ref RoutxGraph created by routx_graph_new(). The graph may be NULL.
 */
void routx_graph_delete(RoutxGraph* graph);

/**
 * Returns the number of @ref RoutxNode "RoutxNodes" in a Graph,
 * and (optionally) creates an iterator over them.
 *
 * The Graph must not be modified while any iterators are allocated.
 *
 * @param[in] g Graph to get the nodes of. May be NULL - in this case no nodes will be reported.
 * @param[out] it_ptr Optional (NULLable) destination of the returned opaque iterator. If not NULL,
 * routx_graph_iterator_delete() must be called to deallocate the iterator.
 * @returns the number of nodes
 */
size_t routx_graph_get_nodes(RoutxGraph const* graph, RoutxGraphIterator** it_ptr);

/**
 * Advances a @ref RoutxGraphIterator "node iterator" and returns the next node.
 * A zero node (`id == 0`) will be returned to mark the end of iteration.
 *
 * The iterator may be NULL, in which case this function returns a zero node.
 */
RoutxNode routx_graph_iterator_next(RoutxGraphIterator* it);

/**
 * Deallocates a @ref RoutxGraphIterator created by routx_graph_get_nodes().
 *
 * May be called without exhausting the iterator, or with a NULL iterator.
 */
void routx_graph_iterator_delete(RoutxGraphIterator* it);

/**
 * Finds a node with the provided id. If no such node was found, returns a zero (`id == 0`) node.
 *
 * If the graph is NULL, returns a zero node.
 */
RoutxNode routx_graph_get_node(RoutxGraph const* graph, int64_t id);

/**
 * Creates or updates a @ref RoutxNode with the provided id.
 *
 * All outgoing and incoming edges are preserved, thus updating a @ref RoutxNode position
 * might result in violation of the @ref RoutxEdge invariant (and thus break route finding).
 * It **is discouraged** to update nodes, and it is the caller's responsibility not to break this
 * invariant.
 *
 * When called with a NULL graph, this function does nothing and returns false.
 *
 * @returns true if an existing node was updated/overwritten, false otherwise
 */
bool routx_graph_set_node(RoutxGraph* graph, RoutxNode node);

/**
 * Deletes a @ref RoutxNode with the provided id.
 *
 * Outgoing edges are removed, but incoming edges are preserved (for performance reasons).
 * Thus, deleting a node and then reusing its id might result in violation of
 * @ref RoutxEdge cost invariant (breaking route finding) and **is therefore discouraged**.
 * It is the caller's responsibility not to break this invariant.
 *
 * When called with a NULL graph, this function does nothing and return false.
 *
 * @returns true if a node was actually deleted, false otherwise
 */
bool routx_graph_delete_node(RoutxGraph* graph, int64_t id);

/**
 * Finds the closest canonical (`id == osm_id`) @ref RoutxNode to the given position.
 *
 * This function requires computing distance to every @ref RoutxNode in the @ref RoutxGraph,
 * and is not suitable for large graphs or for multiple searches. Use @ref RoutxKDTree
 * (routx_kd_tree_new()) for faster NN finding.
 *
 * If the graph is NULL or has no nodes, returns a zero (`id == 0`) node.
 */
RoutxNode routx_graph_find_nearest_node(RoutxGraph const* graph, float lat, float lon);

/**
 * Gets all outgoing @ref RoutxEdge "RoutxEdges" from a node with a given id.
 *
 * The Graph must not be modified while using the return edge array, as it might be reallocated.
 *
 * @param[in] graph Graph to get the edges from. May be NULL - in this case no edges will be
 * reported.
 * @param[in] from_id ID of the source node.
 * @param[out] edges_ptr Optional (NULLable) destination for the pointer to the array of edges.
 * When there are no edges, this might be set to NULL or to a dangling pointer - such pointer must
 * not be used.
 * @returns the number of edges
 */
size_t routx_graph_get_edges(RoutxGraph const* graph, int64_t from_id,
                             RoutxEdge const** edges_ptr);

/**
 * Gets the cost of a @ref RoutxEdge from one node to another.
 * Returns positive infinity when the provided edge can't be found, or when the graph is NULL.
 */
float routx_graph_get_edge(RoutxGraph const* graph, int64_t from_id, int64_t to_id);

/**
 * Creates or updates a @ref RoutxEdge from a node with a given id.
 *
 * The `cost` must not be smaller than the crow-flies distance between nodes,
 * as this would violate the A* invariant and break route finding. It is the caller's
 * responsibility to do so.
 *
 * When called with a NULL graph, this function does nothing and returns false.
 *
 * @returns true if an existing edge was updated, false otherwise
 */
bool routx_graph_set_edge(RoutxGraph* graph, int64_t from_id, RoutxEdge edge);

/**
 * Removes a @ref RoutxEdge from one node to another.
 * If no such edge exists (or the graph is NULL), does nothing.
 *
 * @returns true if an edge was removed, false otherwise
 */
bool routx_graph_delete_edge(RoutxGraph* graph, int64_t from_id, int64_t to_id);

/**
 * Numeric multiplier for OSM ways with specific keys and values.
 */
typedef struct RoutxOsmProfilePenalty {
    /// Key of an OSM way for which this penalty applies,
    /// used for @ref RoutxOsmProfilePenalty::value "value" comparison (e.g. "highway" or
    /// "railway").
    char const* key;

    /// Value under @ref RoutxOsmProfilePenalty::key "key" of an OSM way for which this penalty
    /// applies. E.g. "motorway", "residential" or "rail".
    char const* value;

    /// Multiplier of the length, to express preference for a specific way.
    /// Must be not less than one and a finite floating-point number.
    float penalty;
} RoutxOsmProfilePenalty;

/**
 * Describes how to convert OSM data into a @ref RoutxGraph.
 */
typedef struct RoutxOsmProfile {
    /// Human readable name of the routing profile,
    /// customary the most specific [access tag](https://wiki.openstreetmap.org/wiki/Key:access).
    ///
    /// This value is not used for actual OSM data interpretation,
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
    /// A way is matched against all @ref RoutxOsmProfilePenalty objects in order, and
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
    ///   match any @ref RoutxOsmProfilePenalty.
    RoutxOsmProfilePenalty const* penalties;

    /// Length of the @ref RoutxOsmProfile::penalties "penalties" array.
    size_t penalties_len;

    /// Array of OSM
    /// [access tags](https://wiki.openstreetmap.org/wiki/Key:access#Land-based_transportation)
    /// (in order from least to most specific) to consider when checking for road prohibitions.
    ///
    /// This array is used mainly used to follow the access tags, but also to follow mode-specific
    /// one-way and turn restrictions.
    char const** access;

    /// Length of the @ref RoutxOsmProfile::access "access" array.
    size_t access_len;

    /// Force no routing over [motorroad=yes](https://wiki.openstreetmap.org/wiki/Key:motorroad)
    /// ways.
    bool disallow_motorroad;

    /// Force ignoring of
    /// [turn restrictions](https://wiki.openstreetmap.org/wiki/Turn_restriction).
    bool disable_restrictions;
} RoutxOsmProfile;

/**
 * Car routing profile.
 *
 * Penalties:
 *
 * | Tag                    | Penalty |
 * |------------------------|---------|
 * | highway=motorway       | 1.0     |
 * | highway=motorway_link  | 1.0     |
 * | highway=trunk          | 2.0     |
 * | highway=trunk_link     | 2.0     |
 * | highway=primary        | 5.0     |
 * | highway=primary_link   | 5.0     |
 * | highway=secondary      | 6.5     |
 * | highway=secondary_link | 6.5     |
 * | highway=tertiary       | 10.0    |
 * | highway=tertiary_link  | 10.0    |
 * | highway=unclassified   | 10.0    |
 * | highway=minor          | 10.0    |
 * | highway=residential    | 15.0    |
 * | highway=living_street  | 20.0    |
 * | highway=track          | 20.0    |
 * | highway=service        | 20.0    |
 *
 * Access tags: `access`, `vehicle`, `motor_vehicle`, `motorcar`.
 *
 * Allows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_CAR ((RoutxOsmProfile const*)1)

/**
 * Bus routing profile.
 *
 * Penalties:
 *
 * | Tag                    | Penalty |
 * |------------------------|---------|
 * | highway=motorway       | 1.0     |
 * | highway=motorway_link  | 1.0     |
 * | highway=trunk          | 1.0     |
 * | highway=trunk_link     | 1.0     |
 * | highway=primary        | 1.1     |
 * | highway=primary_link   | 1.1     |
 * | highway=secondary      | 1.15    |
 * | highway=secondary_link | 1.15    |
 * | highway=tertiary       | 1.15    |
 * | highway=tertiary_link  | 1.15    |
 * | highway=unclassified   | 1.5     |
 * | highway=minor          | 1.5     |
 * | highway=residential    | 2.5     |
 * | highway=living_street  | 2.5     |
 * | highway=track          | 5.0     |
 * | highway=service        | 5.0     |
 *
 * Access tags: `access`, `vehicle`, `motor_vehicle`, `psv`, `bus`, `routing:ztm`.
 *
 * Allows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_BUS ((RoutxOsmProfile const*)2)

/**
 * Bicycle routing profile.
 *
 * Penalties:
 *
 * | Tag                    | Penalty |
 * |------------------------|---------|
 * | highway=trunk          | 50.0    |
 * | highway=trunk_link     | 50.0    |
 * | highway=primary        | 10.0    |
 * | highway=primary_link   | 10.0    |
 * | highway=secondary      | 3.0     |
 * | highway=secondary_link | 3.0     |
 * | highway=tertiary       | 2.5     |
 * | highway=tertiary_link  | 2.5     |
 * | highway=unclassified   | 2.5     |
 * | highway=minor          | 2.5     |
 * | highway=cycleway       | 1.0     |
 * | highway=residential    | 1.0     |
 * | highway=living_street  | 1.5     |
 * | highway=track          | 2.0     |
 * | highway=service        | 2.0     |
 * | highway=bridleway      | 3.0     |
 * | highway=footway        | 3.0     |
 * | highway=steps          | 5.0     |
 * | highway=path           | 2.0     |
 *
 * Access tags: `access`, `vehicle`, `bicycle`.
 *
 * Disallows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_BICYCLE ((RoutxOsmProfile const*)3)

/**
 * Pedestrian routing profile.
 *
 * Penalties:
 *
 * | Tag                       | Penalty |
 * |---------------------------|---------|
 * | highway=trunk             | 4.0     |
 * | highway=trunk_link        | 4.0     |
 * | highway=primary           | 2.0     |
 * | highway=primary_link      | 2.0     |
 * | highway=secondary         | 1.3     |
 * | highway=secondary_link    | 1.3     |
 * | highway=tertiary          | 1.2     |
 * | highway=tertiary_link     | 1.2     |
 * | highway=unclassified      | 1.2     |
 * | highway=minor             | 1.2     |
 * | highway=residential       | 1.2     |
 * | highway=living_street     | 1.2     |
 * | highway=track             | 1.2     |
 * | highway=service           | 1.2     |
 * | highway=bridleway         | 1.2     |
 * | highway=footway           | 1.05    |
 * | highway=path              | 1.05    |
 * | highway=steps             | 1.15    |
 * | highway=pedestrian        | 1.0     |
 * | highway=platform          | 1.1     |
 * | railway=platform          | 1.1     |
 * | public_transport=platform | 1.1     |
 *
 * Access tags: `access`, `foot`.
 *
 * Disallows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad).
 *
 * One-way is only considered when explicitly tagged with `oneway:foot` or on
 * `highway=footway`, `highway=path`, `highway=steps`, `highway/public_transport/railway=platform`.
 *
 * Turn restrictions are only considered when explicitly tagged with `restriction:foot`.
 */
#define ROUTX_OSM_PROFILE_FOOT ((RoutxOsmProfile const*)4)

/**
 * Railway routing profile.
 *
 * Penalties:
 *
 * | Tag                  | Penalty |
 * |----------------------|---------|
 * | railway=rail         | 1.0     |
 * | railway=light_rail   | 1.0     |
 * | railway=subway       | 1.0     |
 * | railway=narrow_gauge | 1.0     |
 *
 * Access tags: `access`, `train`.
 *
 * Allows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_RAILWAY ((RoutxOsmProfile const*)5)

/**
 * Tram and light rail routing profile.
 *
 * Penalties:
 *
 * | Tag                  | Penalty |
 * |----------------------|---------|
 * | railway=tram         | 1.0     |
 * | railway=light_rail   | 1.0     |
 *
 * Access tags: `access`, `tram`.
 *
 * Allows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_TRAM ((RoutxOsmProfile const*)6)

/**
 * Subway routing profile.
 *
 * Penalties:
 *
 * | Tag            | Penalty |
 * |----------------|---------|
 * | railway=subway | 1.0     |
 *
 * Access tags: `access`, `subway`.
 *
 * Allows [motorroads](https://wiki.openstreetmap.org/wiki/Key:motorroad) and considers turn
 * restrictions.
 */
#define ROUTX_OSM_PROFILE_SUBWAY ((RoutxOsmProfile const*)7)

/**
 * Format of the input OSM file.
 */
typedef enum RoutxOsmFormat {
    /// Unknown format - guess the format based on the content
    RoutxOsmFormatUnknown = 0,

    /// Force uncompressed [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    RoutxOsmFormatXml = 1,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [gzip](https://en.wikipedia.org/wiki/Gzip) compression
    RoutxOsmFormatXmlGz = 2,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [bzip2](https://en.wikipedia.org/wiki/Bzip2) compression
    RoutxOsmFormatXmlBz2 = 3,

    /// Force [OSM PBF](https://wiki.openstreetmap.org/wiki/PBF_Format)
    RoutxOsmFormatPbf = 4,
} RoutxOsmFormat;

/**
 * Controls for interpreting OSM data as a routing @ref RoutxGraph.
 */
typedef struct RoutxOsmOptions {
    /// How OSM features should be interpreted, see @ref RoutxOsmProfile.
    ///
    /// Must not be NULL. Due to C-Rust String/Vector/Slice interface mismatch,
    /// there is overhead with profile conversion.
    ///
    /// For this reason, pointer numeric values from 1 to 64 (incl.) are reserved
    /// for built-in profiles, which do not require conversion and are faster to use.
    /// Use the ROUTX_OSM_PROFILE_* macros to refer to them.
    RoutxOsmProfile const* profile;

    /// Format of the input OSM data, see @ref RoutxOsmFormat.
    RoutxOsmFormat file_format;

    /// Filter features by a specific bounding box. In order: left (min lon), bottom (min lat),
    /// right (max lon), top (max lat). Ignored if all values are set to zero.
    float bbox[4];
} RoutxOsmOptions;

/**
 * Parses OSM data from the provided file and adds it to the provided graph.
 *
 * @param graph Graph to which the OSM data will be added. If NULL, this function does nothing and
 * returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param filename Path to the OSM file to be parsed. Must not be NULL.
 * @returns false if an error occurred, true otherwise
 */
bool routx_graph_add_from_osm_file(RoutxGraph* graph, RoutxOsmOptions const* options,
                                   char const* filename);

/**
 * Parses OSM data from the provided buffer and adds it to the provided graph.
 *
 * @param graph Graph to which the OSM data will be added. If NULL, this function does nothing and
 * returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param content Pointer to the buffer containing OSM data. Must be not be NULL, even if
 * content_len == 0.
 * @param content_len Length of the buffer in bytes.
 * @returns false if an error occurred, true otherwise
 */
bool routx_graph_add_from_osm_memory(RoutxGraph* graph, RoutxOsmOptions const* options,
                                     char const* content, size_t content_len);

/**
 * High-level route search status, also used as the tag for the anonymous union in
 * @ref RoutxRouteResult.
 */
typedef enum RoutxRouteResultType {
    /// The search was successful.
    RoutxRouteResultTypeOk = 0,

    /// `from` or `to` nodes do not exist in the graph.
    RoutxRouteResultTypeInvalidReference = 1,

    /// Search exceeded its step limit. Either the nodes are really far apart, or no route exists.
    ///
    /// Concluding that no route exists requires traversing the whole graph, which can result in a
    /// denial-of-service.
    /// The step limit protects against resource exhaustion.
    RoutxRouteResultTypeStepLimitExceeded = 2,
} RoutxRouteResultType;

/**
 * A list of nodes, returned as a result of successful route search.
 */
typedef struct RoutxRouteResultOk {
    /// Sequence of nodes of the route.
    /// If `len == 0`, this might be set to NULL or to a dangling pointer - such pointer
    /// must not be used.
    int64_t* nodes;

    /// Length of the route.
    uint32_t len;

    /// Capacity of the `nodes` array; used for internal bookkeeping.
    uint32_t capacity;
} RoutxRouteResultAsOk;

/**
 * find_route called with a reference to a non-existing note.
 */
typedef struct RoutxRouteResultInvalidReference {
    /// ID of the non-existing node
    int64_t invalid_node_id;
} RoutxRouteResultInvalidReference;

typedef struct RoutxRouteResult {
    union {
        /// List of nodes, valid if and only if `type` is set to @ref RoutxRouteResultTypeOk.
        RoutxRouteResultAsOk as_ok;

        /// Reference to a non-existing node, valid if and only if `type` is set to
        /// @ref RoutxRouteResultTypeInvalidReference.
        RoutxRouteResultInvalidReference as_invalid_reference;
    };

    /**
     * Indicates the overall outcome of routx_find_route()/
     * routx_find_route_without_turn_around():
     * - @ref RoutxRouteResultTypeOk - the search was successful, use `as_ok`.
     * - @ref RoutxRouteResultTypeInvalidReference - `from` or `to` do not exist in the graph, use
     * `as_invalid_reference`.
     * - @ref RoutxRouteResultTypeStepLimitExceeded - search exceeded its step limit. Either the
     * nodes are really far apart, or no route exists. Concluding that no route exists requires
     * traversing the whole graph, which can result in a denial-of-service. The step limit protects
     * against resource exhaustion.
     */
    RoutxRouteResultType type;
} RoutxRouteResult;

/**
 * Finds the shortest route between two nodes using the
 * [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm) in the provided graph.
 *
 * The returned result must be destroyed by calling routx_route_result_delete().
 *
 * Returns an @ref RoutxRouteResultTypeOk "ok result" with an empty vector if no route exists.
 *
 * For graphs with turn restrictions, use routx_find_route_without_turn_around(), as this
 * implementation will generate unrealistic instructions with immediate turnarounds (A-B-A) to
 * circumvent any restrictions.
 *
 * The `step_limit` parameter limits how many nodes can be expanded during search before returning
 * @ref RoutxRouteResultTypeStepLimitExceeded "step limit exceeded". Concluding that no route
 * exists requires expanding all nodes accessible from the start, which is usually very time
 * consuming, especially on large datasets. Recommended value is @ref ROUTX_DEFAULT_STEP_LIMIT.
 */
RoutxRouteResult routx_find_route(RoutxGraph const* graph, int64_t from, int64_t to,
                                  size_t step_limit);

/**
 * Finds the shortest route between two nodes using the
 * [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm). in the provided graph.
 *
 * The returned result must be destroyed by calling routx_route_result_delete().
 *
 * Returns an @ref RoutxRouteResultTypeOk "ok result" with an empty vector if no route exists.
 *
 * For graphs without turn restrictions, use routx_find_route(), as it runs faster.
 * This function has an extra dimension - it needs to not only consider the current node,
 * but also what was the previous node to prevent immediate turnaround (A-B-A) instructions.
 *
 * The `step_limit` parameter limits how many nodes can be expanded during search before returning
 * @ref RoutxRouteResultTypeStepLimitExceeded "step limit exceeded". Concluding that no route
 * exists requires expanding all nodes accessible from the start, which is usually very time
 * consuming, especially on large datasets. Recommended value is @ref ROUTX_DEFAULT_STEP_LIMIT.
 */
RoutxRouteResult routx_find_route_without_turn_around(RoutxGraph const* graph, int64_t from,
                                                      int64_t to, size_t step_limit);

/**
 * Deallocates a @ref RoutxRouteResult created by routx_find_route() or
 * routx_find_route_without_turn_around().
 */
void routx_route_result_delete(RoutxRouteResult);

/**
 * A [k-d tree data structure](https://en.wikipedia.org/wiki/K-d_tree) which can be used to
 * speed up nearest-neighbor search for large datasets.
 *
 * Practice shows that routx_graph_find_nearest_node() takes significantly more time than
 * routx_find_route() when generating multiple routes with routx. A k-d tree helps with that,
 * trading CPU time for memory usage.
 */
typedef struct RoutxKDTree RoutxKDTree;

/**
 * Builds a @ref RoutxKDTree with all canonical (`id == osm_id`) @ref RoutxNode "RoutxNodes"
 * contained in the provided @ref RoutxGraph.
 *
 * Must be deallocated with routx_kd_tree_delete().
 *
 * Returns NULL if the graph has no nodes.
 */
RoutxKDTree* routx_kd_tree_new(RoutxGraph const*);

/**
 * Deallocates a @ref RoutxKDTree created by routx_kd_tree_new(). The k-d tree may be NULL.
 */
void routx_kd_tree_delete(RoutxKDTree*);

/**
 * Finds the closest node to the provided position and returns it.
 * If there are no nodes or the k-d tree is NULL, returns a zero (`id == 0`) node.
 */
RoutxNode routx_kd_tree_find_nearest_node(RoutxKDTree const* kd_tree, float lat, float lon);

/**
 * Calculates the great-circle distance between two positions using the
 * [haversine formula](https://en.wikipedia.org/wiki/Haversine_formula).
 * Returns the result in kilometers.
 */
float routx_earth_distance(float lat1, float lon1, float lat2, float lon2);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // ROUTX_H
