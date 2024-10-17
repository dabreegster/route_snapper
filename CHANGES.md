# Changes

This package is changing quickly due to evolving requirements from its main
user. Before `1.0`, expect the API to bounce all over the place. Please open a
Github issue if you're actively using this and want to give feedback on API
changes.

## Unreleased

## 0.4.4

- Add two new experimental stateless APIs. Do not use yet.

## 0.4.3

- Upgrade Rust geo dependencies

## 0.4.2

- The OSM importer can now handle MultiPolygon boundaries for clipping
- The final route feature has a new `full_path` property, with every snapped and freehand node

## 0.4.1

- Removed `fetchWithProgress`, which is a general-purpose utility method that
  should come from another library
- Improved the tool that makes graphs from GeoJSON files by splitting
  LineStrings that touch at interior points
- Added `debugSnappableNodes` as a faster alternative to `debugRenderGraph`

## 0.4.0

- More details in `debugRenderGraph`
- Add a new tool to make graphs from a GeoJSON file
- Allow edges to have custom costs in each direction. This is a breaking change
  to the binary graph format!

## 0.3.0

- Internally store WGS84 coordinates instead of Mercator (Breaking change to
  the binary graph format!)

## 0.2.5

- Undo support

## 0.2.4

- Include road labels for waypoints in interactive output
- Add an `addSnappedWaypoint` API for use with a geocoder

## 0.2.3

- Always start in snap mode for a new route, even if the user last used freehand mode.
- Color lines to show freehand/snapped as well

## 0.2.2

- Change the `renderGeojson` output to distinguish snapped and freehand waypoints
- Include a `cursor` property in the `renderGeojson` output
- Distinguish hovered points in the `renderGeojson` output
- Use a keypress to toggle snap/freehand mode, instead of holding down a key
- Convert existing nodes between snapped/freehand

## 0.2.1

- Added a `routeNameForWaypoints` API

## 0.2.0

- Include road names in the graph, and auto-populate a route name. (Breaking
  change to the binary graph format!)

## 0.1.15

- Output GeoJSON precision is now trimmed to 6 decimal places
- `fetchWithProgress` now takes a callback to return the progress as a percentage to the user.
- Add `debugRenderGraph` and `changeGraph` APIs
- Split `setConfig` into `setRouteConfig` and `setAreaMode` to prevent incorrect configuration

## 0.1.14

- Fix bugs where built-in controls can get out-of-sync with current settings
- By default, don't keep drawing more points to the end of the route
- Fix bug that created a new waypoint when clicking (and not dragging) an
  intermediate point
- Double click to end a route

## 0.1.13

- Add an optional mode to draw closed areas

## 0.1.12

- Adjust when the `activate` event is fired

## 0.1.11

- Add a `start` method, so the tool can be programmatically controlled
- Add a button to cancel from drawing or editing a route

## 0.1.10

- Backfill missing `waypoints` properties if possible when calling
  `editExisting`. If you input routes drawn before 0.1.8, this will help, but
  may imperfectly restore the previous route.
- Fix some race conditions at startup

## 0.1.9

- Add a `stop` method, so the tool can be programmatically controlled

## 0.1.8

- Add an `editExisting` method to modify previously drawn routes
- The output now includes a `waypoints` property, used for `editExisting`

## 0.1.7

- Add a `setConfig` method
- Add optional behavior to avoid a route doubling back on itself
- The output now includes a `length_meters` property

## 0.1.6

- Added README to NPM package

## 0.1.5

- If the graph is disconnected, draw straight lines between points instead of breaking

## 0.1.4

- Fix missing lines when switching between freehand and snapped points
- Fix bugs snapping to wrong point when many are clustered together
- Always snap to the nearest point, no matter how far it is

## 0.1.3

- Improved styling for draggable points
- Start a `tearDown` method

## 0.1.2

- fetchWithProgress takes an element, not ID, to work better with Svelte
- Added `isActive()` method and new events `activate` and `no-new-route`

## 0.1.1

- No more top-level await required; callers must call `await init` themselves

## 0.1.0

- Initial NPM package
