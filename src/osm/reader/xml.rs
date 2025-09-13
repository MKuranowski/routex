// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::io;
use std::str::from_utf8;

use super::model;
use crate::Node;

pub fn features_from_file<R: io::BufRead>(
    reader: R,
) -> impl Iterator<Item = Result<model::Feature, quick_xml::Error>> {
    Reader::from_io(reader)
}

pub fn features_from_buffer(
    b: &[u8],
) -> impl Iterator<Item = Result<model::Feature, quick_xml::Error>> + '_ {
    Reader::from_buffer(b)
}
/// Parser is a trait for objects which can parse XML.
///
/// This trait only exists to fix the mismatch of
/// [quick_xml::Reader::read_event] when working on buffered data
/// and [quick_xml::Reader::read_event_into] when working on IO.
trait Parser {
    fn read_event<'a>(&'a mut self) -> quick_xml::Result<quick_xml::events::Event<'a>>;
}

/// IoParser implements [Parser] over an [std::io::BufRead].
struct IoParser<R: io::BufRead>(quick_xml::Reader<R>, Vec<u8>);

impl<R: io::BufRead> IoParser<R> {
    #[inline]
    fn new(reader: R) -> Self {
        Self(quick_xml::Reader::from_reader(reader), Vec::default())
    }
}

impl<R: io::BufRead> Parser for IoParser<R> {
    #[inline]
    fn read_event<'a>(&'a mut self) -> quick_xml::Result<quick_xml::events::Event<'a>> {
        self.0.read_event_into(&mut self.1)
    }
}

/// BufParser implements [Parser] over a slice of bytes (`&[u8]`).
struct BufParser<'a>(quick_xml::Reader<&'a [u8]>);

impl<'a> BufParser<'a> {
    #[inline]
    fn new(data: &'a [u8]) -> Self {
        Self(quick_xml::Reader::from_reader(data))
    }
}

impl<'a> Parser for BufParser<'a> {
    #[inline]
    fn read_event<'b>(&'b mut self) -> quick_xml::Result<quick_xml::events::Event<'b>> {
        self.0.read_event()
    }
}

/// Reader reads osm [Features](Feature) from an XML file.
struct Reader<P: Parser> {
    parser: P,
    eof: bool,
}

impl<P: Parser> Reader<P> {
    #[inline]
    fn new(parser: P) -> Self {
        Self { parser, eof: false }
    }
}

impl<P: Parser> Iterator for Reader<P> {
    type Item = Result<model::Feature, quick_xml::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut f: Option<model::Feature> = None;

