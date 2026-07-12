// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { parseAutocorrectInfo } from './autocorrect';
import { decodeReply, encodeRequest } from './codec';
import {
  encodeGetFeaturesArgs,
  encodeSetFeatureEnabledArgs,
  parseFeaturesPage,
  type FeatureRecord,
} from './features';
import { Cmd, Status } from './protocol';
import { FEATURE_DEFS, createFakeDevice, fakeFirmwareHandle, type FakeDevice } from './firmware-fixture';

/** Page every feature out of the fixture exactly as `KcpClient.listFeatures` does. */
function listAllFeatures(device: FakeDevice): FeatureRecord[] {
  const all: FeatureRecord[] = [];
  for (;;) {
    const reply = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.GetFeatures, 0, encodeGetFeaturesArgs(all.length)), device),
    );
    const page = parseFeaturesPage(reply.payload);
    all.push(...page.records);
    if (page.records.length === 0 || all.length >= page.count) {
      return all;
    }
  }
}

describe('parseFeaturesPage / encode args (mirror of the FEATURES dispatch)', () => {
  it('reads count, page_len and each {id, enabled, name_len, name} record', () => {
    // [count=2, page_len=2, {id 3, on, "Hi"}, {id 4, off, "Yo"}]
    const payload = new Uint8Array([2, 2, 3, 1, 2, 0x48, 0x69, 4, 0, 2, 0x59, 0x6f]);
    expect(parseFeaturesPage(payload)).toEqual({
      count: 2,
      records: [
        { id: 3, enabled: true, name: 'Hi' },
        { id: 4, enabled: false, name: 'Yo' },
      ],
    });
  });

  it('reads only page_len records, ignoring trailing bytes from an earlier page', () => {
    // page_len = 1 but two records' worth of bytes follow; only the first is read.
    const payload = new Uint8Array([5, 1, 0, 1, 1, 0x41, 1, 0, 1, 0x42]);
    expect(parseFeaturesPage(payload)).toEqual({
      count: 5,
      records: [{ id: 0, enabled: true, name: 'A' }],
    });
  });

  it('encodes the request payloads', () => {
    expect(encodeGetFeaturesArgs(7)).toEqual([7]);
    expect(encodeSetFeatureEnabledArgs(3, true)).toEqual([3, 1]);
    expect(encodeSetFeatureEnabledArgs(2, false)).toEqual([2, 0]);
  });
});

describe('FEATURES dispatch through the codec (enumerate + toggle)', () => {
  it('enumerates every registered feature, all enabled at boot, paged by the device', () => {
    const device = createFakeDevice();
    const all = listAllFeatures(device);

    // Every FEATURE_DEFS entry appears, in registry order, with its id, name and default-on.
    expect(all).toEqual(FEATURE_DEFS.map((f) => ({ id: f.id, enabled: true, name: f.name })));
    // The set outgrew one 29-byte frame, so the device must have paged it.
    expect(all.length).toBeGreaterThan(0);
  });

  it('toggles one feature by id and observes it on the next enumeration', () => {
    const device = createFakeDevice();
    const capsWord = FEATURE_DEFS.find((f) => f.name === 'Caps Word')!;

    const set = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetFeatureEnabled, 1, encodeSetFeatureEnabledArgs(capsWord.id, false)),
        device,
      ),
    );
    expect(set.status).toBe(Status.Ok);

    const after = listAllFeatures(device).find((f) => f.id === capsWord.id);
    expect(after?.enabled).toBe(false);
  });

  it('refuses to disable an always-on (structural) feature with BadArg', () => {
    const device = createFakeDevice();
    const timed = FEATURE_DEFS.find((f) => f.name === 'Timed Engine')!;
    expect(timed.alwaysOn).toBe(true);

    const reply = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetFeatureEnabled, 1, encodeSetFeatureEnabledArgs(timed.id, false)),
        device,
      ),
    );
    expect(reply.status).toBe(Status.BadArg);
    // Enabling it again is a no-op success, never an error.
    const reEnable = decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetFeatureEnabled, 2, encodeSetFeatureEnabledArgs(timed.id, true)),
        device,
      ),
    );
    expect(reEnable.status).toBe(Status.Ok);
  });

  it('rejects an unknown id or a non-boolean value with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SetFeatureEnabled, 1, [0xfe, 1]), device)).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SetFeatureEnabled, 2, [3, 2]), device)).status,
    ).toBe(Status.BadArg);
  });

  it('shares the autocorrect enable between the FEATURES and TEXT groups (one folded bit)', () => {
    const device = createFakeDevice();
    const autocorrect = FEATURE_DEFS.find((f) => f.name === 'Autocorrect')!;

    // Disable autocorrect through the FEATURES group...
    decodeReply(
      fakeFirmwareHandle(
        encodeRequest(Cmd.SetFeatureEnabled, 1, encodeSetFeatureEnabledArgs(autocorrect.id, false)),
        device,
      ),
    );
    // ...and the TEXT group's AUTOCORRECT_INFO sees it.
    const info = parseAutocorrectInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectInfo, 2), device)).payload,
    );
    expect(info.enabled).toBe(false);

    // Re-enable through the TEXT group, and the FEATURES enumeration sees it.
    decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectSet, 3, [1]), device));
    const back = listAllFeatures(device).find((f) => f.id === autocorrect.id);
    expect(back?.enabled).toBe(true);
  });
});
