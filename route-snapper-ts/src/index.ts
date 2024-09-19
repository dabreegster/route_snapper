import type { Feature, GeoJSON, LineString, Polygon, Position } from "geojson";
import type { Map, MapMouseEvent } from "maplibre-gl";
import init, { JsRouteSnapper } from "route-snapper";
import { splitRoute } from "./split";

export { init, splitRoute };

const snapDistancePixels = 30;

interface Writable<T> {
  set(value: T): void;
}

export interface RouteProps {
  waypoints: Waypoint[];
  length_meters: number;
  route_name: string;
  full_path: Node[];
}

export type Node = { snapped: number } | { free: [number, number] };

export interface AreaProps {
  waypoints: Waypoint[];
}

export interface Waypoint {
  lon: number;
  lat: number;
  snapped: boolean;
}

export class RouteTool {
  map: Map;
  inner: JsRouteSnapper;
  active: boolean;
  eventListenersSuccess: ((
    f: Feature<LineString, RouteProps> | Feature<Polygon, AreaProps>,
  ) => void)[];
  eventListenersUpdated: ((
    f: Feature<LineString, RouteProps> | Feature<Polygon, AreaProps>,
  ) => void)[];
  eventListenersFailure: (() => void)[];

  routeToolGj: Writable<GeoJSON>;
  snapMode: Writable<boolean>;
  undoLength: Writable<number>;

  constructor(
    map: Map,
    graphBytes: Uint8Array,
    routeToolGj: Writable<GeoJSON>,
    snapMode: Writable<boolean>,
    undoLength: Writable<number>,
  ) {
    this.map = map;
    console.time("Deserialize and setup JsRouteSnapper");
    this.inner = new JsRouteSnapper(graphBytes);
    console.timeEnd("Deserialize and setup JsRouteSnapper");
    this.active = false;
    this.eventListenersSuccess = [];
    this.eventListenersUpdated = [];
    this.eventListenersFailure = [];

    this.routeToolGj = routeToolGj;
    this.snapMode = snapMode;
    this.undoLength = undoLength;

    this.map.on("mousemove", this.onMouseMove);
    this.map.on("click", this.onClick);
    this.map.on("dblclick", this.onDoubleClick);
    this.map.on("dragstart", this.onDragStart);
    this.map.on("mouseup", this.onMouseUp);
    document.addEventListener("keydown", this.onKeyDown);
    document.addEventListener("keypress", this.onKeyPress);
  }

  tearDown() {
    this.map.off("mousemove", this.onMouseMove);
    this.map.off("click", this.onClick);
    this.map.off("dblclick", this.onDoubleClick);
    this.map.off("dragstart", this.onDragStart);
    this.map.off("mouseup", this.onMouseUp);
    document.removeEventListener("keydown", this.onKeyDown);
    document.removeEventListener("keypress", this.onKeyPress);
  }

  onMouseMove = (e: MapMouseEvent) => {
    if (!this.active) {
      return;
    }
    const nearbyPoint: [number, number] = [
      e.point.x - snapDistancePixels,
      e.point.y,
    ];
    const circleRadiusMeters = this.map
      .unproject(e.point)
      .distanceTo(this.map.unproject(nearbyPoint));
    if (
      this.inner.onMouseMove(e.lngLat.lng, e.lngLat.lat, circleRadiusMeters)
    ) {
      this.redraw();
      // TODO We'll call this too frequently
      this.dataUpdated();
    }
  };

  onClick = () => {
    if (!this.active) {
      return;
    }
    this.inner.onClick();
    this.redraw();
    this.dataUpdated();
  };

  onDoubleClick = (e: MapMouseEvent) => {
    if (!this.active) {
      return;
    }
    // When we finish, we'll re-enable doubleClickZoom, but we don't want this to zoom in
    e.preventDefault();
    // Double clicks happen as [click, click, dblclick]. The first click adds a
    // point, the second immediately deletes it, and so we simulate a third
    // click to add it again.
    this.inner.onClick();
    this.finish();
  };

  onDragStart = () => {
    if (!this.active) {
      return;
    }
    if (this.inner.onDragStart()) {
      this.map.dragPan.disable();
    }
  };

  onMouseUp = () => {
    if (!this.active) {
      return;
    }
    if (this.inner.onMouseUp()) {
      this.map.dragPan.enable();
    }
  };

  onKeyDown = (e: KeyboardEvent) => {
    if (!this.active) {
      return;
    }
    if (e.key == "Escape") {
      e.stopPropagation();
      this.cancel();
    }
  };

