// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Little-endian integer reads over a `Uint8Array`. kcp carries every multi-byte
 * field little-endian (`to_le_bytes` in `kcp.rs`), so these are the building
 * blocks for the group parsers.
 */

/** Read an unsigned 16-bit little-endian value at `offset`. */
export function readU16LE(bytes: Uint8Array, offset = 0): number {
  return (bytes[offset] | (bytes[offset + 1] << 8)) & 0xffff;
}

/** Read an unsigned 32-bit little-endian value at `offset` (always positive). */
export function readU32LE(bytes: Uint8Array, offset = 0): number {
  return (
    (bytes[offset] |
      (bytes[offset + 1] << 8) |
      (bytes[offset + 2] << 16) |
      (bytes[offset + 3] << 24)) >>>
    0
  );
}
