// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

/// Describes how to convert OSM data into a [Graph](crate::Graph).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Profile<'a> {
    /// Human readable name of the routing profile,
    /// customary the most specific [access tag](https://wiki.openstreetmap.org/wiki/Key:access).
    ///
    /// This values us not used for actual OSM data interpretation,
    /// except when set to "foot", which adds the following logic:
    /// - `oneway` tags are ignored - only `oneway:foot` tags are considered, except on:
    ///    - `highway=footway`,
    ///    - `highway=path`,
    ///    - `highway=steps`,
    ///    - `highway=platform`
    ///    - `public_transport=platform`,
    ///    - `railway=platform`;
    /// - only `restriction:foot` turn restrictions are considered.
    pub name: &'a str,

    /// Array of tags which OSM ways can be used for routing.
    ///
    /// A way is matched against all [Penalty] objects in order, and
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
    ///   match any [Penalty].
    pub penalties: &'a [Penalty<'a>],

    /// Array of OSM [access tags](https://wiki.openstreetmap.org/wiki/Key:access#Land-based_transportation)
    /// (in order from least to most specific) to consider when checking for road prohibitions.
    ///
    /// This array is used mainly used to follow the access tags, but also to follow mode-specific
    /// one-way and turn restrictions (see [Profile::is_allowed], [Profile::way_direction] and
    /// [Profile::is_exempted]).
    pub access: &'a [&'a str],

    /// Force no routing over [motorroad=yes](https://wiki.openstreetmap.org/wiki/Key:motorroad) ways.
    pub disallow_motorroad: bool,

    /// Force ignoring of [turn restrictions](https://wiki.openstreetmap.org/wiki/Turn_restriction).
    pub disable_restrictions: bool,
}

/// Numeric multiplier for OSM ways with specific keys and values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Penalty<'a> {
    /// Key of an OSM way for which this Penalty applies,
    /// used for [Penalty::value] comparison (e.g. "highway" or "railway")
    pub key: &'a str,

    /// Value under [Penalty::key] of an OSM way for which this Penalty applies.
    /// E.g. "motorway", "residential" or "rail".
    pub value: &'a str,

    /// Multiplier of the length, to express preference for a specific way.
    /// Must be not less than one and a finite floating-point number.
    pub penalty: f32,
}

/// Turn restriction kind indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnRestriction {
    /// Not a turn restriction, or a turn restriction which does not apply for the current [Profile].
    Inapplicable,

    /// The sequence of nodes indicated by this restriction is prohibited.
    Prohibitory,

    /// The sequence of nodes must be followed after using an edge identified by the first two nodes.
    Mandatory,
}

impl<'a> Profile<'a> {
    /// Finds the first matching [Penalty] for a way with given tags.
    /// If there is no matching penalty, or if the way is disallowed
    /// by the access tags (as determined by [Profile::is_allowed]),
    /// returns [f32::INFINITY].
    pub fn way_penalty(&self, tags: &HashMap<String, String>) -> f32 {
        let penalty = self.get_penalty(tags);
        if !penalty.is_normal() || !self.is_allowed(tags) {
            return f32::INFINITY;
        }
        return penalty;
    }

    /// Returns the first matching penalty from way tags, or [f32::INFINITY] otherwise.
    fn get_penalty(&self, tags: &HashMap<String, String>) -> f32 {
        self.penalties
            .iter()
            .find_map(|p| {
                if tags.get(p.key).map(|v| v.as_str()) == Some(p.value) {
                    Some(p.penalty)
                } else {
                    None
                }
            })
            .unwrap_or(f32::INFINITY)
    }

    /// Checks if the way is routable, by considering motor roads ([Profile::disallow_motorroad])
    /// and access tags ([Profile::access]).
    pub fn is_allowed(&self, tags: &HashMap<String, String>) -> bool {
        // Check against the motorroad tag
        if self.disallow_motorroad && tags.get("motorroad").map(|v| v.as_str()) == Some("yes") {
            return false;
        }

        // Check against the access tags
        match self
            .access
            .iter()
            .rev()
            .find_map(|&mode| tags.get(mode).map(|v| v.as_str()))
        {
            Some("no") | Some("private") => false,
            _ => true,
        }
    }

