// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

#include <gtest/gtest.h>

#include <cmath>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <routx.hpp>
#include <string>
#include <string_view>

static std::string_view osm_file_fixture =
    "<?xml version='1.0' encoding='UTF-8'?><osm version='0.6'>\n"
    "<node id='-1' lat='0.00' lon='0.02' />\n"
    "<node id='-2' lat='0.00' lon='0.01' />\n"
    "<node id='-3' lat='0.00' lon='0.00' />\n"
    "<node id='-4' lat='0.01' lon='0.01' />\n"
    "<node id='-5' lat='0.01' lon='0.00' />\n"
    "<way id='-10'>\n"
    "  <nd ref='-1' /><nd ref='-2' />\n"
    "  <tag k='highway' v='tertiary' />\n"
    "</way>\n"
    "<way id='-11'>\n"
    "  <nd ref='-2' /><nd ref='-3' />\n"
    "  <tag k='highway' v='tertiary' />\n"
    "</way>\n"
    "<way id='-12'>\n"
    "  <nd ref='-2' /><nd ref='-4' />\n"
    "  <tag k='highway' v='residential' />\n"
    "</way>\n"
    "<way id='-13'>\n"
    "  <nd ref='-4' /><nd ref='-5' /><nd ref='-3' />\n"
    "  <tag k='highway' v='service' />\n"
    "</way>\n"
    "<relation id='-20'>\n"
    "  <member type='way' ref='-10' role='from' />\n"
    "  <member type='node' ref='-2' role='via' />\n"
    "  <member type='way' ref='-12' role='to' />\n"
    "  <tag k='restriction' v='only_left_turn' />\n"
    "  <tag k='type' v='restriction' />\n"
    "</relation>\n"
    "</osm>\n";

TEST(Graph, ManipulateNodes) {
    routx::Graph g = {};

    // Check size() and is_empty() on an empty Graph
    ASSERT_EQ(g.size(), 0);
    ASSERT_TRUE(g.is_empty());

    // Add a few nodes
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.01, .lon = 0.05});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.09});

    // Check size() and is_empty() on a non-empty Graph
    ASSERT_EQ(g.size(), 3);
    ASSERT_FALSE(g.is_empty());

    // Check get_node() on an existing node
    auto n = g.get_node(2);
    ASSERT_EQ(n.id, 2);
    ASSERT_EQ(n.osm_id, 2);
    ASSERT_FLOAT_EQ(n.lat, 0.01);
    ASSERT_FLOAT_EQ(n.lon, 0.05);

    // Check get_node() on a non-existing node
    ASSERT_EQ(g.get_node(42).id, 0);

    // Check delete_node() and size() on an existing node
    auto removed = g.delete_node(3);
    ASSERT_TRUE(removed);
    ASSERT_EQ(g.size(), 2);
    ASSERT_EQ(g.get_node(3).id, 0);

    // Check delete_node() on a non-existing node
    removed = g.delete_node(42);
    ASSERT_FALSE(removed);
}

TEST(Graph, OverwriteNode) {
    routx::Graph g = {};

    auto updated = g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    ASSERT_FALSE(updated);
    auto n = g.get_node(1);
    ASSERT_EQ(n.id, 1);
    ASSERT_EQ(n.osm_id, 1);
    ASSERT_FLOAT_EQ(n.lat, 0.01);
    ASSERT_FLOAT_EQ(n.lon, 0.01);

    updated = g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.05});
    ASSERT_TRUE(updated);
    n = g.get_node(1);
    ASSERT_EQ(n.id, 1);
    ASSERT_EQ(n.osm_id, 1);
    ASSERT_FLOAT_EQ(n.lat, 0.01);
    ASSERT_FLOAT_EQ(n.lon, 0.05);
}

TEST(Graph, Iterator) {
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.01, .lon = 0.05});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.09});

    auto it = g.get_nodes();

    auto n = it.next();
    ASSERT_EQ(n.id, 1);
    ASSERT_EQ(n.osm_id, 1);
    ASSERT_FLOAT_EQ(n.lat, 0.01);
    ASSERT_FLOAT_EQ(n.lon, 0.01);

    n = it.next();
    ASSERT_EQ(n.id, 2);
    ASSERT_EQ(n.osm_id, 2);
    ASSERT_FLOAT_EQ(n.lat, 0.01);
    ASSERT_FLOAT_EQ(n.lon, 0.05);

    n = it.next();
    ASSERT_EQ(n.id, 3);
    ASSERT_EQ(n.osm_id, 3);
    ASSERT_FLOAT_EQ(n.lat, 0.03);
    ASSERT_FLOAT_EQ(n.lon, 0.09);

    n = it.next();
    ASSERT_EQ(n.id, 0);
}

