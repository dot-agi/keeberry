// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  ConnectionState,
  Devs,
  connectionStateLabel,
  encodeSleepPolicyArgs,
  parseBattery,
  parseWirelessState,
  type WirelessState,
} from './wireless';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  DEFAULT_WIRELESS_STATE,
  createFakeDevice,
  fakeFirmwareHandle,
  packBattery,
  packWirelessState,
  type WirelessStateSample,
} from './firmware-fixture';

describe('parseWirelessState (mirror of pack_wireless_state)', () => {
  it('reads [devs, state, battery, version]', () => {
    const sample: WirelessStateSample = {
      devs: Devs.G2_4,
      state: ConnectionState.Connected,
      battery: 73,
      version: 5,
    };
    const expected: WirelessState = { ...sample };
    expect(parseWirelessState(packWirelessState(sample))).toEqual(expected);
  });

  it('reads the default connected-over-BT1 snapshot', () => {
    expect(parseWirelessState(packWirelessState())).toEqual(DEFAULT_WIRELESS_STATE);
  });
});

describe('parseBattery', () => {
  it('reads the battery percent from byte 0', () => {
    expect(parseBattery(packBattery(64))).toBe(64);
  });
});

describe('Devs / ConnectionState helpers vs the firmware', () => {
  it('matches the firmware Devs codes (USB=0, BT1..3=1..3, 2.4G=6)', () => {
    expect([Devs.Usb, Devs.Bt1, Devs.Bt2, Devs.Bt3, Devs.G2_4]).toEqual([0, 1, 2, 3, 6]);
  });

  it('matches the firmware MdState codes and labels them', () => {
    expect([
      ConnectionState.None,
      ConnectionState.Pairing,
      ConnectionState.Connected,
      ConnectionState.Disconnected,
      ConnectionState.Reject,
    ]).toEqual([0, 1, 2, 3, 4]);
    expect(connectionStateLabel(ConnectionState.Pairing)).toBe('Pairing');
    expect(connectionStateLabel(ConnectionState.Connected)).toBe('Connected');
    expect(connectionStateLabel(ConnectionState.Reject)).toBe('Rejected');
    expect(connectionStateLabel(9)).toBe('Unknown');
  });
});

describe('encodeSleepPolicyArgs', () => {
  it('lays out [enable_bt, enable_2g4]', () => {
    expect(encodeSleepPolicyArgs(true, false)).toEqual([1, 0]);
    expect(encodeSleepPolicyArgs(false, true)).toEqual([0, 1]);
  });
});

describe('WIRELESS dispatch through the codec', () => {
  it('GET_STATE reflects a SET_MODE, which drops the link to Disconnected', () => {
    const device = createFakeDevice(); // seeded BT1, connected
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.WlsSetMode, 1, [Devs.G2_4]), device),
    );
    expect(set.status).toBe(Status.Ok);

    const state = parseWirelessState(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsGetState, 2), device)).payload,
    );
    expect(state.devs).toBe(Devs.G2_4);
    expect(state.state).toBe(ConnectionState.Disconnected);
  });

  it('rejects an unknown transport code with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsSetMode, 1, [5]), device)).status,
    ).toBe(Status.BadArg);
  });

  it('PAIR resets the link; UNPAIR and SET_SLEEP_POLICY answer Ok', () => {
    const device = createFakeDevice();
    expect(decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsPair, 1), device)).status).toBe(
      Status.Ok,
    );
    expect(
      parseWirelessState(
        decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsGetState, 2), device)).payload,
      ).state,
    ).toBe(ConnectionState.Disconnected);

    expect(decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsUnpair, 3), device)).status).toBe(
      Status.Ok,
    );
    expect(
      decodeReply(
        fakeFirmwareHandle(
          encodeRequest(Cmd.WlsSetSleepPolicy, 4, encodeSleepPolicyArgs(false, true)),
          device,
        ),
      ).status,
    ).toBe(Status.Ok);
    expect(device.sleepPolicy).toEqual({ bt: false, g2g4: true });
  });

  it('GET_BATTERY returns the last reported level', () => {
    const device = createFakeDevice();
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.WlsGetBattery, 1), device));
    expect(reply.status).toBe(Status.Ok);
    expect(parseBattery(reply.payload)).toBe(DEFAULT_WIRELESS_STATE.battery);
  });
});