    /// Checks if a way is traversable forward (first return value) and
    /// backwards (second return value) by investigating mode-specific and generic one-way tags.
    ///
    /// Some ways (highway=motorway, highway=motorway_link, junction=roundabout and
    /// junction=circular) default to being one-way, except if overridden by specific tags.
    pub fn way_direction(&self, tags: &HashMap<String, String>) -> (bool, bool) {
        let mut forward = true;
        let mut backward = true;

        // Default one-way ways (foot profile exception - does not apply)
        if !self.apply_foot_exceptions() {
            match tags.get("highway").map(|s| s.as_str()).unwrap_or("") {
                "motorway" | "motorway_link" => {
                    backward = false;
                }
                _ => {}
            }

            match tags.get("junction").map(|s| s.as_str()).unwrap_or("") {
                "roundabout" | "circular" => {
                    backward = false;
                }
                _ => {}
            }
        }

        // Check the oneway tag
        match self.get_active_oneway_value(tags) {
            "yes" | "true" | "1" => {
                forward = true;
                backward = false;
            }

            "-1" | "reverse" => {
                forward = false;
                backward = true;
            }

            "no" => {
                forward = true;
                backward = true;
            }

            _ => {}
        }

        return (forward, backward);
    }

    /// Returns the value of the most specific "oneway:MODE" tag (based on [Profile::access]),
    /// falling back to simply "oneway", and returning an empty string if no relevant tag was found.
    fn get_active_oneway_value<'t>(&self, tags: &'t HashMap<String, String>) -> &'t str {
        if self.apply_foot_exceptions() {
            // foot profile exception - only consider "oneway:foot" and "oneway" in select cases
            if let Some(oneway_foot) = tags.get("oneway:foot") {
                return oneway_foot.as_str();
            }

            if Self::allow_generic_oneway_to_apply_on_foot(tags) {
                if let Some(oneway) = tags.get("oneway") {
                    return oneway.as_str();
                }
            }

            return "";
        } else {
            self.access
                .iter()
                .rev()
                .filter(|&&mode| mode != "access")
                .find_map(|&mode| tags.get(&format!("oneway:{}", mode)))
                .or_else(|| tags.get("oneway"))
                .map(|oneway_tag| oneway_tag.as_str())
                .unwrap_or("")
        }
    }

    fn allow_generic_oneway_to_apply_on_foot(tags: &HashMap<String, String>) -> bool {
        // By default, on foot, only "oneway:foot" is considered. However, on the following
        // ways the generic "oneway" tag also applies.

        // highway=footway, highway=path, highway=steps, highway=platform
        match tags.get("highway").map(|v| v.as_str()) {
            Some("footway") | Some("path") | Some("steps") | Some("platform") => return true,
            _ => {}
        }

        // public_transport=platform
        if tags.get("public_transport").map(|v| v.as_str()) == Some("platform") {
            return true;
        }

        // railway=platform
        if tags.get("railway").map(|v| v.as_str()) == Some("platform") {
            return true;
        }

        // Default to false
        return false;
    }

    /// Figures out what kind of [TurnRestriction] a relation with given tags represents.
    pub fn restriction_kind(&self, tags: &HashMap<String, String>) -> TurnRestriction {
        // Short-circuit when restrictions are disabled,
        // relation is not a restriction, or the current profile is exempted
        if self.disable_restrictions
            || tags.get("type").map(|v| v.as_str()) != Some("restriction")
            || self.is_exempted(tags)
        {
            return TurnRestriction::Inapplicable;
        }

        // Parse the restriction tag
        let (kind, description) = self
            .get_active_restriction_tag(tags)
            .split_once('_')
            .unwrap_or(("", ""));

        // Check that the description is supported
        match description {
            "right_turn" | "left_turn" | "u_turn" | "straight_on" => {}
            _ => return TurnRestriction::Inapplicable,
        }

        // Return the applicable restriction kind
        return match kind {
            "no" => TurnRestriction::Prohibitory,
            "only" => TurnRestriction::Mandatory,
            _ => TurnRestriction::Inapplicable,
        };
    }

    /// Returns true if [Profile::access] intersects with any mode present in the `except` tag.
    /// If the tag is missing, returns false.
    pub fn is_exempted(&self, tags: &HashMap<String, String>) -> bool {
        tags.get("except")
            .map_or("", |v| v.as_str())
            .split(';')
            .any(|exempted_type| self.access.contains(&exempted_type))
    }

    /// Returns the value of the most specific "restriction:MODE" tag (based on [Profile::access]),
    /// falling back to simply "restriction", and returning an empty string if no relevant tag
    /// was found.
    fn get_active_restriction_tag<'t>(&self, tags: &'t HashMap<String, String>) -> &'t str {
        if self.apply_foot_exceptions() {
            // foot profile exception - only consider "restriction:foot"
            tags.get("restriction:foot")
                .map(|v| v.as_str())
                .unwrap_or("")
        } else {
            self.access
                .iter()
                .rev()
                .filter(|&&mode| mode != "access")
                .find_map(|&mode| tags.get(&format!("restriction:{}", mode)))
                .or_else(|| tags.get("restriction"))
                .map(|v| v.as_str())
                .unwrap_or("")
        }
    }

    fn apply_foot_exceptions(&self) -> bool {
        self.name == "foot"
    }
}

