// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

mod fileformat;
mod osmformat;

use super::model::{Feature, FeatureType, Relation, RelationMember, Way};
use crate::Node;

use protobuf::Message;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;

/// Max permitted size for a serialized [blob header](https://wiki.openstreetmap.org/wiki/PBF_Format#File_format) -
/// 64 KiB.
const MAX_BLOB_HEADER_SIZE: u32 = 64 * 1024;

/// Max permitted size for a serialized & decompressed [blob](https://wiki.openstreetmap.org/wiki/PBF_Format#File_format) -
/// 32 MiB.
const MAX_BLOB_SIZE: u32 = 32 * 1024 * 1024;

/// All strings used by an [OSM PBF Block](https://wiki.openstreetmap.org/wiki/PBF_Format#Definition_of_OSMData_fileblock),
/// reference-counted as this table is referred to by multiple coexisting iterators and
/// closures without any concrete ownership.
type StringTable = Rc<Vec<String>>;

/// Error which can occur when reading a PBF file.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("protobuf: {0}")]
    Protobuf(#[from] Arc<protobuf::Error>),

    #[error("io: {0}")]
    Io(#[from] Arc<io::Error>),

    #[error("BlobHeader too large: {0} > {MAX_BLOB_HEADER_SIZE}")]
    BlobHeaderTooLarge(u32),

    #[error("Blob too large: {0} > {MAX_BLOB_SIZE}")]
    BlobTooLarge(u32),

    #[error("BlobHeader.type: got {got:?}, expected {expected:?}")]
    UnexpectedBlobHeaderType { got: String, expected: &'static str },

    #[error("BlobHeader.datasize is negative")]
    NegativeBlobHeaderSize,

    #[error("unsupported compression: {0} (supported: raw, zlib and bzip2)")]
    UnsupportedCompression(&'static str),

    #[error("file requires unsupported features: {0:?}")]
    UnsupportedFeatures(Vec<String>),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(Arc::new(e))
    }
}

impl From<protobuf::Error> for Error {
    fn from(e: protobuf::Error) -> Self {
        Error::Protobuf(Arc::new(e))
    }
}

/// Returns an iterator over all features from an OSM PBF file.
pub fn features_from_file<R: io::Read>(reader: R) -> impl Iterator<Item = Result<Feature, Error>> {
    File(reader).features()
}

/// File abstracts away a whole OSM PBF file, a file encoding multiple [blocks](osmformat::PrimitiveBlock).
/// [fileformat::Blob] pairs, into a friendly interface.
struct File<R: io::Read>(R);

impl<R: io::Read> File<R> {
    /// Returns an iterator over all [Blocks](Block) in this file.
    fn blocks(self) -> impl Iterator<Item = Result<Block, Error>> {
        FileBlocks {
            reader: self.0,
            done: false,
        }
    }

    /// Returns a flattened iterator over all [Features](Feature) from all
    /// [Groups](Group) from all [Blocks](Block) in this file.
    fn features(self) -> impl Iterator<Item = Result<Feature, Error>> {
        self.blocks().flat_map(block_result_features)
    }
}

/// Iterator over [Blocks](Block) in a [File].
struct FileBlocks<R: io::Read> {
    reader: R,
    done: bool,
}

impl<R: io::Read> Iterator for FileBlocks<R> {
    type Item = Result<Block, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let result = match self.read_and_check_header() {
                Ok(true) => Some(self.read_data()),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            };

            self.done = match &result {
                None | Some(Err(_)) => true,
                Some(Ok(_)) => false,
            };

            result
        }
    }
}

