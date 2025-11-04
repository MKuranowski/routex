// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

#ifndef ROUTX_HPP
#define ROUTX_HPP

#include <routx.h>

#include <cstddef>
#include <cstdint>
#include <iterator>
#include <optional>
#include <span>
#include <stdexcept>
#include <utility>
#include <variant>

namespace routx {

/// Recommended A* step limit for Graph::find_route() and Graph::find_route_without_turn_around().
constexpr size_t DEFAULT_STEP_LIMIT = ROUTX_DEFAULT_STEP_LIMIT;

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
void set_logging_callback(void (*callback)(void* arg, int level, char const* target,
                                           char const* message),
                          void (*flush_callback)(void* arg), void* arg, int min_level) {
    return routx_set_logging_callback(callback, flush_callback, arg, min_level);
}

/**
 * Calculates the great-circle distance between two positions using the
 * [haversine formula](https://en.wikipedia.org/wiki/Haversine_formula).
 * Returns the result in kilometers.
 */
float earth_distance(float lat1, float lon1, float lat2, float lon2) {
    return routx_earth_distance(lat1, lon1, lat2, lon2);
}

/**
 * An element of the @ref Graph.
 *
 * Due to turn restriction processing, one OpenStreetMap node
 * may be represented by multiple nodes in the graph. If that is the
 * case, a "canonical" node (not bound by any turn restrictions) will
 * have `id == osm_id`.
 *
 * Nodes with `id == 0` signify the absence of a node.
 */
using Node = RoutxNode;

/**
 * Outgoing (one-way) connection from a @ref Node.
 *
 * `cost` must be greater than the crow-flies distance between the two nodes.
 */
using Edge = RoutxEdge;

namespace osm {

/**
 * Numeric multiplier for OSM ways with specific keys and values.
 */
using Penalty = RoutxOsmProfilePenalty;

/**
 * Describes how to convert OSM data into a @ref Graph.
 */
using Profile = RoutxOsmProfile;

/**
 * Format of the input OSM file.
 */
using Format = RoutxOsmFormat;

/**
 * Controls for interpreting OSM data as a routing @ref Graph.
 */
using Options = RoutxOsmOptions;

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
Profile const* const ProfileCar = ROUTX_OSM_PROFILE_CAR;

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
Profile const* const ProfileBus = ROUTX_OSM_PROFILE_BUS;

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
Profile const* const ProfileBicycle = ROUTX_OSM_PROFILE_BICYCLE;

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
Profile const* const ProfileFoot = ROUTX_OSM_PROFILE_FOOT;

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
Profile const* const ProfileRailway = ROUTX_OSM_PROFILE_RAILWAY;

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
Profile const* const ProfileTram = ROUTX_OSM_PROFILE_TRAM;

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
Profile const* const ProfileSubway = ROUTX_OSM_PROFILE_SUBWAY;

/**
 * Thrown when the routx library has failed to load OSM data. See logs for details.
 */
class LoadingFailed : std::runtime_error {
   public:
    LoadingFailed() : std::runtime_error("failed to load OSM data") {}
};

}  // namespace osm

/**
 * Route represents an _owned_ sequence of nodes, that when traversed, get you from A to B.
 *
 * This class inherits from a
 * [std::span<int64_t>](https://en.cppreference.com/w/cpp/container/span.html) and therefore all of
 * span methods are available. However, this class has an extra field (required for the underlying
 * destructor) and is not copyable.
 */
class Route : public std::span<int64_t> {
   public:
    Route(int64_t* data, uint32_t len, uint32_t cap)
        : std::span<int64_t>(data, len), m_capacity(cap) {}

    Route(Route const&) = delete;
    Route& operator=(Route const&) = delete;

    ~Route() {
        RoutxRouteResult raw = {
            .as_ok =
                {
                    .nodes = data(),
                    .len = static_cast<uint32_t>(size()),
                    .capacity = m_capacity,
                },
            .type = RoutxRouteResultTypeOk,
        };
        routx_route_result_delete(raw);
    }

   private:
    uint32_t m_capacity;
};

/**
 * Graph::find_route() has been given a node that does not exists.
 */
class InvalidReference : public std::out_of_range {
   public:
    InvalidReference(int64_t node_id)
        : std::out_of_range("no such node exists"), m_invalid_node_id(node_id) {}