/// Example routing [Profile] for cars, with high preference for faster roads
/// and with appropriate [access tags](https://wiki.openstreetmap.org/wiki/Key:access).
pub const CAR_PROFILE: Profile = Profile {
    name: "motorcar",
    penalties: &[
        Penalty {
            key: "highway",
            value: "motorway",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "motorway_link",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "trunk",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "trunk_link",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "primary",
            penalty: 5.0,
        },
        Penalty {
            key: "highway",
            value: "primary_link",
            penalty: 5.0,
        },
        Penalty {
            key: "highway",
            value: "secondary",
            penalty: 6.5,
        },
        Penalty {
            key: "highway",
            value: "secondary_link",
            penalty: 6.5,
        },
        Penalty {
            key: "highway",
            value: "tertiary",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "tertiary_link",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "unclassified",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "minor",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "residential",
            penalty: 15.0,
        },
        Penalty {
            key: "highway",
            value: "living_street",
            penalty: 20.0,
        },
        Penalty {
            key: "highway",
            value: "track",
            penalty: 20.0,
        },
        Penalty {
            key: "highway",
            value: "service",
            penalty: 20.0,
        },
    ],
    access: &["access", "vehicle", "motor_vehicle", "motorcar"],
    disallow_motorroad: false,
    disable_restrictions: false,
};

/// Example routing [Profile] for buses, without high preference differences for different
/// route types and with appropriate [access tags](https://wiki.openstreetmap.org/wiki/Key:access).
pub const BUS_PROFILE: Profile = Profile {
    name: "bus",
    penalties: &[
        Penalty {
            key: "highway",
            value: "motorway",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "motorway_link",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "trunk",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "trunk_link",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "primary",
            penalty: 1.1,
        },
        Penalty {
            key: "highway",
            value: "primary_link",
            penalty: 1.1,
        },
        Penalty {
            key: "highway",
            value: "secondary",
            penalty: 1.15,
        },
        Penalty {
            key: "highway",
            value: "secondary_link",
            penalty: 1.15,
        },
        Penalty {
            key: "highway",
            value: "tertiary",
            penalty: 1.15,
        },
        Penalty {
            key: "highway",
            value: "tertiary_link",
            penalty: 1.15,
        },
        Penalty {
            key: "highway",
            value: "unclassified",
            penalty: 1.5,
        },
        Penalty {
            key: "highway",
            value: "minor",
            penalty: 1.5,
        },
        Penalty {
            key: "highway",
            value: "residential",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "living_street",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "track",
            penalty: 5.0,
        },
        Penalty {
            key: "highway",
            value: "service",
            penalty: 5.0,
        },
    ],
    access: &[
        "access",
        "vehicle",
        "motor_vehicle",
        "psv",
        "bus",
        "routing:ztm",
    ],
    disallow_motorroad: false,
    disable_restrictions: false,
};

