import init, { JsRouteSnapper } from "./route_snapper.js";

export { init };

export class RouteSnapper {
  constructor(map, graphBytes, controlDiv) {
    const circleRadiusPixels = 10;

    this.controlDiv = controlDiv;
    this.map = map;
    console.time("Deserialize and setup JsRouteSnapper");
    this.inner = new JsRouteSnapper(graphBytes);
    console.timeEnd("Deserialize and setup JsRouteSnapper");
    console.log("JsRouteSnapper ready, waiting for idle event");
    this.active = false;

    // on(load) is a bad trigger, because downloading the RouteSnapper input can race. Just wait for the map to be usable.
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
        type: "circle",
        source: "route-snapper",
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
        filter: ["in", "$type", "Point"],
      });
      this.map.addLayer({
        id: "route-lines",
        type: "line",
        source: "route-snapper",
        layout: {
          "line-cap": "round",
          "line-join": "round",
        },
        paint: {
          "line-color": "black",
          "line-width": 2.5,
        },
        filter: ["in", "$type", "LineString"],
      });

      this.map.on("mousemove", (e) => {
        if (!this.active) {
          return;
        }
        const nearbyPoint = { x: e.point.x - circleRadiusPixels, y: e.point.y };
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

      this.#inactiveControl();
    });
  }

  isActive() {
    return this.active;
  }

  #inactiveControl() {
    this.active = false;

    this.inner.clearState();
    this.#redraw();

    this.controlDiv.innerHTML = "";
    var btn = document.createElement("button");
    btn.innerText = "Route tool";
    btn.type = "button";
    btn.onclick = () => {
      this.#activeControl();
    };
    this.controlDiv.appendChild(btn);
  }

  #activeControl() {
    this.active = true;
    this.controlDiv.dispatchEvent(new CustomEvent("activate"));

    this.controlDiv.innerHTML = "";
    var btn = document.createElement("button");
    btn.type = "button";
    btn.innerText = "Finish route";
    btn.onclick = () => {
      this.#finishSnapping();
    };
    this.controlDiv.appendChild(btn);

    const instructions = document.createElement("ul");
    instructions.innerHTML =
      `<li><b>Click</b> green points on the transport network</br>to create snapped routes</li>` +
      `<li>Hold <b>Shift</b> to draw a point anywhere</li>` +
      `<li><b>Click and drag</b> any point to move it</li>` +
      `<li><b>Click</b> a red waypoint to delete it</li>` +
      `<li>Press <b>Enter</b> to finish route</li>`;

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
    this.#inactiveControl();
  }

  #redraw() {
    this.map
      .getSource("route-snapper")
      .setData(JSON.parse(this.inner.renderGeojson()));
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
