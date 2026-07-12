// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  CLEAR_ALL,
  MIN_COMBO_KEYS,
  SocdMode,
  encodeComboSetArgs,
  encodeLeaderSetArgs,
  encodeOverrideSetArgs,
  encodeSocdSetArgs,
  encodeTapdanceSetArgs,
  formatModifiers,
  parseBehaviorInfo,
  parseCombo,
  parseLeader,
  parseOverride,
  parseSocdPair,
  parseTapdance,
  parseTimedInfo,
  type Combo,
  type KeyOverride,
  type Leader,
  type SocdPair,
  type TapDance,
} from './behavior';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  MAX_COMBO,
  MAX_COMBO_KEYS,
  MAX_LEADER,
  MAX_LEADER_SEQ,
  MAX_MACRO,
  MAX_MACRO_STEPS,
  MAX_OVERRIDES,
  MAX_SOCD,
  MAX_TAP_DANCE,
  createFakeDevice,
  fakeFirmwareHandle,
  packBehaviorInfo,
  packOverride,
  packSocdPair,
  type KeyOverrideSample,
  type SocdPairSample,
} from './firmware-fixture';

describe('parseSocdPair (mirror of pack_socd_pair)', () => {
  it('reads [present, a_lo, a_hi, b_lo, b_hi, mode] with keycodes u16 LE', () => {
    const sample: SocdPairSample = { a: 0x001a, b: 0x0016, mode: SocdMode.Neutral };
    const expected: SocdPair = { ...sample };
    expect(parseSocdPair(packSocdPair(sample))).toEqual(expected);
  });

  it('returns null for an empty slot (present = 0)', () => {
    expect(parseSocdPair(packSocdPair(null))).toBeNull();
  });
});

describe('parseOverride (mirror of pack_override)', () => {
  it('reads trigger/replacement (u16 LE), their mod bytes, layer mask and enabled', () => {
    const sample: KeyOverrideSample = {
      trigger: 0x0004, // A
      triggerMods: 0b0000_0010, // LShift
      replacement: 0x0005, // B
      replacementMods: 0b0000_0001, // LCtrl
      layerMask: 0x0003,
      enabled: true,
    };
    const expected: KeyOverride = { ...sample };
    expect(parseOverride(packOverride(sample))).toEqual(expected);
  });

  it('returns null for an empty slot', () => {
    expect(parseOverride(packOverride(null))).toBeNull();
  });
});

describe('parseBehaviorInfo', () => {
  it('reads [MAX_SOCD, MAX_OVERRIDES] = [8, 16]', () => {
    expect(parseBehaviorInfo(packBehaviorInfo())).toEqual({ maxSocd: 8, maxOverrides: 16 });
  });
});

describe('formatModifiers', () => {
  it('formats the HID modifier byte', () => {
    expect(formatModifiers(0)).toBe('none');
    expect(formatModifiers(0b0000_0101)).toBe('LCtrl+LAlt');
  });
});

describe('SOCD dispatch through the codec (set/get/clear is live)', () => {
  it('a SOCD_SET is observed by a following SOCD_GET', () => {
    const device = createFakeDevice();
    const pair: SocdPair = { a: 0x001a, b: 0x0016, mode: SocdMode.LastWins };
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.SocdSet, 1, encodeSocdSetArgs(2, pair)), device),
    );
    expect(set.status).toBe(Status.Ok);

    const got = parseSocdPair(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdGet, 2, [2]), device)).payload,
    );
    expect(got).toEqual(pair);
  });

  it('SOCD_CLEAR empties one slot; the 0xFF sentinel clears the whole table', () => {
    const device = createFakeDevice();
    fakeFirmwareHandle(
      encodeRequest(Cmd.SocdSet, 1, encodeSocdSetArgs(0, { a: 4, b: 5, mode: 0 })),
      device,
    );
    fakeFirmwareHandle(
      encodeRequest(Cmd.SocdSet, 2, encodeSocdSetArgs(1, { a: 6, b: 7, mode: 1 })),
      device,
    );

    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdClear, 3, [0]), device));
    const slot0 = parseSocdPair(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdGet, 4, [0]), device)).payload,
    );
    const slot1 = parseSocdPair(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdGet, 5, [1]), device)).payload,
    );
    expect(slot0).toBeNull();
    expect(slot1).not.toBeNull();

    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdClear, 6, [CLEAR_ALL]), device));
    const after = parseSocdPair(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdGet, 7, [1]), device)).payload,
    );
    expect(after).toBeNull();
  });

  it('rejects an out-of-range index and an unassigned mode with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SocdGet, 1, [MAX_SOCD]), device)).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(Cmd.SocdSet, 2, encodeSocdSetArgs(0, { a: 4, b: 5, mode: 3 })),
          device,
        ),
      ).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(Cmd.SocdSet, 3, encodeSocdSetArgs(MAX_SOCD, { a: 4, b: 5, mode: 0 })),
          device,
        ),
      ).status,
    ).toBe(Status.BadArg);
  });
});