TEST(Graph, FindNearestNode) {
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.01, .lon = 0.05});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.09});
    g.set_node(routx::Node{.id = 4, .osm_id = 4, .lat = 0.04, .lon = 0.03});
    g.set_node(routx::Node{.id = 5, .osm_id = 5, .lat = 0.04, .lon = 0.07});
    g.set_node(routx::Node{.id = 6, .osm_id = 6, .lat = 0.07, .lon = 0.03});
    g.set_node(routx::Node{.id = 7, .osm_id = 7, .lat = 0.07, .lon = 0.01});
    g.set_node(routx::Node{.id = 8, .osm_id = 8, .lat = 0.08, .lon = 0.05});
    g.set_node(routx::Node{.id = 9, .osm_id = 9, .lat = 0.08, .lon = 0.09});

    EXPECT_EQ(g.find_nearest_node(0.02, 0.02).id, 1);
    EXPECT_EQ(g.find_nearest_node(0.05, 0.03).id, 4);
    EXPECT_EQ(g.find_nearest_node(0.05, 0.08).id, 5);
    EXPECT_EQ(g.find_nearest_node(0.09, 0.06).id, 8);
}

TEST(Graph, FindNearestNodeEmptyGraph) {
    routx::Graph g = {};
    EXPECT_EQ(g.find_nearest_node(0.02, 0.02).id, 0);
}

TEST(Graph, FindNearestNodeCanonical) {
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 100, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 101, .osm_id = 1, .lat = 0.01, .lon = 0.01});

    EXPECT_EQ(g.find_nearest_node(0.02, 0.02).id, 1);
}

TEST(Graph, ManipulateEdges) {
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.02, .lon = 0.01});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.01});

    // get_edge() and get_edges() on a Graph without edges
    ASSERT_FLOAT_EQ(g.get_edge(2, 1), HUGE_VALF);
    ASSERT_EQ(g.get_edges(2).size(), 0);

    // set_edge() on new edges
    auto updated = g.set_edge(1, routx::Edge{.to = 2, .cost = 200.0});
    ASSERT_FALSE(updated);

    g.set_edge(2, routx::Edge{.to = 1, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 3, .cost = 100.0});
    g.set_edge(3, routx::Edge{.to = 2, .cost = 100.0});

    // get_edge() and get_edges() on a Graph with edges
    ASSERT_FLOAT_EQ(g.get_edge(2, 1), 200.0);
    {
        auto edges = g.get_edges(2);
        ASSERT_EQ(edges.size(), 2);
        ASSERT_EQ(edges[0].to, 1);
        ASSERT_FLOAT_EQ(edges[0].cost, 200.0);
        ASSERT_EQ(edges[1].to, 3);
        ASSERT_FLOAT_EQ(edges[1].cost, 100.0);
    }

    // set_edge() on existing edge
    updated = g.set_edge(1, routx::Edge{.to = 2, .cost = 150.0});
    ASSERT_TRUE(updated);
    ASSERT_FLOAT_EQ(g.get_edge(1, 2), 150.0);

    // delete_edge() on existing edge
    auto deleted = g.delete_edge(1, 2);
    ASSERT_TRUE(deleted);
    ASSERT_FLOAT_EQ(g.get_edge(1, 2), HUGE_VALF);

    // delete_edge() on non-existing edge
    deleted = g.delete_edge(1, 42);
    ASSERT_FALSE(deleted);
}

TEST(Graph, FindRoute) {
    //   200   200   200
    // 1─────2─────3─────4
    //       └─────5─────┘
    //         100    100
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.02, .lon = 0.01});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.01});
    g.set_node(routx::Node{.id = 4, .osm_id = 4, .lat = 0.04, .lon = 0.01});
    g.set_node(routx::Node{.id = 5, .osm_id = 5, .lat = 0.03, .lon = 0.00});
    g.set_edge(1, routx::Edge{.to = 2, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 1, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 3, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 5, .cost = 100.0});
    g.set_edge(3, routx::Edge{.to = 2, .cost = 200.0});
    g.set_edge(3, routx::Edge{.to = 4, .cost = 200.0});
    g.set_edge(4, routx::Edge{.to = 3, .cost = 200.0});
    g.set_edge(4, routx::Edge{.to = 5, .cost = 100.0});
    g.set_edge(5, routx::Edge{.to = 2, .cost = 100.0});
    g.set_edge(5, routx::Edge{.to = 4, .cost = 100.0});

    constexpr size_t step_limit = 100;

    {
        auto r = g.find_route(1, 4, step_limit);
        ASSERT_EQ(r.size(), 4);
        ASSERT_EQ(r[0], 1);
        ASSERT_EQ(r[1], 2);
        ASSERT_EQ(r[2], 5);
        ASSERT_EQ(r[3], 4);
    }

    {
        auto r = g.find_route_without_turn_around(1, 4, step_limit);
        ASSERT_EQ(r.size(), 4);
        ASSERT_EQ(r[0], 1);
        ASSERT_EQ(r[1], 2);
        ASSERT_EQ(r[2], 5);
        ASSERT_EQ(r[3], 4);
    }
}

