use geo::{Coord, LineString};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
pub struct RouteSnapperMap {
    #[serde(
        serialize_with = "serialize_coords",
        deserialize_with = "deserialize_coords"
    )]
    pub nodes: Vec<Coord>,
    pub edges: Vec<Edge>,
}

#[derive(Serialize, Deserialize)]
pub struct Edge {
    pub node1: NodeID,
    pub node2: NodeID,
    #[serde(
        serialize_with = "serialize_linestring",
        deserialize_with = "deserialize_linestring"
    )]
    pub geometry: LineString,
    #[serde(skip_serializing, skip_deserializing)]
    pub length_meters: f64,
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EdgeID(pub u32);
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NodeID(pub u32);

impl RouteSnapperMap {
    pub fn edge(&self, id: EdgeID) -> &Edge {
        &self.edges[id.0 as usize]
    }
    pub fn node(&self, id: NodeID) -> Coord {
        self.nodes[id.0 as usize]
    }
}

fn serialize_coords<S: Serializer>(coords: &Vec<Coord>, s: S) -> Result<S::Ok, S::Error> {
    let mut flattened: Vec<i32> = Vec::new();
    for pt in coords {
        flattened.push(serialize_f64(pt.x));
        flattened.push(serialize_f64(pt.y));
    }
    flattened.serialize(s)
}

fn deserialize_coords<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Coord>, D::Error> {
    let flattened = <Vec<i32>>::deserialize(d)?;
    let mut pts = Vec::new();
    for pair in flattened.chunks(2) {
        pts.push(Coord {
            x: deserialize_f64(pair[0]),
            y: deserialize_f64(pair[1]),
        });
    }
    Ok(pts)
}

fn serialize_linestring<S: Serializer>(linestring: &LineString, s: S) -> Result<S::Ok, S::Error> {
    serialize_coords(&linestring.0, s)
}

fn deserialize_linestring<'de, D: Deserializer<'de>>(d: D) -> Result<LineString, D::Error> {
    let pts = deserialize_coords(d)?;
    Ok(LineString::new(pts))
}

/// Serializes a trimmed `f64` as an `i32` to save space.
fn serialize_f64(x: f64) -> i32 {
    // 6 decimal places gives about 10cm of precision
    (x * 1_000_000.0).round() as i32
}

/// Deserializes a trimmed `f64` from an `i32`.
fn deserialize_f64(x: i32) -> f64 {
    x as f64 / 1_000_000.0
}