describe('Override dispatch through the codec', () => {
  it('an OVERRIDE_SET is observed by a following OVERRIDE_GET', () => {
    const device = createFakeDevice();
    const ov: KeyOverride = {
      trigger: 0x0004, // A
      triggerMods: 0b0000_0010, // LShift
      replacement: 0x0029, // Esc
      replacementMods: 0,
      layerMask: 0x0001,
      enabled: true,
    };
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.OverrideSet, 1, encodeOverrideSetArgs(3, ov)), device),
    );
    expect(set.status).toBe(Status.Ok);

    const got = parseOverride(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.OverrideGet, 2, [3]), device)).payload,
    );
    expect(got).toEqual(ov);
  });

  it('a fresh slot reads back empty, and out-of-range indexes are BadArg', () => {
    const device = createFakeDevice();
    expect(
      parseOverride(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.OverrideGet, 1, [0]), device)).payload,
      ),
    ).toBeNull();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.OverrideGet, 2, [MAX_OVERRIDES]), device))
        .status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(
            Cmd.OverrideSet,
            3,
            encodeOverrideSetArgs(MAX_OVERRIDES, {
              trigger: 4,
              triggerMods: 0,
              replacement: 5,
              replacementMods: 0,
              layerMask: 1,
              enabled: true,
            }),
          ),
          device,
        ),
      ).status,
    ).toBe(Status.BadArg);
  });

  it('reports the table capacities over BEHAVIOR_INFO', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.BehaviorInfo, 0)));
    expect(reply.status).toBe(Status.Ok);
    expect(parseBehaviorInfo(reply.payload)).toEqual({
      maxSocd: MAX_SOCD,
      maxOverrides: MAX_OVERRIDES,
    });
  });
});

describe('Tap-dance dispatch through the codec', () => {
  it('a TAPDANCE_SET is observed by a following TAPDANCE_GET', () => {
    const device = createFakeDevice();
    const td: TapDance = { tap: 0x0004, hold: 0x00e0, double: 0x0005, termMs: 180 };
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.TapdanceSet, 1, encodeTapdanceSetArgs(4, td)), device),
    );
    expect(set.status).toBe(Status.Ok);

    const got = parseTapdance(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceGet, 2, [4]), device)).payload,
    );
    expect(got).toEqual(td);
  });

  it('a fresh slot reads back empty; out-of-range indexes are BadArg', () => {
    const device = createFakeDevice();
    expect(
      parseTapdance(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceGet, 1, [0]), device)).payload,
      ),
    ).toBeNull();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceGet, 2, [MAX_TAP_DANCE]), device))
        .status,
    ).toBe(Status.BadArg);
  });

  it('TAPDANCE_CLEAR empties one slot; 0xFF clears the table', () => {
    const device = createFakeDevice();
    const td: TapDance = { tap: 4, hold: 5, double: 0, termMs: 200 };
    fakeFirmwareHandle(encodeRequest(Cmd.TapdanceSet, 1, encodeTapdanceSetArgs(0, td)), device);
    fakeFirmwareHandle(encodeRequest(Cmd.TapdanceSet, 2, encodeTapdanceSetArgs(1, td)), device);
    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceClear, 3, [CLEAR_ALL]), device));
    expect(
      parseTapdance(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceGet, 4, [0]), device)).payload,
      ),
    ).toBeNull();
    expect(
      parseTapdance(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TapdanceGet, 5, [1]), device)).payload,
      ),
    ).toBeNull();
  });
});

