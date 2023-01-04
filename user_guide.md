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

## Adding to a MapLibre app

See [the end-to-end
example](https://github.com/dabreegster/route_snapper/blob/main/examples/index.html).

### Installation

If you're using NPM, do `npm i route-snapper` and then in your JS:

```
import { init, RouteSnapper, fetchWithProgress } from "route-snapper/lib.js";
```

You can also load from a CDN:

```
import {
  init,
  RouteSnapper,
  fetchWithProgress,
} from "https://unpkg.com/route-snapper/lib.js";
```

### Setup

The div, fetchWithProgress, constructing the tool

### Events

activate, new-route, no-new-route

### MapLibre gotchas

You must specify `boxZoom: false` when creating your
[Map](https://maplibre.org/maplibre-gl-js-docs/api/map/), or shift-click for
drawing freehand points won't work.

### Using with mapbox-gl-draw

[mapbox-gl-draw](https://github.com/mapbox/mapbox-gl-draw) is a common plugin
for drawing things on a map. There are a few tricks to making `route-snapper`
work with it.

static mode...
