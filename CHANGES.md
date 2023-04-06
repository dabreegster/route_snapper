# Changes

## Unreleased

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