impl<R: io::Read> FileBlocks<R> {
    /// Reads the next size + [fileformat::BlobHeader] + [fileformat::Blob] sequence,
    /// expecting an `OSMHeader` block containing an [osmformat::HeaderBlock].
    ///
    /// Returns `Ok(true)` if a header block was successfully read and validated,
    /// `Ok(false)` on EOF, or an [Error] if anything bad has happened.
    fn read_and_check_header(&mut self) -> Result<bool, Error> {
        // 1. Read the BlobHeader size
        let blob_header_size = match self.read_blob_header_size()? {
            Some(size) => size,
            None => return Ok(false), // no more blobs
        };

        // 2. Read the BlobHeader
        let blob_header = self.read_blob_header(blob_header_size)?;

        // 2.1. Verify the BlobHeader.type
        if blob_header.type_() != "OSMHeader" {
            return Err(Error::UnexpectedBlobHeaderType {
                got: blob_header.type_.unwrap_or_default(),
                expected: "OSMHeader",
            });
        }

        // 3. Read the OSMHeader blob
        let blob = self.read_blob(blob_header.datasize())?;
        let header = osmformat::HeaderBlock::parse_from_bytes(&blob)?;

        // 3.1. Check required features
        let mut unknown_features = Vec::new();
        for required_feature in &header.required_features {
            match required_feature.as_str() {
                "OsmSchema-V0.6" | "DenseNodes" => {}
                other => unknown_features.push(other.to_string()),
            }
        }
        if !unknown_features.is_empty() {
            return Err(Error::UnsupportedFeatures(unknown_features));
        }

        // OSMHeader blob read and verified - proceed to read PrimitiveBlock
        Ok(true)
    }

    /// Reads the next size + [fileformat::BlobHeader] + [fileformat::Blob] sequence,
    /// expecting an `OSMData` block containing an [Block] ([osmformat::PrimitiveBlock]).
    fn read_data(&mut self) -> Result<Block, Error> {
        // 1. Read the BlobHeader size
        let blob_header_size = match self.read_blob_header_size()? {
            Some(size) => size,
            None => {
                return Err(Error::Io(Arc::new(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "expected BlobHeader for PrimitiveBlock, got EOF",
                ))))
            }
        };

        // 2. Read the BlobHeader
        let blob_header = self.read_blob_header(blob_header_size)?;

        // 2.1. Verify the BlobHeader.type
        if blob_header.type_() != "OSMData" {
            return Err(Error::UnexpectedBlobHeaderType {
                got: blob_header.type_.unwrap_or_default(),
                expected: "OSMData",
            });
        }

        // 3. Read the PrimitiveBlock blob
        let blob = self.read_blob(blob_header.datasize())?;
        let block = osmformat::PrimitiveBlock::parse_from_bytes(&blob)?;
        Ok(Block(block))
    }

    /// Reads the next 4 bytes to read the size of the subsequent [fileformat::BlobHeader].
    ///
    /// Returns `Ok(Some(_))` on success, `Ok(None)` on EOF, or an [Error].
    fn read_blob_header_size(&mut self) -> Result<Option<u32>, Error> {
        let mut buf = [0u8; 4];
        match self.reader.read_exact(&mut buf) {
            Ok(_) => Ok(Some(u32::from_be_bytes(buf))),
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    Ok(None) // no more blobs
                } else {
                    Err(Error::Io(Arc::new(e)))
                }
            }
        }
    }

    /// Reads the next [fileformat::BlobHeader] of a given size.
    fn read_blob_header(&mut self, size: u32) -> Result<fileformat::BlobHeader, Error> {
        if size > MAX_BLOB_HEADER_SIZE {
            return Err(Error::BlobHeaderTooLarge(size));
        }
        let mut buf = vec![0u8; size as usize];
        self.reader.read_exact(&mut buf)?;
        let header = fileformat::BlobHeader::parse_from_bytes(&buf)?;
        Ok(header)
    }

    /// Reads the next [fileformat::Blob] and returns the decompressed contents of it.
    fn read_blob(&mut self, size: i32) -> Result<Vec<u8>, Error> {
        if size < 0 {
            return Err(Error::NegativeBlobHeaderSize);
        }

        let mut buf = vec![0u8; size as usize];
        self.reader.read_exact(&mut buf)?;

        let blob = fileformat::Blob::parse_from_bytes(&buf)?;

        // FIXME: Don't blindly trust `blob.raw_size` for detecting too large blobs.
        //        There should be a way to prevent too large allocations during decompression.
        let blob_size = blob.raw_size() as u32;
        if blob_size > MAX_BLOB_SIZE {
            return Err(Error::BlobTooLarge(blob_size));
        }

        match blob
            .data
            .expect("Blob.data must not be None after parse_from_bytes")
        {
            fileformat::blob::Data::Raw(data) => Ok(data),

            fileformat::blob::Data::ZlibData(data) => {
                let mut d = flate2::read::ZlibDecoder::new(&data[..]);
                let mut decompressed = Vec::with_capacity(blob_size as usize);
                d.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            }

            fileformat::blob::Data::LzmaData(_) => Err(Error::UnsupportedCompression("lzma")),

            fileformat::blob::Data::OBSOLETEBzip2Data(data) => {
                let mut d = bzip2::read::BzDecoder::new(&data[..]);
                let mut decompressed = Vec::with_capacity(blob_size as usize);
                d.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            }

            fileformat::blob::Data::Lz4Data(_) => Err(Error::UnsupportedCompression("lz4")),

            fileformat::blob::Data::ZstdData(_) => Err(Error::UnsupportedCompression("zstd")),
        }
    }
}

