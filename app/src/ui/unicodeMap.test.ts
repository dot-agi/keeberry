// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { restoreUnicodeMap } from './unicodeMap';
import { UNICODE_MAP_SLOTS, UnicodeMode, parseUnicodeInfo, type UnicodeInfo } from '../kcp';
import { createFakeDevice, packUnicodeInfo, type FakeDevice } from '../kcp/firmware-fixture';

/** A minimal kcp client backed by the firmware fixture's RAM Unicode state. */
function deviceBackedClient(device: FakeDevice) {
  return {
    async unicodeGet(): Promise<UnicodeInfo> {
      return parseUnicodeInfo(packUnicodeInfo(device.unicode));
    },
    async unicodeSetMap(slot: number, codepoint: number): Promise<void> {
      device.unicode.map[slot] = codepoint;
    },
  };
}

describe('restoreUnicodeMap (connect-time RAM map restore)', () => {
  it('uploads every cached slot to the just-connected device over SET_MAP', async () => {
    const device = createFakeDevice();
    // A freshly-powered device starts with an empty RAM map.
    expect(device.unicode.map.every((cp) => cp === 0)).toBe(true);

    const cached = new Array<number>(UNICODE_MAP_SLOTS).fill(0);
    cached[0] = 0x00e9; // é
    cached[3] = 0x1f600; // 😀

    const info = await restoreUnicodeMap(deviceBackedClient(device), cached);

    expect(info.slots).toBe(UNICODE_MAP_SLOTS);
    expect(info.mode).toBe(UnicodeMode.Linux);
    expect(device.unicode.map[0]).toBe(0x00e9);
    expect(device.unicode.map[3]).toBe(0x1f600);
    expect(device.unicode.map[5]).toBe(0); // untouched slots stay empty
  });

  it('uploads exactly the device-reported slot count', async () => {
    const device = createFakeDevice();
    const cached = new Array<number>(UNICODE_MAP_SLOTS).fill(0x41); // 'A' in every slot
    await restoreUnicodeMap(deviceBackedClient(device), cached);
    expect(device.unicode.map.filter((cp) => cp === 0x41)).toHaveLength(UNICODE_MAP_SLOTS);
  });
});