        while !self.eof {
            let event = match self.parser.read_event() {
                Ok(e) => e,
                Err(e) => return Some(Err(e)),
            };

            match event {
                quick_xml::events::Event::Empty(start) => {
                    match start.local_name().as_ref() {
                        b"node" => match parse_node(start) {
                            Some(n) => return Some(Ok(model::Feature::Node(n))),
                            None => {}
                        },
                        // "way" or "relation" can't be self-closing
                        b"tag" => {
                            if let Some(tags) = feature_tags(&mut f) {
                                if let Some((k, v)) = parse_tag(start) {
                                    tags.insert(k, v);
                                }
                            }
                        }
                        b"nd" => {
                            if let Some(nodes) = feature_nodes(&mut f) {
                                if let Some(ref_) = parse_nd(start) {
                                    nodes.push(ref_);
                                }
                            }
                        }
                        b"member" => {
                            if let Some(members) = feature_members(&mut f) {
                                if let Some(member) = parse_member(start) {
                                    members.push(member);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                quick_xml::events::Event::Start(start) => match start.local_name().as_ref() {
                    b"node" => f = parse_node(start).map(|n| model::Feature::Node(n)),
                    b"way" => f = parse_way(start).map(|w| model::Feature::Way(w)),
                    b"relation" => f = parse_relation(start).map(|r| model::Feature::Relation(r)),
                    // "tag", "nd" and "member" must be self-closing
                    _ => {}
                },

                quick_xml::events::Event::End(end) => match end.local_name().as_ref() {
                    b"node" | b"way" | b"relation" => {
                        if let Some(f) = f.take() {
                            return Some(Ok(f));
                        }
                    }
                    _ => {}
                },

                quick_xml::events::Event::Eof => {
                    self.eof = true;
                }

                _ => {}
            }
        }

        return f.map(Ok);
    }
}

impl<'a> Reader<BufParser<'a>> {
    #[inline]
    fn from_buffer(data: &'a [u8]) -> Self {
        Self::new(BufParser::new(data))
    }
}

impl<R: io::BufRead> Reader<IoParser<R>> {
    #[inline]
    fn from_io(reader: R) -> Self {
        Self::new(IoParser::new(reader))
    }
}

fn parse_node(start: quick_xml::events::BytesStart<'_>) -> Option<Node> {
    // TODO: Log errors instead of silencing them

    let mut id: i64 = 0;
    let mut lat = f32::NAN;
    let mut lon = f32::NAN;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"id" => id = from_utf8(&attr.value).ok()?.parse().ok()?,
            b"lat" => lat = from_utf8(&attr.value).ok()?.parse().ok()?,
            b"lon" => lon = from_utf8(&attr.value).ok()?.parse().ok()?,
            _ => {}
        }
    }

    if id != 0 && lat.is_finite() && lon.is_finite() {
        Some(Node {
            id: id,
            osm_id: id,
            lat: lat,
            lon: lon,
        })
    } else {
        None
    }
}

fn parse_way(start: quick_xml::events::BytesStart<'_>) -> Option<model::Way> {
    // TODO: Log errors instead of silencing them

    let mut id: i64 = 0;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"id" => id = from_utf8(&attr.value).ok()?.parse().ok()?,
            _ => {}
        }
    }

    if id != 0 {
        Some(model::Way {
            id: id,
            nodes: Vec::default(),
            tags: HashMap::default(),
        })
    } else {
        None
    }
}

fn parse_relation(start: quick_xml::events::BytesStart<'_>) -> Option<model::Relation> {
    // TODO: Log errors instead of silencing them

    let mut id: i64 = 0;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"id" => id = from_utf8(&attr.value).ok()?.parse().ok()?,
            _ => {}
        }
    }

    if id != 0 {
        Some(model::Relation {
            id: id,
            members: Vec::default(),
            tags: HashMap::default(),
        })
    } else {
        None
    }
}

fn parse_tag(start: quick_xml::events::BytesStart<'_>) -> Option<(String, String)> {
    // TODO: Log errors instead of silencing them

    let mut k = None;
    let mut v = None;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"k" => k = from_utf8(&attr.value).ok().map(|s| s.to_string()),
            b"v" => v = from_utf8(&attr.value).ok().map(|s| s.to_string()),
            _ => {}
        }
    }

    if let Some(k) = k {
        Some((k, v.unwrap_or_default()))
    } else {
        None
    }
}

fn parse_nd(start: quick_xml::events::BytesStart<'_>) -> Option<i64> {
    // TODO: Log errors instead of silencing them

    let mut ref_: i64 = 0;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"ref" => ref_ = from_utf8(&attr.value).ok()?.parse().ok()?,
            _ => {}
        }
    }

    if ref_ != 0 {
        Some(ref_)
    } else {
        None
    }
}

fn parse_member(start: quick_xml::events::BytesStart<'_>) -> Option<model::RelationMember> {
    // TODO: Log errors instead of silencing them

    let mut ref_: i64 = 0;
    let mut type_ = None;
    let mut role = None;

    for attr in start.attributes() {
        let attr = attr.ok()?;
        match attr.key.as_ref() {
            b"ref" => ref_ = from_utf8(&attr.value).ok()?.parse().ok()?,
            b"type" => type_ = Some(parse_feature_type(&attr.value)?),
            b"role" => role = Some(from_utf8(&attr.value).ok()?.to_string()),
            _ => {}
        }
    }

    match (ref_, type_, role) {
        (0, _, _) => None,
        (ref_, Some(type_), Some(role)) => Some(model::RelationMember { type_, ref_, role }),
        _ => None,
    }
}

