# User guide

## Building a graph file

The plugin can draw routes on any
[graph](https://github.com/dabreegster/route_snapper/blob/main/route-snapper-graph/src/lib.rs)
that has coordinates defined for the edges.

A common use case is routing along a street network. You can create an example
file from OpenStreetMap data using
[osm2streets](https://github.com/a-b-street/osm2streets). You need an
`.osm.xml` file, and optionally a GeoJSON file with one polygon representing
the boundary of your area.

```
cd osm-to-route-snapper
cargo run --release \
  -i path_to_osm.xml \
  [-b path_to_boundary.geojson]
```

## MapLibre gotchas

You must specify `boxZoom: false` when creating your
[Map](https://maplibre.org/maplibre-gl-js-docs/api/map/), or shift-click for
drawing freehand points won't work.