/// Example routing [Profile] for bicycles, with preferences for quieter roads
/// and with appropriate [access tags](https://wiki.openstreetmap.org/wiki/Key:access).
pub const BICYCLE_PROFILE: Profile = Profile {
    name: "bicycle",
    penalties: &[
        Penalty {
            key: "highway",
            value: "trunk",
            penalty: 50.0,
        },
        Penalty {
            key: "highway",
            value: "trunk_link",
            penalty: 50.0,
        },
        Penalty {
            key: "highway",
            value: "primary",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "primary_link",
            penalty: 10.0,
        },
        Penalty {
            key: "highway",
            value: "secondary",
            penalty: 3.0,
        },
        Penalty {
            key: "highway",
            value: "secondary_link",
            penalty: 3.0,
        },
        Penalty {
            key: "highway",
            value: "tertiary",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "tertiary_link",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "unclassified",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "minor",
            penalty: 2.5,
        },
        Penalty {
            key: "highway",
            value: "cycleway",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "residential",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "living_street",
            penalty: 1.5,
        },
        Penalty {
            key: "highway",
            value: "track",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "service",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "bridleway",
            penalty: 3.0,
        },
        Penalty {
            key: "highway",
            value: "footway",
            penalty: 3.0,
        },
        Penalty {
            key: "highway",
            value: "steps",
            penalty: 5.0,
        },
        Penalty {
            key: "highway",
            value: "path",
            penalty: 2.0,
        },
    ],
    access: &["access", "vehicle", "bicycle"],
    disallow_motorroad: true,
    disable_restrictions: false,
};

/// Example routing [Profile] for walking, with preferences for quieter roads
/// and with appropriate [access tags](https://wiki.openstreetmap.org/wiki/Key:access).
pub const FOOT_PROFILE: Profile = Profile {
    name: "foot",
    penalties: &[
        Penalty {
            key: "highway",
            value: "trunk",
            penalty: 4.0,
        },
        Penalty {
            key: "highway",
            value: "trunk_link",
            penalty: 4.0,
        },
        Penalty {
            key: "highway",
            value: "primary",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "primary_link",
            penalty: 2.0,
        },
        Penalty {
            key: "highway",
            value: "secondary",
            penalty: 1.3,
        },
        Penalty {
            key: "highway",
            value: "secondary_link",
            penalty: 1.3,
        },
        Penalty {
            key: "highway",
            value: "tertiary",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "tertiary_link",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "unclassified",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "minor",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "residential",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "living_street",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "track",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "service",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "bridleway",
            penalty: 1.2,
        },
        Penalty {
            key: "highway",
            value: "footway",
            penalty: 1.05,
        },
        Penalty {
            key: "highway",
            value: "path",
            penalty: 1.05,
        },
        Penalty {
            key: "highway",
            value: "steps",
            penalty: 1.15,
        },
        Penalty {
            key: "highway",
            value: "pedestrian",
            penalty: 1.0,
        },
        Penalty {
            key: "highway",
            value: "platform",
            penalty: 1.1,
        },
        Penalty {
            key: "railway",
            value: "platform",
            penalty: 1.1,
        },
        Penalty {
            key: "public_transport",
            value: "platform",
            penalty: 1.1,
        },
    ],
    access: &["access", "foot"],
    disallow_motorroad: true,
    disable_restrictions: false,
};

