use geom::{Distance, GPSBounds, PolyLine, Pt2D};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RouteSnapperMap {
    pub gps_bounds: GPSBounds,
    pub nodes: Vec<Pt2D>,
    pub edges: Vec<Edge>,
}

#[derive(Serialize, Deserialize)]
pub struct Edge {
    pub node1: NodeID,
    pub node2: NodeID,
    pub geometry: PolyLine,
    #[serde(skip_serializing, skip_deserializing)]
    pub length: Distance,
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
    pub fn node(&self, id: NodeID) -> Pt2D {
        self.nodes[id.0 as usize]
    }
}
