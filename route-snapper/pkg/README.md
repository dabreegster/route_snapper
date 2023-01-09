# route-snapper user guide

## Building a graph file

The plugin can draw routes on any
[graph](https://github.com/dabreegster/route_snapper/blob/main/route-snapper-graph/src/lib.rs)
that has coordinates defined for the edges.

A common use case is routing along a street network. You can create an example
file from OpenStreetMap data using
[osm2streets](https://github.com/a-b-street/osm2streets). You need an
`.osm.xml` file, and optionally a GeoJSON file with one polygon representing
the boundary of your area.

You'll need to [install Rust](https://www.rust-lang.org/tools/install) to run this:

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

To initialize the WASM library, you have to `await init()`.

You'll need to get the raw graph file you built. You can do this however you like, such as using [fetch](https://developer.mozilla.org/en-US/docs/Web/API/fetch). For convenience, the JS library includes a helper function `fetchWithProgress` that draws a progress bar to a `div` element you pass in.

To create the route snapper, you need a MapLibre map (it can be initialized or not), the graph, and a `div` element for the plugin to render its controls. From the [example](https://github.com/dabreegster/route_snapper/blob/main/examples/index.html), it might look like this:

```
await init();

const graphBytes = await fetchWithProgress(
  url,
  document.getElementById("snap-progress")
);
let routeSnapper = new RouteSnapper(
  map,
  graphBytes,
  document.getElementById("snap-tool")
);
```

### Events

The above is all you need to get the tool working. To actually get the resulting GeoJSON line-string that the user draws, you listen to the `new-route` event on the `div` element that you passed into the constructor:

```
document.getElementById("snap-tool").addEventListener("new-route", (e) => {
  // A GeoJSON LineString feature with no properties set
  console.log(e.detail);
});
```

There are other events you may care about:

- `activate`: The user clicked the button to start drawing a route
- `no-new-route`: The user started drawing a route, but cancelled or otherwise
  didn't produce any valid result

### API

There are a few methods on the `RouteSnapper` object you can call:

- `isActive()` returns true when the tool is active and interpreting mouse events
- `tearDown()` cleans up the internal sources and layers added to the map.
  (Note it doesn't yet clean up event listeners!)

### MapLibre gotchas

You must specify `boxZoom: false` when creating your
[Map](https://maplibre.org/maplibre-gl-js-docs/api/map/), or shift-click for
drawing freehand points won't work.

### Using with mapbox-gl-draw

For a full example in a Svelte app, see [here](https://github.com/acteng/atip/blob/dcfd6efbc6e5f25060ddd8f449bae5ac1bca672a/components/DrawControls.svelte).

[mapbox-gl-draw](https://github.com/mapbox/mapbox-gl-draw) is a common plugin
for drawing things on a map. There are a few tricks to making `route-snapper`
work with it. While the user is drawing a route, you probably don't want
`mapbox-gl-draw` to interpret mouse events if the route happens to cross some
drawn object.

First you can create a "static mode" using something like [this](https://github.com/mapbox/mapbox-gl-draw-static-mode), to disable all controls for clicking objects and dragging points around. Then you can switch to this whenever the route plugin is active:

```
document.getElementById("snap-tool").addEventListener("activate", () => {
  // Disable interactions with other drawn objects
  drawControls.changeMode("static");
});
document.getElementById("snap-tool").addEventListener("no-new-route", () => {
  // Reactivate interactions
  drawControls.changeMode("simple_select");
});
```

If you want `mapbox-gl-draw` to manage line-strings that the tool produces, you can do this:

```
document.getElementById("snap-tool").addEventListener("new-route", () => {
  let feature = e.detail;
  let ids = drawControls.add(feature);
  // Act like we've selected the line-string we just drew
  drawControls.changeMode("direct_select", {
    featureId: ids[0],
  });
```