TEST(Graph, FindRouteWithTurnRestriction) {
    // 1
    // │
    // │10
    // │ 10
    // 2─────4
    // │     │
    // │10   │100
    // │ 10  │
    // 3─────5
    // mandatory 1-2-4
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.00, .lon = 0.02});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.00, .lon = 0.01});
    g.set_node(routx::Node{.id = 20, .osm_id = 2, .lat = 0.00, .lon = 0.01});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.00, .lon = 0.00});
    g.set_node(routx::Node{.id = 4, .osm_id = 4, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 5, .osm_id = 5, .lat = 0.01, .lon = 0.00});
    g.set_edge(1, routx::Edge{.to = 20, .cost = 10.0});
    g.set_edge(2, routx::Edge{.to = 1, .cost = 10.0});
    g.set_edge(2, routx::Edge{.to = 3, .cost = 10.0});
    g.set_edge(2, routx::Edge{.to = 4, .cost = 10.0});
    g.set_edge(20, routx::Edge{.to = 4, .cost = 10.0});
    g.set_edge(3, routx::Edge{.to = 2, .cost = 10.0});
    g.set_edge(3, routx::Edge{.to = 5, .cost = 10.0});
    g.set_edge(4, routx::Edge{.to = 2, .cost = 10.0});
    g.set_edge(4, routx::Edge{.to = 5, .cost = 100.0});
    g.set_edge(5, routx::Edge{.to = 3, .cost = 10.0});
    g.set_edge(5, routx::Edge{.to = 4, .cost = 100.0});

    constexpr size_t step_limit = 100;

    {
        auto r = g.find_route(1, 3, step_limit);
        ASSERT_EQ(r.size(), 5);
        ASSERT_EQ(r[0], 1);
        ASSERT_EQ(r[1], 20);
        ASSERT_EQ(r[2], 4);
        ASSERT_EQ(r[3], 2);
        ASSERT_EQ(r[4], 3);
    }

    {
        auto r = g.find_route_without_turn_around(1, 3, step_limit);
        ASSERT_EQ(r.size(), 5);
        ASSERT_EQ(r[0], 1);
        ASSERT_EQ(r[1], 20);
        ASSERT_EQ(r[2], 4);
        ASSERT_EQ(r[3], 5);
        ASSERT_EQ(r[4], 3);
    }
}

TEST(Graph, FindRouteNoRoute) {
    //   200   200   200
    // 1─────2─────3─────4
    //       └─────5─────┘
    //         100    100
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.02, .lon = 0.01});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.01});
    g.set_node(routx::Node{.id = 4, .osm_id = 4, .lat = 0.04, .lon = 0.01});
    g.set_node(routx::Node{.id = 5, .osm_id = 5, .lat = 0.03, .lon = 0.00});
    g.set_edge(1, routx::Edge{.to = 2, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 1, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 3, .cost = 200.0});
    g.set_edge(2, routx::Edge{.to = 5, .cost = 100.0});
    g.set_edge(3, routx::Edge{.to = 2, .cost = 200.0});
    g.set_edge(3, routx::Edge{.to = 4, .cost = 200.0});
    g.set_edge(4, routx::Edge{.to = 3, .cost = 200.0});
    g.set_edge(4, routx::Edge{.to = 5, .cost = 100.0});
    g.set_edge(5, routx::Edge{.to = 2, .cost = 100.0});
    g.set_edge(5, routx::Edge{.to = 4, .cost = 100.0});

    constexpr size_t step_limit = 2;
    ASSERT_THROW(g.find_route(1, 4, step_limit), routx::StepLimitExceeded);
}

TEST(Graph, FindRouteInvalidReference) {
    routx::Graph g = {};
    ASSERT_THROW(g.find_route(1, 2), routx::InvalidReference);
}

class TemporaryFile {
   public:
    TemporaryFile() : m_path(std::tmpnam(nullptr)) {}

    ~TemporaryFile() { std::filesystem::remove(m_path); }