  onKeyPress = (e: KeyboardEvent) => {
    if (!this.active) {
      return;
    }
    // Ignore keypresses if we're not focused on the map
    if ((e.target as HTMLElement).tagName == "INPUT") {
      return;
    }

    if (e.key == "Enter") {
      e.stopPropagation();
      this.finish();
    } else if (e.key == "s" || e.key == "S") {
      e.stopPropagation();
      this.inner.toggleSnapMode();
      this.redraw();
    } else if (e.key == "z" && e.ctrlKey) {
      this.undo();
    }
  };

  // Activate the tool with blank state.
  startRoute() {
    // If we were already active, don't do anything
    // TODO Or... error? Why'd this happen?
    if (this.active) {
      return;
    }

    this.active = true;

    // Otherwise, shift+click breaks
    this.map.boxZoom.disable();
    // Otherwise, double clicking to finish breaks
    this.map.doubleClickZoom.disable();
  }

  // Activate the tool with blank state.
  startArea() {
    // If we were already active, don't do anything
    // TODO Or... error? Why'd this happen?
    if (this.active) {
      return;
    }

    this.inner.setAreaMode();
    this.active = true;
    this.map.boxZoom.disable();
    this.map.doubleClickZoom.disable();
  }

  // Deactivate the tool, clearing all state. No events are fired for eventListenersFailure.
  stop() {
    this.active = false;
    this.inner.clearState();
    this.redraw();
    this.map.boxZoom.enable();
    this.map.doubleClickZoom.enable();
  }

  // This takes a GeoJSON feature previously returned. It must have all
  // properties returned originally. If waypoints are missing (maybe because
  // the route was produced by a different tool, or an older version of this
  // tool), the edited line-string may differ from the input.
  editExistingRoute(feature: Feature<LineString, RouteProps>) {
    if (this.active) {
      window.alert("Bug: editExistingRoute called when tool is already active");
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

    this.startRoute();
    this.inner.editExisting(feature.properties.waypoints);
    this.redraw();
  }

  // This only handles features previously returned by this tool.
  editExistingArea(feature: Feature<Polygon, AreaProps>) {
    if (this.active) {
      window.alert("Bug: editExistingArea called when tool is already active");
    }

    if (!feature.properties.waypoints) {
      window.alert(
        "Bug: editExistingArea called for a polygon not produced by the route-snapper",
      );
    }

    this.startArea();
    this.inner.editExisting(feature.properties.waypoints);
    this.redraw();
  }

  addEventListenerSuccess(
    callback: (
      f: Feature<LineString, RouteProps> | Feature<Polygon, AreaProps>,
    ) => void,
  ) {
    this.eventListenersSuccess.push(callback);
  }
  addEventListenerUpdated(
    callback: (
      f: Feature<LineString, RouteProps> | Feature<Polygon, AreaProps>,
    ) => void,
  ) {
    this.eventListenersUpdated.push(callback);
  }
  addEventListenerFailure(callback: () => void) {
    this.eventListenersFailure.push(callback);
  }
  clearEventListeners() {
    this.eventListenersSuccess = [];
    this.eventListenersUpdated = [];
    this.eventListenersFailure = [];
  }

  isActive(): boolean {
    return this.active;
  }

  // Either a success or failure event will happen, depending on current state
  finish() {
    let rawJSON = this.inner.toFinalFeature();
    if (rawJSON) {
      // Pass copies to each callback
      for (let cb of this.eventListenersSuccess) {
        cb(
          JSON.parse(rawJSON) as
            | Feature<LineString, RouteProps>
            | Feature<Polygon, AreaProps>,
        );
      }
    } else {
      for (let cb of this.eventListenersFailure) {
        cb();
      }
    }
    this.stop();
  }

  // This stops the tool and fires a failure event
  cancel() {
    this.inner.clearState();
    this.finish();
  }

  setRouteConfig(config: {
    avoid_doubling_back: boolean;
    extend_route: boolean;
  }) {
    this.inner.setRouteConfig(config);
    this.redraw();
  }

  addSnappedWaypoint(pt: Position) {
    this.inner.addSnappedWaypoint(pt[0], pt[1]);
    this.redraw();
  }

  undo() {
    this.inner.undo();
    this.redraw();
  }

  toggleSnapMode() {
    this.inner.toggleSnapMode();
    this.redraw();
  }

  private redraw() {
    let gj = JSON.parse(this.inner.renderGeojson());
    this.routeToolGj.set(gj);
    this.map.getCanvas().style.cursor = gj.cursor;
    this.snapMode.set(gj.snap_mode);
    this.undoLength.set(gj.undo_length);
  }

  private dataUpdated() {
    let rawJSON = this.inner.toFinalFeature();
    if (rawJSON) {
      // Pass copies to each callback
      for (let cb of this.eventListenersUpdated) {
        cb(
          JSON.parse(rawJSON) as
            | Feature<LineString, RouteProps>
            | Feature<Polygon, AreaProps>,
        );
      }
    }
  }
}