    /// ID of the non-existing node
    int64_t invalid_node_id() const { return m_invalid_node_id; }

   private:
    int64_t m_invalid_node_id;
};

/**
 * Graph::find_route() has exceeded its step limit.
 */
class StepLimitExceeded : public std::length_error {
   public:
    StepLimitExceeded() : std::length_error("step limit exceeded") {}
};

/**
 * OpenStreetMap-based network representation as a set of @ref Node "Nodes"
 * and @ref Edge "Edges" between them.
 */
class Graph {
    friend class KDTree;
    friend class Iterator;

   public:
    /**
     * Iterator over @ref Node "Nodes" contained in a @ref Graph.
     *
     * Note that this class can't fullfill the C++ definition of an "iterator",
     * as it's not copyable.
     *
     * Usage:
     *
     * @code{.cpp}
     * routx::Graph::Iterator it = graph.get_nodes();
     * routx::Node node;
     * while ((node = it.next()).id != 0) {
     *     // Process node
     * }
     * @endcode
     */
    class Iterator {
       public:
        /**
         * Takes ownership of a C-style iterator handle.
         *
         * The pointer maybe null, which creates a NULL iterator, for which all operations are a
         * no-op.
         */
        explicit Iterator(RoutxGraphIterator* i) : m_impl(i) {}

        ~Iterator() { routx_graph_iterator_delete(m_impl); }

        Iterator(Iterator const&) = delete;

        Iterator(Iterator&& other) : m_impl(nullptr) { std::swap(m_impl, other.m_impl); }

        Iterator& operator=(Iterator const&) = delete;

        Iterator& operator=(Iterator&& other) {
            std::swap(m_impl, other.m_impl);
            return *this;
        }

        /**
         * Advances the iterator and returns the next node.
         * A zero node (`id == 0`) will be returned to mark the end of iteration.
         *
         * The iterator may be NULL, in which case this function returns a zero node.
         */
        Node next() { return routx_graph_iterator_next(m_impl); }

       private:
        RoutxGraphIterator* m_impl = nullptr;
    };

    /**
     * Allocates a new Graph.
     */
    Graph() : m_impl(routx_graph_new()) {}

    /**
     * Takes ownership of a C-style Graph handle.
     *
     * The pointer maybe null, which creates a NULL Graph, for which all operations are a no-op.
     */
    explicit Graph(RoutxGraph* g) : m_impl(g) {}

    ~Graph() { routx_graph_delete(m_impl); }

    Graph(Graph const&) = delete;

    Graph(Graph&& other) : m_impl(nullptr) { std::swap(m_impl, other.m_impl); }

    Graph& operator=(Graph const&) = delete;

    Graph& operator=(Graph&& other) noexcept {
        std::swap(m_impl, other.m_impl);
        return *this;
    }

    /**
     * Returns the number of @ref Node "Nodes" in the graph.
     */
    size_t size() const { return routx_graph_get_nodes(m_impl, nullptr); }

    /**
     * Returns true if there are no @ref Node "Nodes" in the graph.
     */
    bool is_empty() const { return routx_graph_get_nodes(m_impl, nullptr) == 0; }

    /**
     * Returns an @ref Iterator over all nodes in this graph.
     * The graph must not be modified while iterating over its nodes.
     *
     * The returned object doesn't comply with standard C++ iterators,
     * as it contains a non-copyable handle.
     *
     * Usage:
     *
     * @code{.cpp}
     * routx::Graph::Iterator it = graph.get_nodes();
     * routx::Node node;
     * while ((node = it.next()).id != 0) {
     *     // Process node
     * }
     * @endcode
     */
    Iterator get_nodes() const {
        RoutxGraphIterator* it_impl = nullptr;
        routx_graph_get_nodes(m_impl, &it_impl);
        return Iterator(it_impl);
    }

    /**
     * Finds a node with the provided id. If no such node was found, returns a zero (`id == 0`)
     * node.
     *
     * If the graph is NULL, returns a zero node.
     */
    Node get_node(int64_t id) const { return routx_graph_get_node(m_impl, id); }

    /**
     * Creates or updates a @ref Node with the provided id.
     *
     * All outgoing and incoming edges are preserved, thus updating a @ref Node position
     * might result in violation of the @ref Edge invariant (and thus break route finding).
     * It **is discouraged** to update nodes, and it is the caller's responsibility not to break
     * this invariant.
     *
     * When called with a NULL graph, this function does nothing and returns false.
     *
     * @returns true if an existing node was updated/overwritten, false otherwise
     */
    bool set_node(Node node) { return routx_graph_set_node(m_impl, node); }