/// Wrapper for a union of any [Feature] iterator with `std::iter::once<Error>`.
enum BlockResultFeatureIterator<I: Iterator<Item = Feature>> {
    Iterating(I),
    Done(Option<Error>),
}

impl<I: Iterator<Item = Feature>> Iterator for BlockResultFeatureIterator<I> {
    type Item = Result<Feature, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Iterating(i) => match i.next() {
                Some(f) => Some(Ok(f)),
                None => {
                    *self = Self::Done(None);
                    None
                }
            },

            Self::Done(e) => e.take().map(|e| Err(e)),
        }
    }
}

fn block_result_features(
    block_result: Result<Block, Error>,
) -> BlockResultFeatureIterator<impl Iterator<Item = Feature>> {
    match block_result {
        Ok(block) => BlockResultFeatureIterator::Iterating(block.features()),
        Err(e) => BlockResultFeatureIterator::Done(Some(e)),
    }
}

/// Block abstracts away an [osmformat::PrimitiveBlock] into a friendly interface.
struct Block(osmformat::PrimitiveBlock);

impl Block {
    /// Returns an iterator over all [Groups](Group) in this block.
    fn groups(self) -> impl Iterator<Item = Group> {
        let coordinate_converter = self.build_coordinate_converter();
        let string_table = Rc::new(self.build_string_table());
        self.0.primitivegroup.into_iter().map(move |g| Group {
            primitive_group: g,
            coordinate_converter: coordinate_converter,
            string_table: string_table.clone(),
        })
    }

    /// Returns a flattened iterator over all [Features](Feature) from all [Groups](Group) in this block.
    fn features(self) -> impl Iterator<Item = Feature> {
        self.groups().flat_map(|g| g.features())
    }

    /// Converts the [osmformat::StringTable] into a simpler `Vec<String>`.
    fn build_string_table(&self) -> Vec<String> {
        self.0
            .stringtable
            .s
            .iter()
            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
            .collect()
    }

    /// Builds a [CoordinateConverter] for this block.
    fn build_coordinate_converter(&self) -> CoordinateConverter {
        CoordinateConverter {
            lat_offset: self.0.lat_offset(),
            lon_offset: self.0.lon_offset(),
            granularity: self.0.granularity() as i64,
        }
    }
}

/// Group abstracts away an [osmformat::PrimitiveGroup] into a friendly interface.
struct Group {
    primitive_group: osmformat::PrimitiveGroup,
    coordinate_converter: CoordinateConverter,
    string_table: StringTable,
}

impl Group {
    /// Returns a flattened iterator over all [Features](Feature) in this group.
    fn features(self) -> impl Iterator<Item = Feature> {
        let nodes =
            Self::nodes(self.primitive_group.nodes, self.coordinate_converter).map(Feature::Node);

        let dense_nodes = Self::dense_nodes(
            self.primitive_group.dense.unwrap_or_default(),
            self.coordinate_converter,
        )
        .map(Feature::Node);

        let ways =
            Self::ways(self.primitive_group.ways, self.string_table.clone()).map(Feature::Way);

        let relations = Self::relations(self.primitive_group.relations, self.string_table)
            .map(Feature::Relation);

        nodes.chain(dense_nodes).chain(ways).chain(relations)
    }

    /// Returns an iterator over all standard (non-dense-encoded) [nodes](Node) from a moved
    /// vector of [raw nodes](osmformat::Node).
    fn nodes(
        raw_nodes: Vec<osmformat::Node>,
        coordinate_converter: CoordinateConverter,
    ) -> impl Iterator<Item = Node> {
        raw_nodes.into_iter().map(move |node| Node {
            id: node.id(),
            osm_id: node.id(),
            lat: coordinate_converter.convert_lat(node.lat()),
            lon: coordinate_converter.convert_lon(node.lon()),
        })
    }

