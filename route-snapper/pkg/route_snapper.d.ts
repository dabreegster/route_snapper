/* tslint:disable */
/* eslint-disable */
/**
*/
export class JsRouteSnapper {
  free(): void;
/**
* @param {Uint8Array} map_bytes
*/
  constructor(map_bytes: Uint8Array);
/**
* Updates configuration and recalculates paths. The caller should redraw.
* @param {any} input
*/
  setRouteConfig(input: any): void;
/**
* Enables area mode, where the snapper produces polygons.
*/
  setAreaMode(): void;
/**
* Gets the current configuration in JSON.
* @returns {string}
*/
  getConfig(): string;
/**
* @returns {string | undefined}
*/
  toFinalFeature(): string | undefined;
/**
* @returns {string}
*/
  renderGeojson(): string;
/**
*/
  toggleSnapMode(): void;
/**
* @param {number} lon
* @param {number} lat
* @param {number} circle_radius_meters
* @returns {boolean}
*/
  onMouseMove(lon: number, lat: number, circle_radius_meters: number): boolean;
/**
*/
  onClick(): void;
/**
* @returns {boolean}
*/
  onDragStart(): boolean;
/**
* @returns {boolean}
*/
  onMouseUp(): boolean;
/**
*/
  clearState(): void;
/**
* @param {any} raw_waypoints
*/
  editExisting(raw_waypoints: any): void;
/**
* Render the graph as GeoJSON points and line-strings, for debugging.
* @returns {string}
*/
  debugRenderGraph(): string;
/**
* Render the graph as GeoJSON points, for helping the user understand the snappable nodes.
* @returns {string}
*/
  debugSnappableNodes(): string;
/**
* @param {any} raw_waypoints
* @returns {string}
*/
  routeNameForWaypoints(raw_waypoints: any): string;
/**
* @param {number} lon
* @param {number} lat
*/
  addSnappedWaypoint(lon: number, lat: number): void;
/**
*/
  undo(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_jsroutesnapper_free: (a: number) => void;
  readonly jsroutesnapper_new: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_setRouteConfig: (a: number, b: number) => void;
  readonly jsroutesnapper_setAreaMode: (a: number) => void;
  readonly jsroutesnapper_getConfig: (a: number, b: number) => void;
  readonly jsroutesnapper_toFinalFeature: (a: number, b: number) => void;
  readonly jsroutesnapper_renderGeojson: (a: number, b: number) => void;
  readonly jsroutesnapper_toggleSnapMode: (a: number) => void;
  readonly jsroutesnapper_onMouseMove: (a: number, b: number, c: number, d: number) => number;
  readonly jsroutesnapper_onClick: (a: number) => void;
  readonly jsroutesnapper_onDragStart: (a: number) => number;
  readonly jsroutesnapper_onMouseUp: (a: number) => number;
  readonly jsroutesnapper_clearState: (a: number) => void;
  readonly jsroutesnapper_editExisting: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_debugRenderGraph: (a: number, b: number) => void;
  readonly jsroutesnapper_debugSnappableNodes: (a: number, b: number) => void;
  readonly jsroutesnapper_routeNameForWaypoints: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_addSnappedWaypoint: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_undo: (a: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_exn_store: (a: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {SyncInitInput} module
*
* @returns {InitOutput}
*/
export function initSync(module: SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {InitInput | Promise<InitInput>} module_or_path
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: InitInput | Promise<InitInput>): Promise<InitOutput>;
