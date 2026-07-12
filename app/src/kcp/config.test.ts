// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  DebounceAlgorithm,
  encodeSetDebounceArgs,
  encodeSetTuningArgs,
  parseDebounce,
  parseStorageInfo,
  parseTuning,
  type DebounceConfig,
  type StorageInfo,
  type TuningConfig,
} from './config';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  CONFIG_REGION_BASE,
  CONFIG_REGION_SIZE,
  DEBOUNCE_EAGER,
  DEFAULT_AUTO_SHIFT_TIMEOUT_MS,
  DEFAULT_DEBOUNCE,
  DEFAULT_DEBOUNCE_INTERVAL,
  DEFAULT_LEADER_TIMEOUT_MS,
  DEFAULT_QUICK_TAP_TERM_MS,
  DEFAULT_STORAGE_INFO,
  DEFAULT_TAP_HOLD_TERM_MS,
  SCHEMA_VERSION,
  createFakeDevice,
  fakeFirmwareHandle,
  packStorageInfo,
  type StorageInfoSample,
} from './firmware-fixture';

describe('parseStorageInfo (mirror of pack_storage_info)', () => {
  it('reads base/size (u32 LE), version (u16 LE) and the valid flag', () => {
    const sample: StorageInfoSample = {
      base: CONFIG_REGION_BASE,
      size: CONFIG_REGION_SIZE,
      version: SCHEMA_VERSION,
      valid: true,
    };
    const expected: StorageInfo = { ...sample };
    expect(parseStorageInfo(packStorageInfo(sample))).toEqual(expected);
  });

  it('reads the firmware region constants (0x0801EE00, 4608 bytes)', () => {
    const info = parseStorageInfo(packStorageInfo());
    expect(info.base).toBe(0x0801_ee00);
    expect(info.size).toBe(4608);
    expect(info).toEqual(DEFAULT_STORAGE_INFO);
  });

  it('decodes the valid flag as a boolean', () => {
    const info = parseStorageInfo(packStorageInfo({ ...DEFAULT_STORAGE_INFO, valid: false }));
    expect(info.valid).toBe(false);
  });
});

describe('parseDebounce / encodeSetDebounceArgs (mirror of CONFIG debounce)', () => {
  it('round-trips an algorithm + interval through the SET args and a GET reply', () => {
    const cfg: DebounceConfig = { algorithm: DebounceAlgorithm.AsymmetricEager, interval: 8 };
    const args = encodeSetDebounceArgs(cfg);
    expect(args).toEqual([DEBOUNCE_EAGER, 8]);
    // A GET reply packs `[algorithm, interval]` at payload[0..2]; parse reads them back.
    expect(parseDebounce(Uint8Array.from([...args, 0, 0]))).toEqual(cfg);
  });
});

describe('CONFIG dispatch through the codec', () => {
  const run = (cmd: number, seq: number, device = createFakeDevice(), payload?: number[]) =>
    decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

  it('GET_STORAGE_INFO reports no valid blob on a fresh device', () => {
    const reply = run(Cmd.ConfigGetStorageInfo, 1);
    expect(reply.status).toBe(Status.Ok);
    const info = parseStorageInfo(reply.payload);
    expect(info.valid).toBe(false);
    expect(info.version).toBe(0);
  });

  it('SAVE persists a valid current-version blob, observed by GET_STORAGE_INFO', () => {
    const device = createFakeDevice();
    const save = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ConfigSave, 1), device));
    expect(save.status).toBe(Status.Ok);

    const info = parseStorageInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ConfigGetStorageInfo, 2), device)).payload,
    );
    expect(info.valid).toBe(true);
    expect(info.version).toBe(SCHEMA_VERSION);
  });

  it('LOAD_DEFAULTS answers Ok without persisting (storage stays invalid)', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ConfigLoadDefaults, 1), device)).status,
    ).toBe(Status.Ok);
    const info = parseStorageInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ConfigGetStorageInfo, 2), device)).payload,
    );
    expect(info.valid).toBe(false);
  });

  it('GET_DEBOUNCE reports the symmetric-defer default on a fresh device', () => {
    const reply = run(Cmd.ConfigGetDebounce, 1);
    expect(reply.status).toBe(Status.Ok);
    expect(parseDebounce(reply.payload)).toEqual({
      algorithm: DebounceAlgorithm.SymmetricDefer,
      interval: DEFAULT_DEBOUNCE_INTERVAL,
    });
    expect(DEFAULT_DEBOUNCE.algorithm).toBe(DebounceAlgorithm.SymmetricDefer);
  });

  it('SET_DEBOUNCE applies live, observed by a following GET_DEBOUNCE', () => {
    const device = createFakeDevice();
    const cfg: DebounceConfig = { algorithm: DebounceAlgorithm.AsymmetricEager, interval: 12 };
    const set = run(Cmd.ConfigSetDebounce, 1, device, encodeSetDebounceArgs(cfg));
    expect(set.status).toBe(Status.Ok);

    const got = run(Cmd.ConfigGetDebounce, 2, device);
    expect(got.status).toBe(Status.Ok);
    expect(parseDebounce(got.payload)).toEqual(cfg);
  });

  it('SET_DEBOUNCE rejects an unknown algorithm with BAD_ARG', () => {
    const reply = run(Cmd.ConfigSetDebounce, 1, createFakeDevice(), [0x7f, 5]);
    expect(reply.status).toBe(Status.BadArg);
  });

  it('SET_DEBOUNCE rejects a zero interval with BAD_ARG', () => {
    const reply = run(Cmd.ConfigSetDebounce, 1, createFakeDevice(), [DEBOUNCE_EAGER, 0]);
    expect(reply.status).toBe(Status.BadArg);
  });

  it('an unknown CONFIG operation answers BAD_CMD', () => {
    expect(decodeReply(fakeFirmwareHandle(encodeRequest(0x4f, 1))).status).toBe(Status.BadCmd);
  });
});

