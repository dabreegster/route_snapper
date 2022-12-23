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
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl RouteSnapperMap {
    #[cfg(osm2streets)]
    pub fn from_streets(streets: &osm2streets::StreetNetwork) -> Self {
        let mut map = Self {
            gps_bounds: streets.gps_bounds.clone(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        let mut id_lookup = std::collections::HashMap::new();
        for i in streets.intersections.values() {
            map.nodes.push(i.point);
            id_lookup.insert(i.id, NodeID(id_lookup.len() as u32));
        }
        for r in streets.roads.values() {
            map.edges.push(Edge {
                node1: id_lookup[&r.src_i],
                node2: id_lookup[&r.dst_i],
                geometry: r.reference_line.clone(),
                length: r.reference_line.length(),
            });
        }

        map
    }
}
