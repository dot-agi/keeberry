// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * KEYMAP group (0x1x) wire helpers. Every layout mirrors `kcp.rs`'s
 * `keymap_dispatch` byte-for-byte; offsets are payload-relative.
 *
 * - GET_KEYCODE (0x10): request `[layer, row, col]`; reply payload is the
 *   keycode as a little-endian `u16` (`[kc_lo, kc_hi]`).
 * - SET_KEYCODE (0x11): request `[layer, row, col, kc_lo, kc_hi]`.
 * - GET_LAYER_COUNT (0x12): reply payload `[LAYERS]` (a single byte).
 * - GET_LAYER_CONFIG (0x13) / SET_LAYER_CONFIG (0x14): the persistent `DF` default
 *   layer and the tri-layer rule, `[default_layer, tri_enabled, tri_l1, tri_l2, tri_l3]`.
 */
import { readU16LE } from './bytes';

/** Build the GET_KEYCODE request payload `[layer, row, col]`. */
export function encodeGetKeycodeArgs(layer: number, row: number, col: number): number[] {
  return [layer & 0xff, row & 0xff, col & 0xff];
}

/** Build the SET_KEYCODE request payload `[layer, row, col, kc_lo, kc_hi]`. */
export function encodeSetKeycodeArgs(
  layer: number,
  row: number,
  col: number,
  keycode: number,
): number[] {
  return [layer & 0xff, row & 0xff, col & 0xff, keycode & 0xff, (keycode >> 8) & 0xff];
}

/** Decode a GET_KEYCODE reply payload: the keycode as a little-endian `u16`. */
export function parseKeycodeReply(payload: Uint8Array): number {
  return readU16LE(payload, 0);
}

/** Decode a GET_LAYER_COUNT reply payload: the layer count in byte 0. */
export function parseLayerCount(payload: Uint8Array): number {
  return payload[0];
}

/**
 * The layer configuration (`KEYMAP.GET_LAYER_CONFIG` / `SET_LAYER_CONFIG`): the
 * persistent default (base) layer a `DF(n)` key selects, plus the tri-layer rule
 * "`l1` and `l2` active ⇒ `l3` active". Mirrors `keymap.rs`'s state.
 */
export interface LayerConfig {
  /** The persistent default (base) layer the active mask starts from. */
  defaultLayer: number;
  /** Whether the tri-layer rule is active. */
  triEnabled: boolean;
  /** First tri-layer trigger layer (`l1`). */
  triL1: number;
  /** Second tri-layer trigger layer (`l2`). */
  triL2: number;
  /** Tri-layer target layer (`l3`), activated when `l1` and `l2` are both active. */
  triL3: number;
}

/** Parse a GET_LAYER_CONFIG reply `[default_layer, tri_enabled, tri_l1, tri_l2, tri_l3]`. */
export function parseLayerConfig(payload: Uint8Array): LayerConfig {
  return {
    defaultLayer: payload[0],
    triEnabled: payload[1] !== 0,
    triL1: payload[2],
    triL2: payload[3],
    triL3: payload[4],
  };
}

/** Encode the SET_LAYER_CONFIG request payload (the same layout {@link parseLayerConfig} reads). */
export function encodeSetLayerConfigArgs(cfg: LayerConfig): number[] {
  return [
    cfg.defaultLayer & 0xff,
    cfg.triEnabled ? 1 : 0,
    cfg.triL1 & 0xff,
    cfg.triL2 & 0xff,
    cfg.triL3 & 0xff,
  ];
}

/**
 * Matrix positions with no physical key on the 75% layout, as `"row,col"`. From
 * the donor keymap (`keymap.rs`): the grid is the full 6×15 scanner matrix, but
 * these seven positions are holes, so the editor renders them as gaps.
 */
export const MATRIX_HOLES: ReadonlySet<string> = new Set([
  '3,12',
  '4,1',
  '5,3',
  '5,4',
  '5,5',
  '5,7',
  '5,8',
]);

/** True when `(row, col)` is a layout hole (no physical key). */
export function isMatrixHole(row: number, col: number): boolean {
  return MATRIX_HOLES.has(`${row},${col}`);
}
