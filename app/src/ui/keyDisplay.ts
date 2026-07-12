// SPDX-License-Identifier: GPL-2.0-or-later
import { keycodeLabel, keycodeName } from '../kcp';

const INTERNAL_KEYCODE_PATTERN = /\b0x[0-9a-f]+\b/i;

export function keyLabel(raw: number): string {
  const label = keycodeLabel(raw);
  return INTERNAL_KEYCODE_PATTERN.test(label) ? 'Special' : label;
}

export function keyName(raw: number): string {
  const name = keycodeName(raw);
  return INTERNAL_KEYCODE_PATTERN.test(name) ? 'Special key' : name;
}