    /**
     * Deletes a @ref Node with the provided id.
     *
     * Outgoing edges are removed, but incoming edges are preserved (for performance reasons).
     * Thus, deleting a node and then reusing its id might result in violation of
     * @ref Edge cost invariant (breaking route finding) and **is therefore discouraged**.
     * It is the caller's responsibility not to break this invariant.
     *
     * When called with a NULL graph, this function does nothing and return false.
     *
     * @returns true if a node was actually deleted, false otherwise
     */
    bool delete_node(int64_t id) { return routx_graph_delete_node(m_impl, id); }

    /**
     * Finds the closest canonical (`id == osm_id`) @ref Node to the given position.
     *
     * This function requires computing distance to every @ref Node in the graph,
     * and is not suitable for large graphs or for multiple searches.
     * Use @ref KDTree for faster NN finding.
     *
     * If the graph is NULL or has no nodes, returns a zero (`id == 0`) node.
     */
    Node find_nearest_node(float lat, float lon) {
        return routx_graph_find_nearest_node(m_impl, lat, lon);
    }

    /**
     * Gets all outgoing @ref Edge "Edges" from a node with a given id.
     *
     * The Graph must not be modified while using the return edge array, as it might be
     * reallocated.
     */
    std::span<Edge const> get_edges(int64_t from_id) const {
        Edge const* data = nullptr;
        size_t size = routx_graph_get_edges(m_impl, from_id, &data);
        return std::span<Edge const>(data, size);
    }

    /**
     * Gets the cost of a @ref RoutxEdge from one node to another.
     * Returns positive infinity when the provided edge can't be found, or when the graph is NULL.
     */
    float get_edge(int64_t from_id, int64_t to_id) const {
        return routx_graph_get_edge(m_impl, from_id, to_id);
    }

    /**
     * Creates or updates a @ref Edge from a node with a given id.
     *
     * The `cost` must not be smaller than the crow-flies distance between nodes,
     * as this would violate the A* invariant and break route finding. It is the caller's
     * responsibility to do so.
     *
     * When called with a NULL graph, this function does nothing and returns false.
     *
     * @returns true if an existing edge was updated, false otherwise
     */
    bool set_edge(int64_t from_id, Edge edge) {
        return routx_graph_set_edge(m_impl, from_id, edge);
    }

    /**
     * Removes a @ref Edge from one node to another.
     * If no such edge exists (or the graph is NULL), does nothing.
     *
     * @returns true if an edge was removed, false otherwise
     */
    bool delete_edge(int64_t from_id, int64_t to_id) {
        return routx_graph_delete_edge(m_impl, from_id, to_id);
    }

    /**
     * Finds the shortest route between two nodes using the
     * [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm) in the provided graph.
     *
     * Returns an empty @ref Route no route exists.
     *
     * For graphs with turn restrictions, use find_route_without_turn_around(), as this
     * implementation will generate unrealistic instructions with immediate turnarounds (A-B-A) to
     * circumvent any restrictions.
     *
     * The `step_limit` parameter limits how many nodes can be expanded during search before
     * throwing @ref StepLimitExceeded. Concluding that no
     * route exists requires expanding all nodes accessible from the start, which is usually very
     * time consuming, especially on large datasets. Recommended (and default) value is
     * @ref DEFAULT_STEP_LIMIT.
     *
     * If `from` or `to` don't exist, throws @ref InvalidReference.
     */
    Route find_route(int64_t from, int64_t to, size_t step_limit = DEFAULT_STEP_LIMIT) const {
        auto result = routx_find_route(m_impl, from, to, step_limit);
        switch (result.type) {
            [[likely]] case RoutxRouteResultTypeOk:
                return Route(result.as_ok.nodes, result.as_ok.len, result.as_ok.capacity);

            case RoutxRouteResultTypeInvalidReference:
                throw InvalidReference(result.as_invalid_reference.invalid_node_id);

            case RoutxRouteResultTypeStepLimitExceeded:
                throw StepLimitExceeded();

            default:
                std::abort();  // invalid RoutxRouteResultType
        }
    }

