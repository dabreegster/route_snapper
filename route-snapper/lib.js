import init, { JsRouteSnapper } from "./route_snapper.js";

export { init };

export class RouteSnapper {
  constructor(map, graphBytes, controlDiv) {
    const circleRadiusPixels = 10;
    const snapDistancePixels = 30;

    this.controlDiv = controlDiv;
    this.map = map;
    console.time("Deserialize and setup JsRouteSnapper");
    this.inner = new JsRouteSnapper(graphBytes);
    console.timeEnd("Deserialize and setup JsRouteSnapper");
    console.log("JsRouteSnapper ready, waiting for idle event");
    this.active = false;
    // Indicates the idle event has been received, and the source/layers are set up
    this.loaded = false;

    // on(load) is a bad trigger, because downloading the RouteSnapper input
    // can race. Just wait for the map to be usable.
    this.map.once("idle", () => {
      console.log("JsRouteSnapper now usable");
      this.map.addSource("route-snapper", {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: [],
        },
      });
      this.map.addLayer({
        id: "route-points",
        source: "route-snapper",
        filter: ["in", "$type", "Point"],
        type: "circle",
        paint: {
          "circle-radius": [
            "match",
            ["get", "type"],
            "node",
            circleRadiusPixels / 2.0,
            // other
            circleRadiusPixels,
          ],
          "circle-color": [
            "match",
            ["get", "type"],
            "snapped-waypoint",
            "red",
            "free-waypoint",
            "blue",
            // other (node)
            "black",
          ],
          "circle-opacity": ["case", ["has", "hovered"], 0.5, 1.0],
        },
      });
      this.map.addLayer({
        id: "route-lines",
        source: "route-snapper",
        filter: ["in", "$type", "LineString"],
        type: "line",
        layout: {
          "line-cap": "round",
          "line-join": "round",
        },
        paint: {
          "line-color": ["case", ["get", "snapped"], "red", "blue"],
          "line-width": 2.5,
        },
      });
      this.map.addLayer({
        id: "route-polygons",
        source: "route-snapper",
        filter: ["in", "$type", "Polygon"],
        type: "fill",
        paint: {
          "fill-color": "black",
          "fill-opacity": 0.4,
        },
      });
      this.loaded = true;

      this.map.on("mousemove", (e) => {
        if (!this.active) {
          return;
        }
        const nearbyPoint = { x: e.point.x - snapDistancePixels, y: e.point.y };
        const circleRadiusMeters = this.map
          .unproject(e.point)
          .distanceTo(this.map.unproject(nearbyPoint));
        if (
          this.inner.onMouseMove(e.lngLat.lng, e.lngLat.lat, circleRadiusMeters)
        ) {
          this.#redraw();
        }
      });

      this.map.on("click", () => {
        if (!this.active) {
          return;
        }
        this.inner.onClick();
        this.#redraw();
      });

      this.map.on("dblclick", () => {
        if (!this.active) {
          return;
        }
        // Treat it like a click, to possibly add a final point
        this.inner.onClick();
        // But then finish
        this.#finishSnapping();
      });

      this.map.on("dragstart", (e) => {
        if (!this.active) {
          return;
        }
        if (this.inner.onDragStart()) {
          this.map.dragPan.disable();
        }
      });

      this.map.on("mouseup", (e) => {
        if (!this.active) {
          return;
        }
        if (this.inner.onMouseUp()) {
          this.map.dragPan.enable();
        }
      });

      document.addEventListener("keypress", (e) => {
        if (!this.active) {
          return;
        }
        if (e.key == "Enter") {
          e.preventDefault();
          this.#finishSnapping();
        } else if (e.key == "s") {
          e.preventDefault();
          this.inner.toggleSnapMode();
          this.#redraw();
        }
      });

      this.stop();
    });
  }

  // Change the underlying graph after initially creating RouteSnapper.
  changeGraph(graphBytes) {
    console.time("Deserialize and setup JsRouteSnapper with new graph");
    this.inner = new JsRouteSnapper(graphBytes);
    console.timeEnd("Deserialize and setup JsRouteSnapper with new graph");
  }

  isActive() {
    return this.active;
  }

  // Destroy resources attached to the map. Warning, this doesn't yet handle
  // event listeners!
  tearDown() {
    if (!this.loaded) {
      // TODO Can we cancel the map.on(idle) event?
      return;
    }
    this.map.removeLayer("route-points");
    this.map.removeLayer("route-lines");
    this.map.removeSource("route-snapper");
    // TODO Remove the event listeners on document and map
  }

  // This takes a GeoJSON feature previously returned from the new-route event.
  // It must have all properties returned originally. If waypoints are missing
  // (maybe because the route was produced by a different tool, or an older
  // version of this tool), the edited line-string may differ from the input.
  //
  // Note no events are fired by calling this.
  editExisting(feature) {
    if (!this.loaded) {
      // TODO This is an unlikely race condition. What should we do?
      console.error(
        "editExisting called before the map idle event received. Not starting tool."
      );
      return;
    }

    if (!feature.properties.waypoints) {
      // Only use the first and last points as waypoints, and assume they're
      // snapped. This only works for the simplest cases.
      feature.properties.waypoints = [
        {
          lon: feature.geometry.coordinates[0][0],
          lat: feature.geometry.coordinates[0][1],
          snapped: true,
        },
        {
          lon: feature.geometry.coordinates[
            feature.geometry.coordinates.length - 1
          ][0],
          lat: feature.geometry.coordinates[
            feature.geometry.coordinates.length - 1
          ][1],
          snapped: true,
        },
      ];
    }

    this.start();

    if (feature.geometry.type == "Polygon") {
      this.inner.setAreaMode();
    }

    this.inner.editExisting(feature.properties.waypoints);
    this.#redraw();
  }

  // Deactivate the tool, clearing all state. No events (`no-new-route`) are fired.
  stop() {
    if (!this.loaded) {
      return;
    }

    this.active = false;

    this.inner.clearState();
    this.#redraw();

    this.controlDiv.innerHTML = `<button type="button" id="start-button">Route tool</button>`;
    document.getElementById("start-button").onclick = () => {
      this.controlDiv.dispatchEvent(new CustomEvent("activate"));
      this.start();
    };
  }

  // Activate the tool.
  start() {
    // If we were already active, don't do anything
    if (this.active) {
      return;
    }

    this.active = true;

    this.controlDiv.innerHTML = `
    <div style="display: flex; justify-content: space-evenly;">
      <button type="button" id="finish-route-button">Finish route</button>
      <button type="button" id="cancel-button">Cancel</button>
    </div>

    <div>
      <label>
        <input type="checkbox" id="avoidDoublingBack" />
        Avoid doubling back
      </label>
    </div>
    <div>
      <label>
        <input type="checkbox" id="extendRoute" />
        Extend the route
      </label>
    </div>
    <div>
      <label>
        <input type="checkbox" id="areaMode" />
        Area mode
      </label>
    </div>

    <div id="snap_mode" style="background: red; color: white; padding: 8px">
      Snapping to transport network
    </div>

    <ul>
      <li><b>Click</b> green points on the transport network</br>to create snapped routes</li>
      <li>Press <b>s</b> to toggle snapping / freehand mode</li>
      <li><b>Click and drag</b> any point to move it</li>
      <li><b>Click</b> a red waypoint to delete it</li>
      <li>Press <b>Enter</b> or <b>double click</b> to finish route</li>
      <li>Press <b>Escape</b> to cancel and discard route</li>
    </ul>
    `;

    document.getElementById("finish-route-button").onclick = () => {
      this.#finishSnapping();
    };
    document.getElementById("cancel-button").onclick = () => {
      this.controlDiv.dispatchEvent(new CustomEvent("no-new-route"));
      this.stop();
    };
    let avoidDoublingBack = document.getElementById("avoidDoublingBack");
    let areaMode = document.getElementById("areaMode");
    let extendRoute = document.getElementById("extendRoute");
    avoidDoublingBack.onclick = () => {
      this.inner.setRouteConfig({
        avoid_doubling_back: avoidDoublingBack.checked,
        extend_route: extendRoute.checked,
      });
      this.#redraw();
    };
    extendRoute.onclick = avoidDoublingBack.onclick;
    areaMode.onclick = () => {
      if (areaMode.checked) {
        avoidDoublingBack.checked = true;
        extendRoute.checked = true;
        this.inner.setAreaMode();
      } else {
        this.inner.setRouteConfig({
          avoid_doubling_back: avoidDoublingBack.checked,
          extend_route: extendRoute.checked,
        });
      }
      this.#redraw();
    };

    // Sync checkboxes with the tool's current state, from the last time it was used
    let config = JSON.parse(this.inner.getConfig());
    avoidDoublingBack.checked = config.avoid_doubling_back;
    extendRoute.checked = config.extend_route;
    areaMode.checked = config.area_mode;
  }

  // Render the graph as GeoJSON points and line-strings, for debugging.
  debugRenderGraph() {
    return this.inner.debugRenderGraph();
  }

  // Given waypoint properties, calculate the route name.
  routeNameForWaypoints(waypoints) {
    return this.inner.routeNameForWaypoints(waypoints);
  }

  #finishSnapping() {
    // Update the source-of-truth in drawControls
    const rawJSON = this.inner.toFinalFeature();
    if (rawJSON) {
      this.controlDiv.dispatchEvent(
        new CustomEvent("new-route", { detail: JSON.parse(rawJSON) })
      );
    } else {
      this.controlDiv.dispatchEvent(new CustomEvent("no-new-route"));
    }
    this.stop();
  }

  #redraw() {
    if (this.loaded) {
      let gj = JSON.parse(this.inner.renderGeojson());
      this.map.getSource("route-snapper").setData(gj);
      this.map.getCanvas().style.cursor = gj.cursor;

      // TODO Detect changes, don't do this constantly?
      let snapDiv = document.getElementById("snap_mode");
      if (!snapDiv) {
        return;
      }
      if (gj.snap_mode) {
        snapDiv.style = "background: red; color: white; padding: 8px";
        snapDiv.innerHtml = "Snapping to transport network";
      } else {
        snapDiv.style = "background: blue; color: white; padding: 8px";
        snapDiv.innerHtml = "Drawing freehand points";
      }
    }
  }
}

export async function fetchWithProgress(url, setProgress = () => {}) {
  const response = await fetch(url);
  const reader = response.body.getReader();

  const contentLength = response.headers.get("Content-Length");

  let receivedLength = 0;
  let chunks = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }

    chunks.push(value);
    receivedLength += value.length;

    const percent = (100.0 * receivedLength) / contentLength;
    setProgress(percent);
  }

  let allChunks = new Uint8Array(receivedLength);
  let position = 0;
  for (let chunk of chunks) {
    allChunks.set(chunk, position);
    position += chunk.length;
  }

  return allChunks;
}
