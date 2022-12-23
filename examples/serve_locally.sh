#!/bin/bash

wasm-pack build --release --target web ../route-snapper && python3 -m http.server --directory .