/// Example simple routing [Profile] for different kinds of trains.
pub const RAILWAY_PROFILE: Profile = Profile {
    name: "train",
    penalties: &[
        Penalty {
            key: "railway",
            value: "rail",
            penalty: 1.0,
        },
        Penalty {
            key: "railway",
            value: "light_rail",
            penalty: 1.0,
        },
        Penalty {
            key: "railway",
            value: "subway",
            penalty: 1.0,
        },
        Penalty {
            key: "railway",
            value: "narrow_gauge",
            penalty: 1.0,
        },
    ],
    access: &["access", "train"],
    disallow_motorroad: false,
    disable_restrictions: false,
};

/// Example simple routing [Profile] for routing over subway lines.
pub const TRAM_PROFILE: Profile = Profile {
    name: "train",
    penalties: &[
        Penalty {
            key: "railway",
            value: "tram",
            penalty: 1.0,
        },
        Penalty {
            key: "railway",
            value: "light_rail",
            penalty: 1.0,
        },
    ],
    access: &["access", "tram"],
    disallow_motorroad: false,
    disable_restrictions: false,
};

/// Example simple routing [Profile] for routing over tram and light rail lines.
pub const SUBWAY_PROFILE: Profile = Profile {
    name: "train",
    penalties: &[Penalty {
        key: "railway",
        value: "subway",
        penalty: 1.0,
    }],
    access: &["access", "subway"],
    disallow_motorroad: false,
    disable_restrictions: false,
};

#[cfg(test)]
mod tests {
    use super::{Penalty, Profile, TurnRestriction, FOOT_PROFILE};
    use std::collections::HashMap;

    const TEST_PROFILE: Profile = Profile {
        name: "cat",
        penalties: &[
            Penalty {
                key: "highway",
                value: "footway",
                penalty: 1.0,
            },
            Penalty {
                key: "highway",
                value: "path",
                penalty: 2.0,
            },
        ],
        access: &["access", "cat"],
        disallow_motorroad: false,
        disable_restrictions: false,
    };

    const TEST_PROFILE_WITHOUT_MOTORROAD: Profile = Profile {
        name: "cat",
        penalties: &[
            Penalty {
                key: "highway",
                value: "footway",
                penalty: 1.0,
            },
            Penalty {
                key: "highway",
                value: "path",
                penalty: 2.0,
            },
        ],
        access: &["access", "cat"],
        disallow_motorroad: true,
        disable_restrictions: false,
    };

    const TEST_PROFILE_WITHOUT_RESTRICTIONS: Profile = Profile {
        name: "cat",
        penalties: &[
            Penalty {
                key: "highway",
                value: "footway",
                penalty: 1.0,
            },
            Penalty {
                key: "highway",
                value: "path",
                penalty: 2.0,
            },
        ],
        access: &["access", "cat"],
        disallow_motorroad: false,
        disable_restrictions: true,
    };

    macro_rules! tags {
        {} => { HashMap::default() };
        {$( $k:literal : $v:literal ),+} => {
            HashMap::from_iter([ $( ($k.to_string(), $v.to_string()) ),+ ])
        };
    }

    #[test]
    fn way_penalty() {
        assert_eq!(TEST_PROFILE.way_penalty(&tags! {"highway": "footway"}), 1.0);
        assert_eq!(TEST_PROFILE.way_penalty(&tags! {"highway": "path"}), 2.0);
        assert_eq!(
            TEST_PROFILE.way_penalty(&tags! {"highway": "motorway"}),
            f32::INFINITY,
        );
        assert_eq!(TEST_PROFILE.way_penalty(&tags! {}), f32::INFINITY);
        assert_eq!(
            TEST_PROFILE.way_penalty(&tags! {"highway": "path", "access": "no"}),
            f32::INFINITY,
        );
        assert_eq!(
            TEST_PROFILE
                .way_penalty(&tags! {"highway": "path", "access": "no", "cat": "destination"}),
            2.0,
        );
        assert_eq!(
            TEST_PROFILE.way_penalty(&tags! {"highway": "path", "motorroad": "yes"}),
            2.0,
        );
        assert_eq!(
            TEST_PROFILE_WITHOUT_MOTORROAD
                .way_penalty(&tags! {"highway": "path", "motorroad": "yes"}),
            f32::INFINITY,
        );
    }