describe('Combo dispatch through the codec', () => {
  it('a COMBO_SET is observed by a following COMBO_GET (keys trimmed to len, flags round-trip)', () => {
    const device = createFakeDevice();
    const combo: Combo = {
      keys: [0x0004, 0x0005, 0x0006],
      action: 0x0029,
      termMs: 40,
      mustHold: true,
      mustTap: false,
      inOrder: true,
    };
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.ComboSet, 1, encodeComboSetArgs(2, combo)), device),
    );
    expect(set.status).toBe(Status.Ok);

    const got = parseCombo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ComboGet, 2, [2]), device)).payload,
    );
    expect(got).toEqual(combo);
  });

  it('rejects a key count below MIN_COMBO_KEYS or above MAX_COMBO_KEYS', () => {
    const device = createFakeDevice();
    const tooFew: Combo = {
      keys: [0x0004],
      action: 0x0005,
      termMs: 50,
      mustHold: false,
      mustTap: false,
      inOrder: false,
    };
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.ComboSet, 1, encodeComboSetArgs(0, tooFew)), device),
      ).status,
    ).toBe(Status.BadArg);
    expect(MIN_COMBO_KEYS).toBe(2);

    const tooMany: Combo = {
      keys: [4, 5, 6, 7, 8],
      action: 9,
      termMs: 50,
      mustHold: false,
      mustTap: false,
      inOrder: false,
    };
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.ComboSet, 2, encodeComboSetArgs(0, tooMany)), device),
      ).status,
    ).toBe(Status.BadArg);
  });

  it('rejects the contradictory must-hold + must-tap flag pair', () => {
    const device = createFakeDevice();
    const both: Combo = {
      keys: [0x0004, 0x0005],
      action: 0x0029,
      termMs: 50,
      mustHold: true,
      mustTap: true,
      inOrder: false,
    };
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.ComboSet, 1, encodeComboSetArgs(0, both)), device),
      ).status,
    ).toBe(Status.BadArg);
  });

  it('rejects a combo whose member keys are not distinct', () => {
    const device = createFakeDevice();
    const dup: Combo = {
      keys: [0x0004, 0x0004],
      action: 0x0029,
      termMs: 50,
      mustHold: false,
      mustTap: false,
      inOrder: false,
    };
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.ComboSet, 1, encodeComboSetArgs(0, dup)), device),
      ).status,
    ).toBe(Status.BadArg);
  });

  it('an out-of-range combo index is BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.ComboGet, 1, [MAX_COMBO]), device)).status,
    ).toBe(Status.BadArg);
  });
});

describe('parseTimedInfo (mirror of TIMED_INFO)', () => {
  it('reads the timed-engine capacities, including the leader caps', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TimedInfo, 0)));
    expect(reply.status).toBe(Status.Ok);
    expect(parseTimedInfo(reply.payload)).toEqual({
      maxTapDance: MAX_TAP_DANCE,
      maxCombo: MAX_COMBO,
      maxComboKeys: MAX_COMBO_KEYS,
      maxMacro: MAX_MACRO,
      maxMacroSteps: MAX_MACRO_STEPS,
      maxLeader: MAX_LEADER,
      maxLeaderSeq: MAX_LEADER_SEQ,
    });
  });
});

describe('leader (mirror of LEADER_SET / LEADER_GET)', () => {
  const run = (cmd: number, seq: number, device = createFakeDevice(), payload?: number[]) =>
    decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

  it('parseLeader trims the sequence to len and reads the action', () => {
    // len 2, seq [KeyA=0x04, KeyB=0x05], action KeyC=0x06 (one-byte highs zeroed).
    const payload = Uint8Array.from([2, 0x04, 0, 0x05, 0, 0, 0, 0, 0, 0, 0, 0x06, 0]);
    expect(parseLeader(payload)).toEqual({ seq: [0x04, 0x05], action: 0x06 });
  });

  it('parseLeader returns null for an empty slot (len 0)', () => {
    expect(parseLeader(new Uint8Array(13))).toBeNull();
  });

  it('round-trips a leader entry through SET and a following GET', () => {
    const device = createFakeDevice();
    const leader: Leader = { seq: [0x14, 0x08, 0x16], action: 0x7700 }; // q,e,d -> MACRO(0)
    const set = run(Cmd.LeaderSet, 1, device, encodeLeaderSetArgs(2, leader));
    expect(set.status).toBe(Status.Ok);

    const got = run(Cmd.LeaderGet, 2, device, [2]);
    expect(got.status).toBe(Status.Ok);
    expect(parseLeader(got.payload)).toEqual(leader);
  });

  it('a zero-length SET clears the slot, observed by a following GET', () => {
    const device = createFakeDevice();
    run(Cmd.LeaderSet, 1, device, encodeLeaderSetArgs(0, { seq: [0x04, 0x05], action: 0x06 }));
    expect(parseLeader(run(Cmd.LeaderGet, 2, device, [0]).payload)).not.toBeNull();

    run(Cmd.LeaderSet, 3, device, encodeLeaderSetArgs(0, { seq: [], action: 0 }));
    expect(parseLeader(run(Cmd.LeaderGet, 4, device, [0]).payload)).toBeNull();
  });

  it('rejects an out-of-range index and an over-long sequence with BadArg', () => {
    expect(
      run(
        Cmd.LeaderSet,
        1,
        createFakeDevice(),
        encodeLeaderSetArgs(MAX_LEADER, { seq: [0x04], action: 0x05 }),
      ).status,
    ).toBe(Status.BadArg);
    // len above the cap: hand-craft a request with len = MAX_LEADER_SEQ + 1.
    const overlong = encodeLeaderSetArgs(0, { seq: [0x04], action: 0x05 });
    overlong[1] = MAX_LEADER_SEQ + 1;
    expect(run(Cmd.LeaderSet, 2, createFakeDevice(), overlong).status).toBe(Status.BadArg);
    expect(run(Cmd.LeaderGet, 3, createFakeDevice(), [MAX_LEADER]).status).toBe(Status.BadArg);
  });
});
