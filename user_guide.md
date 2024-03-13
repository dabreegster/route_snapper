# route-snapper user guide

## Building a graph file

The plugin can draw routes on any
[graph](https://github.com/dabreegster/route_snapper/blob/main/route-snapper-graph/src/lib.rs)
that has coordinates defined for the edges.

### From OpenStreetMap data

A common use case is routing along a street network. You can create an example
file from OpenStreetMap data. The easiest way to do this for smaller areas is
[in your web browser](https://dabreegster.github.io/route_snapper/import.html).

For larger areas, you need an `.osm.xml` or `.osm.pbf` file, and optionally a
GeoJSON file with one polygon representing the boundary of your area. You'll
need to [install Rust](https://www.rust-lang.org/tools/install) to run this:

```
cd osm-to-route-snapper
cargo run --release \
  -i path_to_osm.xml \
  [-b path_to_boundary.geojson]
```

### From custom GeoJSON files

If you have a GeoJSON file with LineStrings representing routable edges in a
network, you can turn this into a graph too. Try first [in your web
browser](https://dabreegster.github.io/route_snapper/import.html) using the
button at the top. For larger areas, install Rust and then:

```
cd geojson-to-route-snapper
cargo run --release -- --input path_to_network.geojson
```

The LineStrings must represent edges, meaning the ends need to have the same
coordinate as other LineStrings. Each LineString has some properties:

- an optional numeric `forward_cost` and `backward_cost`.

  Costs **must** be specified for some of the edges in the file. 
  If a cost is missing, the edge won't be routable in that direction.
  
- an optional string `name`.

Unlike the OpenStreetMap importer, distance is not used as a default cost.

## Adding to a MapLibre app

See [the end-to-end
example](https://github.com/dabreegster/route_snapper/blob/main/examples/index.html).

### Installation

If you're using NPM, do `npm i route-snapper` and then in your JS:

```
import { init, RouteSnapper } from "route-snapper/lib.js";
```

You can also load from a CDN:

```
import { init, RouteSnapper } from "https://unpkg.com/route-snapper/lib.js";
```

### Setup

To initialize the WASM library, you have to `await init()`.

You'll need to get the raw graph file you built. You can do this however you like, such as using [fetch](https://developer.mozilla.org/en-US/docs/Web/API/fetch).

To create the route snapper, you need a MapLibre map (it can be initialized or not), the graph, and a `div` element for the plugin to render its controls. From the [example](https://github.com/dabreegster/route_snapper/blob/main/examples/index.html), it might look like this:

```
await init();

let resp = await fetch(url);
let graphBytes = await resp.arrayBuffer();
let routeSnapper = new RouteSnapper(
  map,
  new Uint8Array(graphBytes),
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

Note `activate` isn't fired if you manually call `start()` or `editExisting()`,
only when the button is pressed. These details are subject to change before the
next major version.

### API

There are a few methods on the `RouteSnapper` object you can call:

- `isActive()` returns true when the tool is active and interpreting mouse events
- `tearDown()` cleans up the internal sources and layers added to the map.
  (Note it doesn't yet clean up event listeners!)
- `setRouteConfig` to change some settings for drawing routes
  - `avoid_doubling_back` (disabled by default): When possible, avoid edges
    already crossed for handling intermediate waypoints
  - `extend_route` (disabled by default): The user can keep clicking to extend the end of the route. When false, the user can only draw two endpoints, then drag intermediate points.
- `setAreaMode()` changes to producing polygons instead of line-strings.
- `editExisting` to restart the tool with a previously created route. See notes
  in [the example](https://github.com/dabreegster/route_snapper/blob/main/examples/index.html)
  about how to call it.
- `start` activates the tool. It has no effect if the tool is already started.
- `stop` deactivates the tool and clears all state
- `debugRenderGraph` returns GeoJSON points and line-strings to debug the graph used for routing.
- `changeGraph` can be used after initialization to change the loaded graph. It
  takes `graphBytes`, same as the constructor.
- `routeNameForWaypoints` takes the `feature.properties.waypoints` and returns
  a name describing the first and last waypoint (useful only for snapped
  waypoints).

### WASM API

If you're using the WASM API directly, the best reference is currently [the code](https://github.com/dabreegster/route_snapper/blob/main/route-snapper/src/lib.rs). Some particulars:

- `renderGeojson` returns a GeoJSON FeatureCollection to render the current state of the tool.
  - It'll include LineStrings showing the confirmed route and also any speculative addition, based on the current state. The LineStrings will have a boolean `snapped` property, which is false if either end touches a freehand point.
  - In area mode, it'll have a Polygon once there are at least 3 points.
  - It'll include a Point for every graph node involved in the current route. These will have a `type` property that's either `snapped-waypoint`, `free-waypoint`, or just `node` to indicate a draggable node that hasn't been touched yet. One Point may also have a `"hovered": true` property to indicate the mouse is currently on that Point. Points may also have a `name` property with the road names for that intersection.
  - The GeoJSON object will have some additional foreign members:
    - `cursor`, indicating the current mode of the tool. The values can be set to `map.getCanvas().style.cursor` as desired.
      - `inherit`: The user is just idling on the map, not interacting with the map
      - `pointer`: The user is hovering on some node
      - `grabbing`: The user is actively dragging a node
      - `crosshair`: The user is choosing a location for a new freehand point. If they click, the point will be added.
    - A boolean `snap_mode`
    - A numeric `undo_length`
- `toggleSnapMode` attempts to switch between snapping and freehand drawing. It may not succeed.
- `addSnappedWaypoint` adds a new waypoint to the end of the route, snapping to the nearest node. It's useful for clients to hook up a geocoder and add a point by address. Unsupported in area mode.

### MapLibre gotchas

You must specify `boxZoom: false` when creating your
[Map](https://maplibre.org/maplibre-gl-js-docs/api/map/), or shift-click for
drawing freehand points won't work. Likewise, you need to disable
`doubleClickZoom` so that you can double click to end a route.

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

## Routing caveats

The routes calculated by the tool are based on the input graph. The default
option described above pulls in road segments from OpenStreetMap for many
modes, including tram or light-rail, walking or cycling only paths, and
`highway=construction`. The "optimal" paths drawn by the tool are based on
Euclidean distance -- no speed limits, safety of following the route by some
user, etc is attempted. The route may violate one-way restrictions. In other
words, if you're using the defaults, you will get routes that shouldn't
actually be followed in the real world for many reasons.

This default is designed for one particular use case: drawing potential new
active travel routes along existing roads. The user designing these proposed
routes is expected to understand the properties of the roads selected, and
incorporate appropriate changes in their larger work. The route snapper UI
emphasizes adjusting waypoints easily, letting the user quickly "mold" whatever
they have in mind.

If you'd like to use this library for other purposes (like offline routing for
end-users), you'll need to generate custom graphs from GeoJSON. See the section
above and please file an issue if you have any trouble.