fn parse_feature_type(s: &[u8]) -> Option<model::FeatureType> {
    match s {
        b"node" => Some(model::FeatureType::Node),
        b"way" => Some(model::FeatureType::Way),
        b"relation" => Some(model::FeatureType::Relation),
        _ => None,
    }
}

fn feature_tags<'a>(f: &'a mut Option<model::Feature>) -> Option<&'a mut HashMap<String, String>> {
    match f {
        None => None,
        Some(model::Feature::Node(_)) => None,
        Some(model::Feature::Way(ref mut w)) => Some(&mut w.tags),
        Some(model::Feature::Relation(ref mut r)) => Some(&mut r.tags),
    }
}

fn feature_nodes<'a>(f: &'a mut Option<model::Feature>) -> Option<&'a mut Vec<i64>> {
    match f {
        Some(model::Feature::Way(ref mut w)) => Some(&mut w.nodes),
        _ => None,
    }
}

fn feature_members<'a>(
    f: &'a mut Option<model::Feature>,
) -> Option<&'a mut Vec<model::RelationMember>> {
    match f {
        Some(model::Feature::Relation(ref mut r)) => Some(&mut r.members),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::FeatureReader;
    use super::model::{Feature, FeatureType, Relation, RelationMember, Way};
    use super::*;

    macro_rules! tags {
        {} => { HashMap::default() };
        {$( $k:literal : $v:literal ),+} => {
            HashMap::from_iter([ $( ($k.to_string(), $v.to_string()) ),+ ])
        };
    }

    const SIMPLE_XML: &[u8] = include_bytes!("test_fixtures/simple.osm");

    fn get_expected_nodes() -> &'static [Node] {
        &[
            Node {
                id: -1,
                osm_id: -1,
                lat: -2.73495245962,
                lon: 2.83923666828,
            },
            Node {
                id: -2,
                osm_id: -2,
                lat: -2.73242793496,
                lon: 2.83923765326,
            },
            Node {
                id: -61,
                osm_id: -61,
                lat: -2.72972768022,
                lon: 2.83926371472,
            },
            Node {
                id: -62,
                osm_id: -62,
                lat: -2.72938616409,
                lon: 2.83956383049,
            },
            Node {
                id: -63,
                osm_id: -63,
                lat: -2.72907392744,
                lon: 2.83920771391,
            },
            Node {
                id: -60,
                osm_id: -60,
                lat: -2.72941544366,
                lon: 2.83890759814,
            },
            Node {
                id: -3,
                osm_id: -3,
                lat: -2.73243462926,
                lon: 2.84135576989,
            },
            Node {
                id: -7,
                osm_id: -7,
                lat: -2.72941089232,
                lon: 2.84139901524,
            },
            Node {
                id: -8,
                osm_id: -8,
                lat: -2.72825076265,
                lon: 2.84257281775,
            },
            Node {
                id: -4,
                osm_id: -4,
                lat: -2.73083636925,
                lon: 2.84072562328,
            },
            Node {
                id: -9,
                osm_id: -9,
                lat: -2.72638097685,
                lon: 2.83923674747,
            },
            Node {
                id: -5,
                osm_id: -5,
                lat: -2.73091659085,
                lon: 2.84209711884,
            },
        ]
    }

    fn get_expected_ways() -> Vec<Way> {
        vec![
            Way {
                id: -100,
                nodes: vec![-1, -2],
                tags: tags! {"highway": "primary", "ref": "-100"},
            },
            Way {
                id: -107,
                nodes: vec![-2, -61],
                tags: tags! {"highway": "primary", "motor_vehicle": "no", "ref": "-107"},
            },
            Way {
                id: -108,
                nodes: vec![-63, -60, -61, -62, -63],
                tags: tags! {"highway": "primary", "junction": "roundabout", "ref": "-108"},
            },
            Way {
                id: -101,
                nodes: vec![-2, -3],
                tags: tags! {"highway": "unclassified", "ref": "-101"},
            },
            Way {
                id: -102,
                nodes: vec![-3, -7],
                tags: tags! {"highway": "unclassified", "ref": "-102"},
            },
            Way {
                id: -109,
                nodes: vec![-7, -62],
                tags: tags! {"highway": "unclassified", "ref": "-109"},
            },
            Way {
                id: -110,
                nodes: vec![-8, -7],
                tags: tags! {"highway": "unclassified", "ref": "-110"},
            },
            Way {
                id: -105,
                nodes: vec![-7, -4],
                tags: tags! {"highway": "unclassified", "oneway": "yes", "ref": "-105"},
            },
            Way {
                id: -103,
                nodes: vec![-4, -3],
                tags: tags! {"highway": "motorway", "ref": "-103"},
            },
            Way {
                id: -111,
                nodes: vec![-63, -9],
                tags: tags! {"highway": "primary", "ref": "-111"},
            },
            Way {
                id: -104,
                nodes: vec![-3, -5],
                tags: tags! {"highway": "motorway", "ref": "-104"},
            },
            Way {
                id: -106,
                nodes: vec![-7, -5],
                tags: tags! {"highway": "unclassified", "oneway": "-1", "ref": "-106"},
            },
        ]
    }

    fn get_expected_relations() -> Vec<Relation> {
        return vec![
            Relation {
                id: -200,
                members: vec![
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -110,
                        role: "from".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Node,
                        ref_: -7,
                        role: "via".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -102,
                        role: "to".to_string(),
                    },
                ],
                tags: tags! {"ref": "-200", "restriction": "no_left_turn", "type": "restriction"},
            },
            Relation {
                id: -201,
                members: vec![
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -100,
                        role: "from".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Node,
                        ref_: -2,
                        role: "via".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -101,
                        role: "to".to_string(),
                    },
                ],
                tags: tags! {"ref": "-201", "restriction": "only_right_turn", "type": "restriction"},
            },
            Relation {
                id: -202,
                members: vec![
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -102,
                        role: "from".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Node,
                        ref_: -3,
                        role: "via".to_string(),
                    },
                    RelationMember {
                        type_: FeatureType::Way,
                        ref_: -104,
                        role: "to".to_string(),
                    },
                ],
                tags: tags! {"except": "motorcar", "ref": "-202", "restriction": "no_left_turn", "type": "restriction"},
            },
        ];
    }

    fn collect_all<F: FeatureReader>(
        features: F,
    ) -> Result<(Vec<Node>, Vec<Way>, Vec<Relation>), F::Error> {
        let mut nodes = Vec::default();
        let mut ways = Vec::default();
        let mut relations = Vec::default();

        for f in features {
            match f {
                Ok(Feature::Node(n)) => nodes.push(n),
                Ok(Feature::Way(w)) => ways.push(w),
                Ok(Feature::Relation(r)) => relations.push(r),
                Err(e) => return Err(e),
            }
        }

        Ok((nodes, ways, relations))
    }

    fn check_against_expected<F: FeatureReader>(features: F) -> Result<(), F::Error> {
        let (nodes, ways, relations) = collect_all(features)?;
        assert_eq!(nodes, get_expected_nodes());
        assert_eq!(ways, get_expected_ways());
        assert_eq!(relations, get_expected_relations());
        Ok(())
    }

    #[test]
    fn parse_from_buf() -> Result<(), quick_xml::Error> {
        check_against_expected(Reader::from_buffer(SIMPLE_XML))
    }

    #[test]
    fn parse_from_io() -> Result<(), quick_xml::Error> {
        check_against_expected(Reader::from_io(io::Cursor::new(SIMPLE_XML)))
    }
}
