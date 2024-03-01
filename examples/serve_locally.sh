#!/bin/bash

set -e
wasm-pack build --release --target web ../route-snapper
wasm-pack build --release --target web ../osm-to-route-snapper
wasm-pack build --release --target web ../geojson-to-route-snapper
cp ../route-snapper/lib.js ../route-snapper/pkg
python3 -m http.server --directory .