    /**
     * Finds the shortest route between two nodes using the
     * [A* algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm) in the provided graph.
     *
     * Returns an empty @ref Route no route exists.
     *
     * For graphs without turn restrictions, use routx_find_route(), as it runs faster.
     * This function has an extra dimension - it needs to not only consider the current node,
     * but also what was the previous node to prevent immediate turnaround (A-B-A) instructions.
     *
     * The `step_limit` parameter limits how many nodes can be expanded during search before
     * throwing @ref StepLimitExceeded. Concluding that no
     * route exists requires expanding all nodes accessible from the start, which is usually very
     * time consuming, especially on large datasets. Recommended (and default) value is
     * @ref DEFAULT_STEP_LIMIT.
     *
     * If `from` or `to` don't exist, throws @ref InvalidReference.
     */
    Route find_route_without_turn_around(int64_t from, int64_t to,
                                         size_t step_limit = DEFAULT_STEP_LIMIT) const {
        auto result = routx_find_route_without_turn_around(m_impl, from, to, step_limit);
        switch (result.type) {
            [[likely]] case RoutxRouteResultTypeOk:
                return Route(result.as_ok.nodes, result.as_ok.len, result.as_ok.capacity);

            case RoutxRouteResultTypeInvalidReference:
                throw InvalidReference(result.as_invalid_reference.invalid_node_id);

            case RoutxRouteResultTypeStepLimitExceeded:
                throw StepLimitExceeded();

            default:
                std::abort();  // invalid RoutxRouteResultType
        }
    }

    /**
     * Parses OSM data from the provided file and adds it to the graph.
     *
     * @param options Options for parsing the OSM data. Must not be NULL.
     * @param filename Path to the OSM file to be parsed. Must not be NULL.
     * @throws @ref osm::LoadingFailed if loading has failed, see logs in such case
     */
    void add_from_osm_file(osm::Options const* options, char const* filename) {
        if (!routx_graph_add_from_osm_file(m_impl, options, filename)) [[unlikely]] {
            throw osm::LoadingFailed();
        }
    }

    /**
     * Parses OSM data from the provided buffer and adds it to the graph.
     *
     * @param options Options for parsing the OSM data. Must not be NULL.
     * @param content Pointer to the buffer containing OSM data. Must be not be NULL, even if
     * content_len == 0.
     * @param content_len Length of the buffer in bytes.
     * @throws @ref osm::LoadingFailed if loading has failed, see logs in such case
     */
    void add_from_osm_memory(osm::Options const* options, char const* data, size_t len) {
        if (!routx_graph_add_from_osm_memory(m_impl, options, data, len)) [[unlikely]] {
            throw osm::LoadingFailed();
        }
    }

   private:
    RoutxGraph* m_impl = nullptr;
};

class KDTree {
   public:
    /// Empty constructor for KDTree is ambiguous, use KDTree::build or KDTree(nullptr) explicitly.
    KDTree() = delete;

    ~KDTree() { routx_kd_tree_delete(m_impl); }

    /**
     * Builds a k-d tree with all canonical (`id == osm_id`) @ref Node "Nodes"
     * contained in the provided @ref Graph.
     *
     * If there are no nodes in the @ref Graph, creates a NULL k-d tree, for which all operations
     * are a no-op.
     */
    static KDTree build(Graph const& graph) { return KDTree(routx_kd_tree_new(graph.m_impl)); }

    /**
     * Takes ownership of a C-style Graph handle.
     *
     * The pointer may be null, which creates a NULL Graph, for which all operations are a no-op.
     */
    explicit KDTree(RoutxKDTree* t) : m_impl(t) {}

    KDTree(KDTree const&) = delete;

    KDTree(KDTree&& other) : m_impl(nullptr) { std::swap(m_impl, other.m_impl); }

    KDTree& operator=(KDTree const&) = delete;

    KDTree& operator=(KDTree&& other) {
        std::swap(m_impl, other.m_impl);
        return *this;
    }

    /**
     * Finds the closest node to the provided position and returns it.
     * If there are no nodes or the k-d tree is NULL, returns a zero (`id == 0`) node.
     */
    Node find_nearest_node(float lat, float lon) {
        return routx_kd_tree_find_nearest_node(m_impl, lat, lon);
    }

   private:
    RoutxKDTree* m_impl = nullptr;
};

}  // namespace routx

#endif  // ROUTX_HPP
