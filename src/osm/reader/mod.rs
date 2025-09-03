// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

use graph_builder::GraphBuilder;

use crate::osm::Profile;
use crate::Graph;

mod graph_builder;
mod model;
mod xml;

/// Format of the input OSM file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Unknown format - guess the format based on the content
    Unknown,

    /// Force uncompressed [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    Xml,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [gzip](https://en.wikipedia.org/wiki/Gzip) compression
    XmlGz,

    /// Force [OSM XML](https://wiki.openstreetmap.org/wiki/OSM_XML)
    /// with [bzip2](https://en.wikipedia.org/wiki/Bzip2) compression
    XmlBz2,

    /// Force [OSM PBF](https://wiki.openstreetmap.org/wiki/PBF_Format)
    Pbf,
}

/// Additional controls for interpreting OSM data as a routing [Graph].
#[derive(Debug)]
pub struct Options<'a> {
    /// How OSM features should be interpreted and converted into a [Graph].
    pub profile: &'a Profile<'a>,

    /// Format of the input data. Currently, only [FileFormat::Xml] is supported.
    pub file_format: FileFormat,

    /// Filter features by a specific bounding box. In order: left (min lon), bottom (min lat),
    /// right (max lon), top (max lat). Ignored if all values are set to zero, or at least one
    /// of them is not finite.
    pub bbox: [f32; 4],
}

/// Internal trait for objects which can stream [osm features](model::Feature)
/// from an underlying source.
trait FeatureReader {
    type Error;
    fn next(&mut self) -> Result<Option<model::Feature>, Self::Error>;
}

/// Parse OSM features from a file at the provided path into a [Graph] as per the provided [Options].
pub fn add_features_from_file<'a, P: AsRef<Path>>(
    g: &'a mut Graph,
    options: &'a Options<'a>,
    path: P,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        options.file_format,
        FileFormat::Xml,
        "unsupported file format {:?} (only xml is currently supported)",
        options.file_format
    );

    let f = File::open(path)?;
    let b = io::BufReader::new(f);
    let r = xml::Reader::from_io(b);
    GraphBuilder::new(g, options).add_features(r)?;
    Ok(())
}

/// Parse OSM features from a reader into a [Graph] as per the provided [Options].
pub fn add_features_from_io<'a, R: io::BufRead>(
    g: &'a mut Graph,
    options: &'a Options<'a>,
    reader: R,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        options.file_format,
        FileFormat::Xml,
        "unsupported file format {:?} (only xml is currently supported)",
        options.file_format
    );

    let r = xml::Reader::from_io(reader);
    GraphBuilder::new(g, options).add_features(r)?;
    Ok(())
}

/// Parse OSM features from a static buffer into a [Graph] as per the provided [Options].
pub fn add_features_from_buffer<'a>(
    g: &'a mut Graph,
    options: &'a Options<'a>,
    data: &[u8],
) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        options.file_format,
        FileFormat::Xml,
        "unsupported file format {:?} (only xml is currently supported)",
        options.file_format
    );

    let r = xml::Reader::from_buffer(data);
    GraphBuilder::new(g, options).add_features(r)?;
    Ok(())
}
