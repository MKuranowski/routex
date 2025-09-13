#ifndef ROUTEX_H
#define ROUTEX_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct RoutexNode {
    int64_t id;
    int64_t osm_id;
    float lat;
    float lon;
} RoutexNode;

typedef struct RoutexEdge {
    int64_t to;
    float cost;
} RoutexEdge;

typedef struct RoutexGraph RoutexGraph;
typedef struct RoutexGraphIterator RoutexGraphIterator;

RoutexGraph* routex_graph_new(void);
void routex_graph_delete(RoutexGraph*);

size_t routex_graph_get_nodes(RoutexGraph const*, RoutexGraphIterator**);
RoutexNode routex_graph_iterator_next(RoutexGraphIterator*);
void routex_graph_iterator_delete(RoutexGraphIterator*);

RoutexNode routex_graph_get_node(RoutexGraph const*, int64_t id);
bool routex_graph_set_node(RoutexGraph*, RoutexNode);
bool routex_graph_delete_node(RoutexGraph*, int64_t id);
RoutexNode routex_graph_find_nearest_node(RoutexGraph const*, float lat, float lon);

size_t routex_graph_get_edges(RoutexGraph const*, int64_t from_id, RoutexEdge const**);
float routex_graph_get_edge(RoutexGraph const*, int64_t from_id, int64_t to_id);
bool routex_graph_set_edge(RoutexGraph*, int64_t from_id, RoutexEdge);
bool routex_graph_delete_edge(RoutexGraph*, int64_t from_id, int64_t to_id);

typedef struct RoutexOsmProfilePenalty {
    char const* key;
    char const* value;
    float penalty;
} RoutexOsmProfilePenalty;

typedef struct RoutexOsmProfile {
    char const* name;

    RoutexOsmProfilePenalty* penalties;
    size_t penalties_len;

    char const** access;
    size_t access_len;

    bool disallow_motorroad;
    bool disable_restrictions;
} RoutexOsmProfile;

typedef enum RoutexOsmFormat {
    RoutexOsmFormatUnknown = 0,
    RoutexOsmFormatXml = 1,
    RoutexOsmFormatXmlGz = 2,
    RoutexOsmFormatXmlBz2 = 3,
    RoutexOsmFormatPbf = 4,
} RoutexOsmFormat;

typedef struct RoutexOsmOptions {
    RoutexOsmProfile const* profile;
    RoutexOsmFormat file_format;
    float bbox[4];
} RoutexOsmOptions;

/**
 * Parses OSM data from the provided file and adds it to the provided graph.
 *
 * @param g Graph to which the OSM data will be added. If NULL, this function does nothing and returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param filename Path to the OSM file to be parsed. Must not be NULL.
 * @returns true if an error occurred, false otherwise
 */
bool routex_graph_add_from_osm_file(RoutexGraph* g, RoutexOsmOptions const* options, char const* filename);

/**
 * Parses OSM data from the provided buffer and adds it to the provided graph.
 *
 * @param g Graph to which the OSM data will be added. If NULL, this function does nothing and returns false.
 * @param options Options for parsing the OSM data. Must not be NULL.
 * @param content Pointer to the buffer containing OSM data. Must be not be NULL, even if content_len == 0.
 * @param content_len Length of the buffer in bytes.
 * @returns true if an error occurred, false otherwise
 */
bool routex_graph_add_from_osm_memory(RoutexGraph* g, RoutexOsmOptions const* options, unsigned char const* content, size_t content_len);

typedef enum RoutexRouteResultType {
    RoutexRouteResultTypeOk = 0,
    RoutexRouteResultTypeInvalidReference = 1,
    RoutexRouteResultTypeStepLimitExceeded = 2,
} RoutexRouteResultType;

typedef struct RoutexRouteResult {
    union {
        struct {
            int64_t* nodes;
            uint32_t len;
            uint32_t capacity;
        } as_ok;

        struct {
            int64_t invalid_node_id;
        } as_invalid_reference;
    };

    RoutexRouteResultType type;
} RoutexRouteResult;

RoutexRouteResult routex_find_route(RoutexGraph const*, int64_t from, int64_t to, size_t step_limit);
RoutexRouteResult routex_find_route_without_turn_around(RoutexGraph const*, int64_t from, int64_t to, size_t step_limit);
void routex_route_result_delete(RoutexRouteResult);

typedef struct RoutexKDTree RoutexKDTree;

RoutexKDTree* routex_kd_tree_new(RoutexGraph const*);
void routex_kd_tree_delete(RoutexKDTree*);
int64_t routex_kd_tree_find_nearest_node(RoutexKDTree const*, float lat, float lon);

float routex_earth_distance(float lat1, float lon1, float lat2, float lon2);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // ROUTEX_H
