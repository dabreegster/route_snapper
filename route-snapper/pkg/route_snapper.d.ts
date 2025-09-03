/* tslint:disable */
/* eslint-disable */
export class JsRouteSnapper {
  free(): void;
  constructor(map_bytes: Uint8Array);
  /**
   * Updates configuration and recalculates paths. The caller should redraw.
   */
  setRouteConfig(input: any): void;
  /**
   * Enables area mode, where the snapper produces polygons.
   */
  setAreaMode(): void;
  /**
   * Gets the current configuration in JSON.
   */
  getConfig(): string;
  toFinalFeature(): string | undefined;
  renderGeojson(): string;
  toggleSnapMode(): void;
  onMouseMove(lon: number, lat: number, circle_radius_meters: number): boolean;
  onClick(): void;
  onDragStart(): boolean;
  onMouseUp(): boolean;
  /**
   * Note this doesn't change route/area mode.
   */
  clearState(): void;
  editExisting(raw_waypoints: any): void;
  /**
   * Render the graph as GeoJSON points and line-strings, for debugging.
   */
  debugRenderGraph(): string;
  /**
   * Render the graph as GeoJSON points, for helping the user understand the snappable nodes.
   */
  debugSnappableNodes(): string;
  routeNameForWaypoints(raw_waypoints: any): string;
  addSnappedWaypoint(lon: number, lat: number): void;
  undo(): void;
  /**
   * Experimental new stateless API. From a list of waypoints, return a Feature with the full
   * geometry and properties. Note this internally modifies state.
   */
  calculateRoute(raw_waypoints: any): string;
  /**
   * Experimental new stateless API. From exactly two waypoints, return a list of extra
   * intermediate nodes and a boolean to indicate if they're snappable or not. Note this
   * internally modifies state.
   */
  getExtraNodes(raw_waypt1: any, raw_waypt2: any): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_jsroutesnapper_free: (a: number, b: number) => void;
  readonly jsroutesnapper_new: (a: number, b: number) => [number, number, number];
  readonly jsroutesnapper_setRouteConfig: (a: number, b: any) => void;
  readonly jsroutesnapper_setAreaMode: (a: number) => void;
  readonly jsroutesnapper_getConfig: (a: number) => [number, number];
  readonly jsroutesnapper_toFinalFeature: (a: number) => [number, number];
  readonly jsroutesnapper_renderGeojson: (a: number) => [number, number];
  readonly jsroutesnapper_toggleSnapMode: (a: number) => void;
  readonly jsroutesnapper_onMouseMove: (a: number, b: number, c: number, d: number) => number;
  readonly jsroutesnapper_onClick: (a: number) => void;
  readonly jsroutesnapper_onDragStart: (a: number) => number;
  readonly jsroutesnapper_onMouseUp: (a: number) => number;
  readonly jsroutesnapper_clearState: (a: number) => void;
  readonly jsroutesnapper_editExisting: (a: number, b: any) => [number, number];
  readonly jsroutesnapper_debugRenderGraph: (a: number) => [number, number];
  readonly jsroutesnapper_debugSnappableNodes: (a: number) => [number, number];
  readonly jsroutesnapper_routeNameForWaypoints: (a: number, b: any) => [number, number, number, number];
  readonly jsroutesnapper_addSnappedWaypoint: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_undo: (a: number) => void;
  readonly jsroutesnapper_calculateRoute: (a: number, b: any) => [number, number, number, number];
  readonly jsroutesnapper_getExtraNodes: (a: number, b: any, c: any) => [number, number, number, number];
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_4: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