describe('parseTuning / encodeSetTuningArgs (mirror of CONFIG tuning)', () => {
  it('round-trips the auto-shift + leader + tap-hold tunables through SET args and a GET reply', () => {
    const cfg: TuningConfig = {
      autoShiftEnabled: true,
      autoShiftTimeoutMs: 200,
      leaderTimeoutMs: 350,
      tapHoldTermMs: 180,
      permissiveHold: true,
      holdOnOtherKeyPress: false,
      retroTapping: true,
      chordalHold: true,
      quickTapTermMs: 120,
    };
    const args = encodeSetTuningArgs(cfg);
    // [as_on, as_timeout(2), leader(2), th_term(2), th_flags, quick_tap(2)]; flags =
    // permissive(1) | retro(4) | chordal(8) = 13.
    expect(args).toEqual([1, 200, 0, 350 & 0xff, 350 >> 8, 180, 0, 13, 120, 0]);
    expect(parseTuning(Uint8Array.from(args))).toEqual(cfg);
  });
});

describe('CONFIG tuning dispatch through the codec', () => {
  const run = (cmd: number, seq: number, device = createFakeDevice(), payload?: number[]) =>
    decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

  it('GET_TUNING reports the power-on defaults on a fresh device', () => {
    const reply = run(Cmd.ConfigGetTuning, 1);
    expect(reply.status).toBe(Status.Ok);
    expect(parseTuning(reply.payload)).toEqual({
      autoShiftEnabled: false,
      autoShiftTimeoutMs: DEFAULT_AUTO_SHIFT_TIMEOUT_MS,
      leaderTimeoutMs: DEFAULT_LEADER_TIMEOUT_MS,
      tapHoldTermMs: DEFAULT_TAP_HOLD_TERM_MS,
      permissiveHold: false,
      holdOnOtherKeyPress: false,
      retroTapping: false,
      chordalHold: false,
      quickTapTermMs: DEFAULT_QUICK_TAP_TERM_MS,
    });
  });

  it('SET_TUNING applies live, observed by a following GET_TUNING', () => {
    const device = createFakeDevice();
    const cfg: TuningConfig = {
      autoShiftEnabled: true,
      autoShiftTimeoutMs: 150,
      leaderTimeoutMs: 250,
      tapHoldTermMs: 175,
      permissiveHold: false,
      holdOnOtherKeyPress: true,
      retroTapping: false,
      chordalHold: false,
      quickTapTermMs: 0,
    };
    expect(run(Cmd.ConfigSetTuning, 1, device, encodeSetTuningArgs(cfg)).status).toBe(Status.Ok);
    expect(parseTuning(run(Cmd.ConfigGetTuning, 2, device).payload)).toEqual(cfg);
  });

  it('SET_TUNING rejects a zero auto-shift, leader or tap-hold term with BAD_ARG', () => {
    const base: TuningConfig = {
      autoShiftEnabled: true,
      autoShiftTimeoutMs: 175,
      leaderTimeoutMs: 300,
      tapHoldTermMs: 200,
      permissiveHold: false,
      holdOnOtherKeyPress: false,
      retroTapping: false,
      chordalHold: false,
      quickTapTermMs: 200,
    };
    const set = (overrides: Partial<TuningConfig>, seq: number) =>
      run(Cmd.ConfigSetTuning, seq, createFakeDevice(), encodeSetTuningArgs({ ...base, ...overrides }))
        .status;
    expect(set({ autoShiftTimeoutMs: 0 }, 1)).toBe(Status.BadArg);
    expect(set({ leaderTimeoutMs: 0 }, 2)).toBe(Status.BadArg);
    expect(set({ tapHoldTermMs: 0 }, 3)).toBe(Status.BadArg);
    // A zero quick-tap window is valid — it disables quick-tap.
    expect(set({ quickTapTermMs: 0 }, 4)).toBe(Status.Ok);
  });
});
