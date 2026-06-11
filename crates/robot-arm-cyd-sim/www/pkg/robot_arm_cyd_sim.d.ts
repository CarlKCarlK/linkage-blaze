/* tslint:disable */
/* eslint-disable */

export class CydSim {
    free(): void;
    [Symbol.dispose](): void;
    height(): number;
    is_reverse_kinematics_running(): boolean;
    constructor();
    reverse_kinematics(): number;
    rgba(): Uint8Array;
    set_frame_dt_seconds(dt_seconds: number): void;
    start_reverse_kinematics(): void;
    stop_reverse_kinematics(): void;
    tick_reverse_kinematics(dt_seconds: number): boolean;
    touch_down(x: number, y: number): void;
    touch_move(x: number, y: number): void;
    touch_up(): void;
    width(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_cydsim_free: (a: number, b: number) => void;
    readonly cydsim_height: (a: number) => number;
    readonly cydsim_is_reverse_kinematics_running: (a: number) => number;
    readonly cydsim_new: () => number;
    readonly cydsim_reverse_kinematics: (a: number) => number;
    readonly cydsim_rgba: (a: number) => [number, number];
    readonly cydsim_set_frame_dt_seconds: (a: number, b: number) => void;
    readonly cydsim_start_reverse_kinematics: (a: number) => void;
    readonly cydsim_stop_reverse_kinematics: (a: number) => void;
    readonly cydsim_tick_reverse_kinematics: (a: number, b: number) => number;
    readonly cydsim_touch_down: (a: number, b: number, c: number) => void;
    readonly cydsim_touch_move: (a: number, b: number, c: number) => void;
    readonly cydsim_touch_up: (a: number) => void;
    readonly cydsim_width: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
