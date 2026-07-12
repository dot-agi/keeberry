// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  encodeGetKeycodeArgs,
  encodeSetKeycodeArgs,
  encodeSetLayerConfigArgs,
  isMatrixHole,
  parseKeycodeReply,
  parseLayerConfig,
  parseLayerCount,
  type LayerConfig,
} from './keymap';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, REPLY_FLAG, Status } from './protocol';
import {
  createFakeDevice,
  fakeFirmwareHandle,
  packKeycodeReply,
  packLayerCount,
} from './firmware-fixture';

describe('keycode u16 LE round-trip against the fixture', () => {
  // None, Transparent, a basic key, a modifier, MomentaryLayer(1), a consumer key, and the ceiling.
  const codes = [0x0000, 0x0001, 0x0004, 0x00e3, 0x5201, 0xc0cd, 0xffff];

  it('parseKeycodeReply reads back exactly what the firmware packs', () => {
    for (const kc of codes) {
      expect(parseKeycodeReply(packKeycodeReply(kc))).toBe(kc);
    }
  });

  it('encodeSetKeycodeArgs lays out [layer, row, col, kc_lo, kc_hi]', () => {
    expect(encodeSetKeycodeArgs(1, 2, 3, 0xc0cd)).toEqual([1, 2, 3, 0xcd, 0xc0]);
  });

  it('encodeGetKeycodeArgs lays out [layer, row, col]', () => {
    expect(encodeGetKeycodeArgs(1, 4, 9)).toEqual([1, 4, 9]);
  });
});

describe('parseLayerCount', () => {
  it('reads the layer count from byte 0', () => {
    expect(parseLayerCount(packLayerCount(2))).toBe(2);
  });
});

describe('KEYMAP dispatch through the codec (set then get is live)', () => {
  it('a SET_KEYCODE is observed by a following GET_KEYCODE', () => {
    const device = createFakeDevice();
    const kc = 0x5201; // MomentaryLayer(1)

    const setReply = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetKeycode, 0x01, encodeSetKeycodeArgs(1, 2, 3, kc)),
        device,
      ),
    );
    expect(setReply.cmd).toBe(Cmd.SetKeycode | REPLY_FLAG);
    expect(setReply.status).toBe(Status.Ok);

    const getReply = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.GetKeycode, 0x02, encodeGetKeycodeArgs(1, 2, 3)),
        device,
      ),
    );
    expect(getReply.status).toBe(Status.Ok);
    expect(parseKeycodeReply(getReply.payload)).toBe(kc);
  });

  it('a fresh position reads back NO (0)', () => {
    const device = createFakeDevice();
    const reply = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.GetKeycode, 0x00, encodeGetKeycodeArgs(0, 0, 0)),
        device,
      ),
    );
    expect(parseKeycodeReply(reply.payload)).toBe(0x0000);
  });

  it('an out-of-range position replies BadArg for both get and set', () => {
    const device = createFakeDevice();
    const get = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.GetKeycode, 0x00, encodeGetKeycodeArgs(16, 0, 0)),
        device,
      ),
    );
    expect(get.status).toBe(Status.BadArg);

    const set = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetKeycode, 0x00, encodeSetKeycodeArgs(0, 0, 99, 0x04)),
        device,
      ),
    );
    expect(set.status).toBe(Status.BadArg);
  });

  it('reports the layer count over GET_LAYER_COUNT', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetLayerCount, 0x00)));
    expect(reply.status).toBe(Status.Ok);
    expect(parseLayerCount(reply.payload)).toBe(16);
  });
});

describe('layer config (DF default layer + tri-layer) through the codec', () => {
  it('a SET_LAYER_CONFIG is observed by a following GET_LAYER_CONFIG', () => {
    const device = createFakeDevice();
    const cfg: LayerConfig = {
      defaultLayer: 2,
      triEnabled: true,
      triL1: 1,
      triL2: 3,
      triL3: 5,
    };
    const set = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetLayerConfig, 1, encodeSetLayerConfigArgs(cfg)),
        device,
      ),
    );
    expect(set.status).toBe(Status.Ok);

    const got = parseLayerConfig(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetLayerConfig, 2), device)).payload,
    );
    expect(got).toEqual(cfg);
  });

  it('the power-on default is base layer 0 with tri-layer off', () => {
    const got = parseLayerConfig(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetLayerConfig, 0))).payload,
    );
    expect(got).toEqual({ defaultLayer: 0, triEnabled: false, triL1: 0, triL2: 0, triL3: 0 });
  });

  it('rejects an out-of-range default layer or an enabled tri-layer with l1 == l2', () => {
    const device = createFakeDevice();
    const badDefault = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(
          Cmd.SetLayerConfig,
          1,
          encodeSetLayerConfigArgs({
            defaultLayer: 16,
            triEnabled: false,
            triL1: 0,
            triL2: 0,
            triL3: 0,
          }),
        ),
        device,
      ),
    );
    expect(badDefault.status).toBe(Status.BadArg);

    const sameTriggers = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(
          Cmd.SetLayerConfig,
          2,
          encodeSetLayerConfigArgs({
            defaultLayer: 0,
            triEnabled: true,
            triL1: 1,
            triL2: 1,
            triL3: 2,
          }),
        ),
        device,
      ),
    );
    expect(sameTriggers.status).toBe(Status.BadArg);
  });
});

describe('matrix layout', () => {
  it('marks the donor 75% layout holes', () => {
    expect(isMatrixHole(3, 12)).toBe(true);
    expect(isMatrixHole(5, 7)).toBe(true);
    expect(isMatrixHole(0, 0)).toBe(false);
    expect(isMatrixHole(0, 14)).toBe(false); // a real (encoder/wireless) position
  });
});
