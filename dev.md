## Publishing a new version

To release a new version of <https://www.npmjs.com/package/route-snapper>:

1.  Bump the version number in `route-snapper/Cargo.toml`
2.  Make sure `router-snapper/pkg/` has the release build with `--target web` and that `lib.js` is in there. If you `cd examples; ./serve_locally.sh`, then this'll happen
3.  **Important**! Manually edit `route-snapper/pkg/package.json` and add `lib.js` to `files`. I can't figure out how to make `wasm-pack` do this.
4.  `cd route-snapper/pkg; npm pack` to sanity check the contents. Then `npm publish`
5.  Update the changelog