    #[test]
    fn is_allowed() {
        assert!(TEST_PROFILE.is_allowed(&tags! {"highway": "footway"}));
        assert!(!TEST_PROFILE.is_allowed(&tags! {"highway": "footway", "access": "no"}));
        assert!(!TEST_PROFILE.is_allowed(&tags! {"highway": "footway", "access": "private"}));
        assert!(TEST_PROFILE.is_allowed(&tags! {"highway": "footway", "access": "destination"}));
        assert!(
            TEST_PROFILE.is_allowed(&tags! {"highway": "footway", "access": "no", "cat": "yes"})
        );
        assert!(TEST_PROFILE.is_allowed(&tags! {"highway": "footway", "motorroad": "yes"}));
        assert!(!TEST_PROFILE_WITHOUT_MOTORROAD
            .is_allowed(&tags! {"highway": "footway", "motorroad": "yes"}));
    }

    #[test]
    fn way_direction() {
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "path"}),
            (true, true),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "path", "oneway": "yes"}),
            (true, false),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "path", "oneway": "-1"}),
            (false, true),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "motorway_link"}),
            (true, false),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "path", "junction": "roundabout"}),
            (true, false),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"highway": "motorway_link", "oneway": "no"}),
            (true, true),
        );
        assert_eq!(
            TEST_PROFILE.way_direction(&tags! {"junction": "circular", "oneway": "-1"}),
            (false, true),
        );
    }

    #[test]
    fn way_direction_foot() {
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "residential"}),
            (true, true),
        );
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "residential", "oneway": "yes"}),
            (true, true),
        );
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "residential", "oneway:foot": "yes"}),
            (true, false),
        );
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "residential", "oneway:foot": "-1"}),
            (false, true),
        );
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "path", "oneway": "yes"}),
            (true, false),
        );
        assert_eq!(
            FOOT_PROFILE.way_direction(&tags! {"highway": "footway", "oneway": "-1"}),
            (false, true),
        );
    }

    #[test]
    fn restriction_kind() {
        assert_eq!(
            TEST_PROFILE.restriction_kind(&tags! {"type": "multipolygon"}),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            TEST_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction": "no_u_turn"}),
            TurnRestriction::Prohibitory,
        );
        assert_eq!(
            TEST_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction": "only_left_turn"}),
            TurnRestriction::Mandatory,
        );
        assert_eq!(
            TEST_PROFILE.restriction_kind(
                &tags! {"type": "restriction", "restriction": "only_left_turn", "except": "psv;cat"}
            ),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            TEST_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction": "only_360"}),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            TEST_PROFILE_WITHOUT_RESTRICTIONS
                .restriction_kind(&tags! {"type": "restriction", "restriction": "no_u_turn"}),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            TEST_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction:car": "no_u_turn"}),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            TEST_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction:cat": "no_u_turn"}),
            TurnRestriction::Prohibitory,
        );
    }

    #[test]
    fn restriction_kind_foot() {
        assert_eq!(
            FOOT_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction": "no_u_turn"}),
            TurnRestriction::Inapplicable,
        );
        assert_eq!(
            FOOT_PROFILE
                .restriction_kind(&tags! {"type": "restriction", "restriction:foot": "no_u_turn"}),
            TurnRestriction::Prohibitory,
        );
    }

    #[test]
    fn is_exempted() {
        assert!(!TEST_PROFILE.is_exempted(&tags! {}));
        assert!(!TEST_PROFILE.is_exempted(&tags! {"except": "car"}));
        assert!(TEST_PROFILE.is_exempted(&tags! {"except": "cat"}));
        assert!(TEST_PROFILE.is_exempted(&tags! {"except": "psv;cat"}));
    }
}