    /// Returns an iterator over all dense-encoded [nodes](Node) from a moved [raw dense nodes](osmformat::DenseNodes).
    fn dense_nodes(
        raw_dense_nodes: osmformat::DenseNodes,
        coordinate_converter: CoordinateConverter,
    ) -> impl Iterator<Item = Node> {
        let ids = raw_dense_nodes.id.into_iter().scan(0, |acc, delta| {
            *acc += delta;
            Some(*acc)
        });

        let lats = raw_dense_nodes.lat.into_iter().scan(0, move |acc, delta| {
            *acc += delta;
            Some(coordinate_converter.convert_lat(*acc))
        });

        let lons = raw_dense_nodes.lon.into_iter().scan(0, move |acc, delta| {
            *acc += delta;
            Some(coordinate_converter.convert_lon(*acc))
        });

        ids.zip(lats.zip(lons)).map(|(id, (lat, lon))| Node {
            id,
            osm_id: id,
            lat,
            lon,
        })
    }

    /// Returns an iterator over all [ways](Way) from a moved vector of [raw ways](osmformat::Way).
    fn ways(raw_ways: Vec<osmformat::Way>, string_table: StringTable) -> impl Iterator<Item = Way> {
        raw_ways.into_iter().map(move |way| Way {
            id: way.id(),
            nodes: collect_way_nodes(&way.refs),
            tags: collect_tags(&way.keys, &way.vals, &string_table),
        })
    }

    /// Returns an iterator over all [relations](Relation) from a moved vector of [raw relations](osmformat::Relation).
    fn relations(
        raw_relations: Vec<osmformat::Relation>,
        string_table: StringTable,
    ) -> impl Iterator<Item = Relation> {
        raw_relations.into_iter().map(move |relation| Relation {
            id: relation.id(),
            members: collect_relation_members(
                &relation.memids,
                &relation.roles_sid,
                &relation.types,
                &string_table,
            ),
            tags: collect_tags(&relation.keys, &relation.vals, &string_table),
        })
    }
}

/// Converts latitudes and longitudes from OSM PBF representation to standard `f32` degrees.
#[derive(Clone, Copy)]
struct CoordinateConverter {
    lat_offset: i64,
    lon_offset: i64,
    granularity: i64,
}

impl CoordinateConverter {
    fn convert_lat(&self, value: i64) -> f32 {
        (self.lat_offset + self.granularity * value) as f32 * 1e-9
    }

    fn convert_lon(&self, value: i64) -> f32 {
        (self.lon_offset + self.granularity * value) as f32 * 1e-9
    }
}

fn collect_tags(keys: &[u32], values: &[u32], string_table: &[String]) -> HashMap<String, String> {
    keys.iter()
        .zip(values.iter())
        .map(|(&key_idx, &value_idx)| {
            (
                get_string(string_table, key_idx),
                get_string(string_table, value_idx),
            )
        })
        .collect()
}

fn collect_way_nodes(ref_deltas: &[i64]) -> Vec<i64> {
    ref_deltas
        .iter()
        .scan(0, |acc, &delta| {
            *acc += delta;
            Some(*acc)
        })
        .collect()
}

fn collect_relation_members(
    member_id_deltas: &[i64],
    roles: &[i32],
    types: &[protobuf::EnumOrUnknown<osmformat::relation::MemberType>],
    string_table: &[String],
) -> Vec<RelationMember> {
    member_id_deltas
        .iter()
        .scan(0, |acc, &delta| {
            *acc += delta;
            Some(*acc)
        })
        .zip(roles.iter().zip(types.iter()))
        .map(|(ref_, (&role_idx, &type_))| RelationMember {
            ref_,
            type_: match type_.unwrap() {
                osmformat::relation::MemberType::NODE => FeatureType::Node,
                osmformat::relation::MemberType::WAY => FeatureType::Way,
                osmformat::relation::MemberType::RELATION => FeatureType::Relation,
            },
            role: get_string(string_table, role_idx as u32),
        })
        .collect()
}

#[inline]
fn get_string(table: &[String], idx: u32) -> String {
    table.get(idx as usize).cloned().unwrap_or_default()
}
