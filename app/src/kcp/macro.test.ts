// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  encodeMacroRecordStartArgs,
  encodeMacroSetStepArgs,
  parseMacroInfo,
  parseMacroStep,
  type MacroStep,
} from './macro';
import { decodeReply, encodeRequest } from './codec';
import { CLEAR_ALL } from './behavior';
import { Cmd, Status } from './protocol';
import {
  MAX_MACRO,
  MAX_MACRO_STEPS,
  createFakeDevice,
  fakeFirmwareHandle,
} from './firmware-fixture';

describe('parseMacroInfo (mirror of MACRO_INFO)', () => {
  it('reads [maxMacro, maxSteps, used(4 LE)]', () => {
    const payload = new Uint8Array([4, 32, 0b0000_0101, 0, 0, 0]);
    expect(parseMacroInfo(payload)).toEqual({ maxMacro: 4, maxSteps: 32, used: 0b0101 });
  });
});

describe('parseMacroStep (mirror of pack-macro-step)', () => {
  it('reads [present, kc_lo, kc_hi, down, delay_lo, delay_hi, len]', () => {
    // keycode 0x0004 (A), down, 200 ms delay, present, macro length 3.
    const payload = new Uint8Array([1, 0x04, 0x00, 1, 0xc8, 0x00, 3]);
    expect(parseMacroStep(payload)).toEqual({
      present: true,
      len: 3,
      step: { keycode: 0x0004, down: true, delayMs: 200 },
    });
  });
});

describe('MACRO dispatch through the codec', () => {
  it('INFO reports capacities and the used bitmap grows as macros gain steps', () => {
    const device = createFakeDevice();
    const before = parseMacroInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroInfo, 0), device)).payload,
    );
    expect(before).toEqual({ maxMacro: MAX_MACRO, maxSteps: MAX_MACRO_STEPS, used: 0 });

    const ev: MacroStep = { keycode: 0x0004, down: true, delayMs: 10 };
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 1, encodeMacroSetStepArgs(2, 0, ev)),
      device,
    );
    const after = parseMacroInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroInfo, 2), device)).payload,
    );
    expect(after.used).toBe(0b0100); // macro index 2 now has a step
  });

  it('SET_STEP grows the length to cover the step and GET_STEP reads it back', () => {
    const device = createFakeDevice();
    const down: MacroStep = { keycode: 0x0004, down: true, delayMs: 0 };
    const up: MacroStep = { keycode: 0x0004, down: false, delayMs: 25 };
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 1, encodeMacroSetStepArgs(0, 0, down)),
      device,
    );
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 2, encodeMacroSetStepArgs(0, 1, up)),
      device,
    );

    const step0 = parseMacroStep(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroGetStep, 3, [0, 0]), device)).payload,
    );
    expect(step0).toEqual({ present: true, len: 2, step: down });

    // A step beyond the active length reads back as not present.
    const step5 = parseMacroStep(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroGetStep, 4, [0, 5]), device)).payload,
    );
    expect(step5.present).toBe(false);
    expect(step5.len).toBe(2);
  });

  it('CLEAR empties a macro and PLAY rejects an empty or out-of-range macro', () => {
    const device = createFakeDevice();
    const ev: MacroStep = { keycode: 0x0004, down: true, delayMs: 0 };
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 1, encodeMacroSetStepArgs(1, 0, ev)),
      device,
    );

    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroPlay, 2, [1]), device)).status,
    ).toBe(Status.Ok);

    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroClear, 3, [1]), device));
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroPlay, 4, [1]), device)).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroPlay, 5, [MAX_MACRO]), device)).status,
    ).toBe(Status.BadArg);
  });

  it('CLEAR with the 0xFF sentinel empties every macro', () => {
    const device = createFakeDevice();
    const ev: MacroStep = { keycode: 0x0004, down: true, delayMs: 0 };
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 1, encodeMacroSetStepArgs(0, 0, ev)),
      device,
    );
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 2, encodeMacroSetStepArgs(3, 0, ev)),
      device,
    );

    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroClear, 3, [CLEAR_ALL]), device));
    const info = parseMacroInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroInfo, 4), device)).payload,
    );
    expect(info.used).toBe(0);
  });

  it('RECORD_START clears the target slot and arms recording; RECORD_STOP ends it', () => {
    const device = createFakeDevice();
    // Seed a step so the RECORD_START wipe is observable in the used bitmap.
    const ev: MacroStep = { keycode: 0x0004, down: true, delayMs: 0 };
    fakeFirmwareHandle(
      encodeRequest(Cmd.MacroSetStep, 1, encodeMacroSetStepArgs(1, 0, ev)),
      device,
    );
    expect(
      parseMacroInfo(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroInfo, 2), device)).payload,
      ).used,
    ).toBe(0b0010);

    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(Cmd.MacroRecordStart, 3, encodeMacroRecordStartArgs(1)),
          device,
        ),
      ).status,
    ).toBe(Status.Ok);
    // Start clears the slot and records which slot is now capturing.
    expect(
      parseMacroInfo(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroInfo, 4), device)).payload,
      ).used,
    ).toBe(0);
    expect(device.recording).toBe(1);

    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroRecordStop, 5), device)).status,
    ).toBe(Status.Ok);
    expect(device.recording).toBeNull();
  });

  it('RECORD_START rejects an out-of-range macro, and RECORD_STOP is a no-op success when idle', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(Cmd.MacroRecordStart, 1, encodeMacroRecordStartArgs(MAX_MACRO)),
          device,
        ),
      ).status,
    ).toBe(Status.BadArg);
    expect(device.recording).toBeNull();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.MacroRecordStop, 2), device)).status,
    ).toBe(Status.Ok);
  });

  it('rejects an out-of-range macro/step with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(
            Cmd.MacroSetStep,
            1,
            encodeMacroSetStepArgs(MAX_MACRO, 0, {
              keycode: 4,
              down: true,
              delayMs: 0,
            }),
          ),
          device,
        ),
      ).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.MacroGetStep, 2, [0, MAX_MACRO_STEPS]), device),
      ).status,
    ).toBe(Status.BadArg);
  });
});
