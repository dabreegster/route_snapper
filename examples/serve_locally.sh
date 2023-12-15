#!/bin/bash

set -e
wasm-pack build --release --target web ../route-snapper
wasm-pack build --release --target web ../osm-to-route-snapper-v2
cp ../route-snapper/lib.js ../route-snapper/pkg
python3 -m http.server --directory .