    TemporaryFile(TemporaryFile const&) = delete;
    TemporaryFile(TemporaryFile&& o) = delete;
    TemporaryFile& operator=(TemporaryFile const&) = delete;
    TemporaryFile& operator=(TemporaryFile&& o) = delete;

    std::filesystem::path const& path() const { return m_path; }

   private:
    std::filesystem::path m_path;
};

TEST(Graph, AddFromOsmFile) {
    // Create a temporary file fixture and write its content
    TemporaryFile temp_file = {};
    std::ofstream(temp_file.path()) << osm_file_fixture;

    // Attempt to load it
    routx::Graph g = {};
    routx::osm::Options o = {
        .profile = routx::osm::ProfileCar,
        .file_format = RoutxOsmFormatXml,
        .bbox = {0},
    };
    g.add_from_osm_file(&o, temp_file.path().c_str());

    EXPECT_EQ(g.size(), 6);
}

TEST(Graph, AddFromOsmFileError) {
    routx::Graph g = {};
    routx::osm::Options o = {
        .profile = routx::osm::ProfileCar,
        .file_format = RoutxOsmFormatUnknown,
        .bbox = {0},
    };

    EXPECT_THROW(g.add_from_osm_file(&o, "non_existing_file.osm"), routx::osm::LoadingFailed);
}

TEST(Graph, AddFromOsmMemory) {
    routx::Graph g = {};
    routx::osm::Options o = {
        .profile = routx::osm::ProfileCar,
        .file_format = RoutxOsmFormatXml,
        .bbox = {0},
    };
    g.add_from_osm_memory(&o, osm_file_fixture.data(), osm_file_fixture.size());

    EXPECT_EQ(g.size(), 6);
}

TEST(Graph, AddFromOsmCustomProfile) {
    routx::Graph g = {};

    routx::osm::Penalty penalties[2] = {
        {.key = "highway", .value = "tertiary", .penalty = 1.0},
        {.key = "highway", .value = "residential", .penalty = 2.0},
    };
    char const* access[2] = {"access", "vehicle"};
    routx::osm::Profile p = {
        .name = "car",
        .penalties = penalties,
        .penalties_len = 2,
        .access = access,
        .access_len = 2,
        .disallow_motorroad = false,
        .disable_restrictions = true,
    };
    routx::osm::Options o = {
        .profile = &p,
        .file_format = RoutxOsmFormatXml,
        .bbox = {0},
    };
    g.add_from_osm_memory(&o, osm_file_fixture.data(), osm_file_fixture.size());

    EXPECT_EQ(g.size(), 4);
}

TEST(Utility, EarthDistance) {
    float centrum_lat = 52.23024;
    float centrum_lon = 21.01062;

    float stadion_lat = 52.23852;
    float stadion_lon = 21.0446;

    float falenica_lat = 52.16125;
    float falenica_lon = 21.21147;

    EXPECT_NEAR(routx::earth_distance(centrum_lat, centrum_lon, stadion_lat, stadion_lon), 2.49049,
                1e-6);
    EXPECT_NEAR(routx::earth_distance(centrum_lat, centrum_lon, falenica_lat, falenica_lon),
                15.692483, 1e-6);
}

TEST(Utility, KDTree) {
    routx::Graph g = {};
    g.set_node(routx::Node{.id = 1, .osm_id = 1, .lat = 0.01, .lon = 0.01});
    g.set_node(routx::Node{.id = 2, .osm_id = 2, .lat = 0.01, .lon = 0.05});
    g.set_node(routx::Node{.id = 3, .osm_id = 3, .lat = 0.03, .lon = 0.09});
    g.set_node(routx::Node{.id = 4, .osm_id = 4, .lat = 0.04, .lon = 0.03});
    g.set_node(routx::Node{.id = 5, .osm_id = 5, .lat = 0.04, .lon = 0.07});
    g.set_node(routx::Node{.id = 6, .osm_id = 6, .lat = 0.07, .lon = 0.03});
    g.set_node(routx::Node{.id = 7, .osm_id = 7, .lat = 0.07, .lon = 0.01});
    g.set_node(routx::Node{.id = 8, .osm_id = 8, .lat = 0.08, .lon = 0.05});
    g.set_node(routx::Node{.id = 9, .osm_id = 9, .lat = 0.08, .lon = 0.09});

    auto kd = routx::KDTree::build(g);
    EXPECT_EQ(kd.find_nearest_node(0.02, 0.02).id, 1);
    EXPECT_EQ(kd.find_nearest_node(0.05, 0.03).id, 4);
    EXPECT_EQ(kd.find_nearest_node(0.05, 0.08).id, 5);
    EXPECT_EQ(kd.find_nearest_node(0.09, 0.06).id, 8);
}
