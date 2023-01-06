# Changes

## Unreleased

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
