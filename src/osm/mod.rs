// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

mod profile;
mod reader;

pub use profile::{
    Penalty, Profile, BICYCLE_PROFILE, BUS_PROFILE, CAR_PROFILE, FOOT_PROFILE, RAILWAY_PROFILE,
    SUBWAY_PROFILE, TRAM_PROFILE,
};
pub use reader::{
    add_features_from_buffer, add_features_from_file, add_features_from_io, FileFormat, Options,
};

#[cfg(test)]
mod tests {
    use super::super::Graph;
    use super::*;

    macro_rules! assert_almost_eq {
        ($a:expr, $b:expr) => {
            assert!(
                (($a - $b).abs() < 1e-4),
                "assertion failed: {} ≈ {}",
                $a,
                $b
            )
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

    fn check_simple_graph(g: &Graph) {
        //   9
        //   │         8
        //  ┌63┐       │
        // 60  62──────7
        //  └61┘      /│\
        //   │       4 │ 5
        //   │        \│/
        //   2─────────3
        //   │
        //   1

        // Check the loaded amount of nodes
        assert_eq!(g.len(), 14);

        // Check edge costs
        assert_almost_eq!(g.get_edge(-62, -7), 2.0385);
        assert_eq!(g.get_edge(-62, -7), g.get_edge(-7, -62));
        assert_almost_eq!(g.get_edge(-2, -1), 1.4036);

        // Check oneway handling: -4 -> -3 -> -5 -> -7 -> -4
        assert_edge!(g, -4, -3);
        assert_edge!(g, -3, -5);
        assert_edge!(g, -5, -7);
        assert_edge!(g, -7, -4);
        assert_no_edge!(g, -4, -7);
        assert_no_edge!(g, -3, -4);
        assert_no_edge!(g, -5, -3);
        assert_no_edge!(g, -7, -5);

        // Check roundabout handling: -60 -> -61 -> -62 -> -63 -> -60
        assert_edge!(g, -60, -61);
        assert_edge!(g, -61, -62);
        assert_edge!(g, -62, -63);
        assert_edge!(g, -63, -60);
        assert_no_edge!(g, -60, -63);
        assert_no_edge!(g, -61, -60);
        assert_no_edge!(g, -62, -61);
        assert_no_edge!(g, -63, -62);

        // Check access tag handling: -2 <-> -61 has motor_vehicle=no
        assert_no_edge!(g, -2, -61);
        assert_no_edge!(g, -61, -2);

        // Check turn restriction -200: no -8 -> -7 -> -3
        {
            assert_no_edge!(g, -8, -7);
            let phantom_node = g
                .get_edges(-8)
                .iter()
                .find(|&e| g.get_node(e.to).unwrap().osm_id == -7)
                .unwrap()
                .to;
            assert_edge!(g, -8, phantom_node);
            assert_no_edge!(g, phantom_node, -3);
        }

        // Check turn restriction with except=car, -201: no -7 -> -3 -> -5
        assert_edge!(g, -7, -3);
        assert_edge!(g, -3, -5);

        // Check turn restriction: only -1 -> -2 -> -3
        {
            assert_no_edge!(g, -1, -2);

            let phantom_node = g
                .get_edges(-1)
                .iter()
                .find(|&e| g.get_node(e.to).unwrap().osm_id == -2)
                .unwrap()
                .to;
            assert_edge!(g, -1, phantom_node);

            let edges = g.get_edges(phantom_node);
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].to, -3);
        }
    }

    #[test]
    fn test_build_graph_xml_round_trip() {
        const DATA: &[u8] = include_bytes!("reader/test_fixtures/simple.osm");

        let g = {
            let mut g = Graph::default();
            let options = Options {
                profile: &CAR_PROFILE,
                file_format: FileFormat::Xml,
                bbox: [0.0; 4],
            };
            add_features_from_buffer(&mut g, &options, DATA).unwrap();
            g
        };

        check_simple_graph(&g);
    }

    #[test]
    fn test_build_graph_gz_round_trip() {
        const DATA: &[u8] = include_bytes!("reader/test_fixtures/simple.osm.gz");

        let g = {
            let mut g = Graph::default();
            let options = Options {
                profile: &CAR_PROFILE,
                file_format: FileFormat::XmlGz,
                bbox: [0.0; 4],
            };
            add_features_from_buffer(&mut g, &options, DATA).unwrap();
            g
        };

        check_simple_graph(&g);
    }
}
