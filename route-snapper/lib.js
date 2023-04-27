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
            "unimportant",
            circleRadiusPixels / 2.0,
            // other
            circleRadiusPixels,
          ],
          "circle-color": [
            "match",
            ["get", "type"],
            "hovered",
            "green",
            "important",
            "red",
            // other
            "black",
          ],
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
          "line-color": "black",
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
        }
      });

      document.addEventListener("keydown", (e) => {
        if (!this.active) {
          return;
        }
        if (e.key == "Shift") {
          e.preventDefault();
          this.inner.setSnapMode(false);
          this.#redraw();
        }
      });
      document.addEventListener("keyup", (e) => {
        if (!this.active) {
          return;
        }
        if (e.key == "Shift") {
          e.preventDefault();
          this.inner.setSnapMode(true);
          this.#redraw();
        }
      });

      this.stop();
    });
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

    // Warning, must do this!
    if (feature.geometry.type == "Polygon") {
      this.inner.setConfig({
        avoid_doubling_back: true,
        area_mode: true,
      });
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

    this.controlDiv.innerHTML = "";
    let btn = document.createElement("button");
    btn.innerText = "Route tool";
    btn.type = "button";
    btn.onclick = () => {
      this.controlDiv.dispatchEvent(new CustomEvent("activate"));
      this.start();
    };
    this.controlDiv.appendChild(btn);
  }

  // Activate the tool.
  start() {
    // If we were already active, don't do anything
    if (this.active) {
      return;
    }

    this.active = true;

    let btn1 = document.createElement("button");
    btn1.type = "button";
    btn1.innerText = "Finish route";
    btn1.onclick = () => {
      this.#finishSnapping();
    };

    let btn2 = document.createElement("button");
    btn2.type = "button";
    btn2.innerText = "Cancel";
    btn2.onclick = () => {
      this.controlDiv.dispatchEvent(new CustomEvent("no-new-route"));
      this.stop();
    };

    this.controlDiv.innerHTML = "";
    let buttons = document.createElement("div");
    buttons.style = "display: flex; justify-content: space-evenly;";
    buttons.appendChild(btn1);
    buttons.appendChild(btn2);
    this.controlDiv.appendChild(buttons);

    let avoidDoublingBack = document.createElement("input");
    avoidDoublingBack.type = "checkbox";
    avoidDoublingBack.id = "avoidDoublingBack";

    let avoidDoublingBackLabel = document.createElement("label");
    avoidDoublingBackLabel.innerText = "Avoid doubling back";
    avoidDoublingBackLabel.for = avoidDoublingBack.id;

    let areaMode = document.createElement("input");
    areaMode.type = "checkbox";
    areaMode.id = "areaMode";

    let areaModelabel = document.createElement("label");
    areaModelabel.innerText = "Area mode";
    areaModelabel.for = areaMode.id;

    let checkboxDiv1 = document.createElement("div");
    checkboxDiv1.appendChild(avoidDoublingBack);
    checkboxDiv1.appendChild(avoidDoublingBackLabel);
    this.controlDiv.append(checkboxDiv1);

    let checkboxDiv2 = document.createElement("div");
    checkboxDiv2.appendChild(areaMode);
    checkboxDiv2.appendChild(areaModelabel);
    this.controlDiv.append(checkboxDiv2);

    avoidDoublingBack.onclick = () => {
      this.inner.setConfig({
        avoid_doubling_back: avoidDoublingBack.checked,
        area_mode: areaMode.checked,
      });
      this.#redraw();
    };

    areaMode.onclick = () => {
      // TODO Force avoidDoublingBack on too in the controls
      this.inner.setConfig({
        avoid_doubling_back: true,
        area_mode: areaMode.checked,
      });
      this.#redraw();
    };

    const instructions = document.createElement("ul");
    instructions.innerHTML =
      `<li><b>Click</b> green points on the transport network</br>to create snapped routes</li>` +
      `<li>Hold <b>Shift</b> to draw a point anywhere</li>` +
      `<li><b>Click and drag</b> any point to move it</li>` +
      `<li><b>Click</b> a red waypoint to delete it</li>` +
      `<li>Press <b>Enter</b> to finish route</li>`;
    `<li>Press <b>Escape</b> to cancel and discard route</li>`;

    this.controlDiv.appendChild(instructions);
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
      this.map
        .getSource("route-snapper")
        .setData(JSON.parse(this.inner.renderGeojson()));
    }
  }
}

export async function fetchWithProgress(url, progressBar) {
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
    progressBar.style = `background: linear-gradient(to right, red ${percent}%, transparent 0);`;
  }

  let allChunks = new Uint8Array(receivedLength);
  let position = 0;
  for (let chunk of chunks) {
    allChunks.set(chunk, position);
    position += chunk.length;
  }

  return allChunks;
}