# User guide

## Building a graph file

The plugin can draw routes on any
[graph](https://github.com/dabreegster/route_snapper/blob/main/route-snapper-graph/src/lib.rs)
that has coordinates defined for the edges.

A common use case is routing along a street network. You can create an example
file from OpenStreetMap data using
[osm2streets](https://github.com/a-b-street/osm2streets). You need an
`.osm.xml` file and a GeoJSON file with one polygon representing the boundary
of your area.

```
cd osm-to-route-snapper
cargo run --release \
  path_to_osm.xml \
  path_to_boundary.geojson
```
