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
* @returns {string | undefined}
*/
  toFinalFeature(): string | undefined;
/**
* @returns {string}
*/
  renderGeojson(): string;
/**
* @param {boolean} snap_mode
*/
  setSnapMode(snap_mode: boolean): void;
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
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_jsroutesnapper_free: (a: number) => void;
  readonly jsroutesnapper_new: (a: number, b: number, c: number) => void;
  readonly jsroutesnapper_toFinalFeature: (a: number, b: number) => void;
  readonly jsroutesnapper_renderGeojson: (a: number, b: number) => void;
  readonly jsroutesnapper_setSnapMode: (a: number, b: number) => void;
  readonly jsroutesnapper_onMouseMove: (a: number, b: number, c: number, d: number) => number;
  readonly jsroutesnapper_onClick: (a: number) => void;
  readonly jsroutesnapper_onDragStart: (a: number) => number;
  readonly jsroutesnapper_onMouseUp: (a: number) => number;
  readonly jsroutesnapper_clearState: (a: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_malloc: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number) => void;
  readonly __wbindgen_realloc: (a: number, b: number, c: number) => number;
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
export default function init (module_or_path?: InitInput | Promise<InitInput>): Promise<InitOutput>;
