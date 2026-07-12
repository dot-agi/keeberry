// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * High-level kcp client: WebHID device selection, connection lifecycle, and the
 * typed group wrappers the UI consumes (INFO, KEYMAP, TELEMETRY, HID_KRO, CONFIG,
 * MACRO, RGB, BEHAVIOR, WIRELESS, TEXT, UNICODE, FEATURES, SYSTEM). Wraps a
 * {@link KcpConnection} and turns each non-OK reply into a thrown {@link KcpProtocolError}.
 */
import { DecodedReply } from './codec';
import {
  Capabilities,
  DeviceInfo,
  ProtocolVersion,
  parseCapabilities,
  parseDeviceInfo,
  parseProtocolVersion,
} from './info';
import {
  LayerConfig,
  encodeGetKeycodeArgs,
  encodeSetKeycodeArgs,
  encodeSetLayerConfigArgs,
  parseKeycodeReply,
  parseLayerConfig,
  parseLayerCount,
} from './keymap';
import { Telemetry, parseTelemetry } from './telemetry';
import { encodeSetKroArgs, parseKro } from './hidkro';
import { AutocorrectInfo, encodeSetAutocorrectArgs, parseAutocorrectInfo } from './autocorrect';
import {
  MacroInfo,
  MacroStep,
  MacroStepReadback,
  encodeMacroGetStepArgs,
  encodeMacroRecordStartArgs,
  encodeMacroSetStepArgs,
  parseMacroInfo,
  parseMacroStep,
} from './macro';
import {
  RgbState,
  ZoneState,
  ZonesInfo,
  packZoneArgs,
  parseModeList,
  parseRgbState,
  parseZone,
  parseZones,
} from './rgb';
import {
  BehaviorInfo,
  CLEAR_ALL,
  Combo,
  KeyOverride,
  Leader,
  SocdPair,
  TapDance,
  TimedInfo,
  encodeComboSetArgs,
  encodeIndexArg,
  encodeLeaderSetArgs,
  encodeOverrideSetArgs,
  encodeSocdSetArgs,
  encodeTapdanceSetArgs,
  parseBehaviorInfo,
  parseCombo,
  parseLeader,
  parseOverride,
  parseSocdPair,
  parseTapdance,
  parseTimedInfo,
} from './behavior';
import { WirelessState, encodeSleepPolicyArgs, parseBattery, parseWirelessState } from './wireless';
import {
  UnicodeInfo,
  UnicodeMode,
  encodeSetMapArgs,
  encodeSetModeArgs,
  parseUnicodeInfo,
} from './unicode';
import {
  FeatureRecord,
  encodeGetFeaturesArgs,
  encodeSetFeatureEnabledArgs,
  parseFeaturesPage,
} from './features';
import {
  DebounceConfig,
  StorageInfo,
  TuningConfig,
  encodeSetDebounceArgs,
  encodeSetTuningArgs,
  parseDebounce,
  parseStorageInfo,
  parseTuning,
} from './config';
import { UsbMode, encodeDigitizerArgs, issueReset, parseUsbMode } from './system';
import { Cmd, Status, statusLabel } from './protocol';
import { KcpConnection, TransactOptions } from './transport';
import type { Transport, TransportDevice, Unsubscribe } from './transport-iface';

/** Raised when the firmware answers a request with a non-OK status. */
export class KcpProtocolError extends Error {
  readonly status: Status;
  readonly cmd: number;

  constructor(cmd: number, status: Status) {
    super(`kcp command 0x${cmd.toString(16)} failed: ${statusLabel(status)}`);
    this.name = 'KcpProtocolError';
    this.status = status;
    this.cmd = cmd;
  }
}

export interface KcpClientOptions {
  /** Called when this client's device is unplugged or otherwise disconnected. */
  onDisconnect?: () => void;
}

/**
 * The setters whose successful write leaves an unsaved change — exactly the
 * groups `CONFIG.SAVE` persists (keymap, NKRO, RGB, the behaviour tables and
 * macros). Each applies live to device RAM and is lost on reboot until saved, so
 * a success here marks the client dirty until the next `CONFIG.SAVE`.
 * Live-but-not-persisted ops (macro *play*, the wireless group) are deliberately
 * absent — they change nothing the flash blob holds.
 */
const PERSISTED_WRITE_CMDS: ReadonlySet<number> = new Set([
  Cmd.SetKeycode,
  Cmd.SetKro,
  Cmd.RgbSetMode,
  Cmd.RgbSetHsv,
  Cmd.RgbSetBrightness,
  Cmd.RgbSetEnabled,
  Cmd.RgbSetSpeed,
  Cmd.RgbSetIndicators,
  Cmd.RgbSetZone,
  Cmd.RgbSetZoneRange,
  Cmd.RgbSetZoneSync,
  Cmd.SocdSet,
  Cmd.SocdClear,
  Cmd.OverrideSet,
  Cmd.OverrideClear,
  Cmd.TapdanceSet,
  Cmd.TapdanceClear,
  Cmd.ComboSet,
  Cmd.ComboClear,
  Cmd.MacroSetStep,
  Cmd.MacroClear,
  // RECORD_START clears the target slot (and the device then captures into it),
  // so the macro table is changed and unsaved exactly as MacroClear/SetStep are.
  Cmd.MacroRecordStart,
  Cmd.ConfigSetDebounce,
  Cmd.ConfigSetTuning,
  Cmd.LeaderSet,
  // The autocorrect flag rides the config globals block's feature-enable bitmap (schema
  // v10), so a SET is live-but-unsaved until CONFIG.SAVE — exactly like the other flags.
  Cmd.TextAutocorrectSet,
  // A feature toggle rides the same persisted enable bitmap, so it is live-but-unsaved
  // until CONFIG.SAVE.
  Cmd.SetFeatureEnabled,
  // `just new-feature --kind config` inserts each generated feature's `Cmd.<Name>Set` above
  // this anchor. Tracking the opcode (not a wrapper method) means every SET that carries it
  // marks the session dirty and arms Save — the generated descriptor's `runOp(Cmd.<Name>Set)`
  // and any typed `client.<name>Set()` wrapper a hand-built panel later adds both send it.
  // @scaffold:persisted-write-cmds
]);

/**
 * A connected keeberry device. Obtain one with {@link KcpClient.request} (which
 * prompts the user and opens the device), then call the INFO wrappers.
 */
export class KcpClient {
  private readonly connection: KcpConnection;
  private readonly device: TransportDevice;
  private readonly unsubscribeDisconnect: Unsubscribe;
  private readonly onDisconnect?: () => void;
  private closed = false;
  /** Whether a persisted-config setter has succeeded since the last save / load. */
  private dirty = false;
  /** Subscribers notified when {@link hasUnsavedChanges} flips. */
  private readonly unsavedListeners = new Set<(hasUnsaved: boolean) => void>();

  private constructor(device: TransportDevice, options?: KcpClientOptions) {
    this.device = device;
    this.connection = new KcpConnection(device);
    this.onDisconnect = options?.onDisconnect;
    this.unsubscribeDisconnect = device.onDisconnect(this.handleDisconnect);
  }

  /**
   * Prompt the user to pick a kcp device over the given {@link Transport}, open
   * it, and return a connected client. Resolves to `null` if the user dismisses
   * the chooser without selecting a device.
   */
  static async request(
    transport: Transport,
    options?: KcpClientOptions,
  ): Promise<KcpClient | null> {
    const device = await transport.requestDevice();
    return device ? KcpClient.fromDevice(device, options) : null;
  }

  /**
   * Open an already-selected {@link TransportDevice} and return a connected
   * client. {@link request} routes the browser's WebHID chooser through here; the
   * native (Tauri) build calls it directly once the user has picked from its own
   * device list, since the macOS webview has no chooser to defer to.
   */
  static async fromDevice(device: TransportDevice, options?: KcpClientOptions): Promise<KcpClient> {
    await device.open();
    return new KcpClient(device, options);
  }

  /** A human-friendly device name for the UI. */
  get name(): string {
    return this.device.name;
  }

  private readonly handleDisconnect = (): void => {
    this.cleanup();
    this.onDisconnect?.();
  };

  private async transactChecked(
    cmd: number,
    payload?: ArrayLike<number>,
    options?: TransactOptions,
  ): Promise<DecodedReply> {
    const reply = await this.connection.transact(cmd, payload, options);
    if (reply.status !== Status.Ok) {
      throw new KcpProtocolError(cmd, reply.status);
    }
    this.noteUnsavedState(cmd);
    return reply;
  }

  /**
   * Track unsaved state off a successful command. `CONFIG.SAVE` commits the
   * complete live state to flash, so it is the only op that clears the flag.
   * `CONFIG.LOAD_DEFAULTS` overwrites live RAM with the firmware defaults but does
   * NOT persist them (a reboot restores the last SAVEd config), so it is itself an
   * unsaved change and sets the flag, exactly like any persisted-config setter.
   * Reads and live-but-not-persisted ops leave the flag untouched.
   */
  private noteUnsavedState(cmd: number): void {
    if (cmd === Cmd.ConfigSave) {
      this.setUnsaved(false);
    } else if (cmd === Cmd.ConfigLoadDefaults || PERSISTED_WRITE_CMDS.has(cmd)) {
      this.setUnsaved(true);
    }
  }

  private setUnsaved(value: boolean): void {
    if (this.dirty === value) {
      return;
    }
    this.dirty = value;
    for (const listener of this.unsavedListeners) {
      listener(value);
    }
  }

  /** Whether a persisted-config setter has succeeded since the last save / load. */
  get hasUnsavedChanges(): boolean {
    return this.dirty;
  }

  /**
   * Subscribe to unsaved-changes transitions; the listener fires with the new
   * value whenever it flips. Returns an unsubscribe function.
   */
  onUnsavedChange(listener: (hasUnsaved: boolean) => void): () => void {
    this.unsavedListeners.add(listener);
    return () => {
      this.unsavedListeners.delete(listener);
    };
  }

  /** INFO 0x00 — read the kcp protocol version. */
  async getProtocolVersion(): Promise<ProtocolVersion> {
    const reply = await this.transactChecked(Cmd.GetVersion);
    return parseProtocolVersion(reply.payload);
  }

  /** INFO 0x01 — read and decode the capabilities bitmask. */
  async getCapabilities(): Promise<Capabilities> {
    const reply = await this.transactChecked(Cmd.GetCapabilities);
    return parseCapabilities(reply.payload);
  }

  /** INFO 0x02 — read the static device descriptor. */
  async getDeviceInfo(): Promise<DeviceInfo> {
    const reply = await this.transactChecked(Cmd.GetDeviceInfo);
    return parseDeviceInfo(reply.payload);
  }

  // === KEYMAP group (0x1x) =================================================

  /** KEYMAP 0x10 — read the keycode bound at a matrix position (u16 LE). */
  async getKeycode(layer: number, row: number, col: number): Promise<number> {
    const reply = await this.transactChecked(Cmd.GetKeycode, encodeGetKeycodeArgs(layer, row, col));
    return parseKeycodeReply(reply.payload);
  }

  /** KEYMAP 0x11 — bind a keycode at a matrix position; applied live. */
  async setKeycode(layer: number, row: number, col: number, keycode: number): Promise<void> {
    await this.transactChecked(Cmd.SetKeycode, encodeSetKeycodeArgs(layer, row, col, keycode));
  }

  /** KEYMAP 0x12 — read the number of keymap layers. */
  async getLayerCount(): Promise<number> {
    const reply = await this.transactChecked(Cmd.GetLayerCount);
    return parseLayerCount(reply.payload);
  }

  /** KEYMAP 0x13 — read the layer config (the default `DF` layer and the tri-layer rule). */
  async getLayerConfig(): Promise<LayerConfig> {
    const reply = await this.transactChecked(Cmd.GetLayerConfig);
    return parseLayerConfig(reply.payload);
  }

  /** KEYMAP 0x14 — set the layer config; applied live, persisted by the next CONFIG.SAVE. */
  async setLayerConfig(cfg: LayerConfig): Promise<void> {
    await this.transactChecked(Cmd.SetLayerConfig, encodeSetLayerConfigArgs(cfg));
  }

  // === TELEMETRY group (0x2x) ==============================================

  /** TELEMETRY 0x20 — read a live telemetry snapshot. */
  async getTelemetry(): Promise<Telemetry> {
    const reply = await this.transactChecked(Cmd.GetTelemetry);
    return parseTelemetry(reply.payload);
  }

  // === HID_KRO group (0x3x) ================================================

  /** HID_KRO 0x30 — read the rollover mode (`true` = NKRO, `false` = boot 6KRO). */
  async getKro(): Promise<boolean> {
    const reply = await this.transactChecked(Cmd.GetKro);
    return parseKro(reply.payload);
  }

  /** HID_KRO 0x31 — set the rollover mode; applied live on the next scan. */
  async setKro(nkroEnabled: boolean): Promise<void> {
    await this.transactChecked(Cmd.SetKro, encodeSetKroArgs(nkroEnabled));
  }

  // === RGB group (0x6x) ====================================================

  /** RGB 0x60 — set the effect mode (an out-of-range id throws BadArg). */
  async rgbSetMode(mode: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetMode, [mode & 0xff]);
  }

  /** RGB 0x61 — set the effect colour (each component 0..=255). */
  async rgbSetHsv(h: number, s: number, v: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetHsv, [h & 0xff, s & 0xff, v & 0xff]);
  }

  /** RGB 0x62 — set the master brightness (0..=255; output clamped to 84). */
  async rgbSetBrightness(value: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetBrightness, [value & 0xff]);
  }

  /** RGB 0x63 — enable or disable RGB output. */
  async rgbSetEnabled(enabled: boolean): Promise<void> {
    await this.transactChecked(Cmd.RgbSetEnabled, [enabled ? 1 : 0]);
  }

  /** RGB 0x64 — read the live RGB state. */
  async rgbGetState(): Promise<RgbState> {
    const reply = await this.transactChecked(Cmd.RgbGetState);
    return parseRgbState(reply.payload);
  }

  /** RGB 0x65 — list the available effect-mode ids. */
  async rgbListModes(): Promise<number[]> {
    // The firmware pages LIST_MODES by start offset (the set outgrew one reply); request
    // successive pages from where we left off until we have all `total` ids.
    const ids: number[] = [];
    for (;;) {
      const reply = await this.transactChecked(Cmd.RgbListModes, [ids.length]);
      const page = parseModeList(reply.payload);
      ids.push(...page.ids);
      if (page.ids.length === 0 || ids.length >= page.total) {
        return ids;
      }
    }
  }

  /** RGB 0x66 — set the animation speed (0..=255). */
  async rgbSetSpeed(speed: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetSpeed, [speed & 0xff]);
  }

  /** RGB 0x67 — enable or disable the status-indicator overlay (persisted from schema v7). */
  async rgbSetIndicators(enabled: boolean): Promise<void> {
    await this.transactChecked(Cmd.RgbSetIndicators, [enabled ? 1 : 0]);
  }

  /**
   * RGB 0x68 — read the zone-table summary (zone count + capacity). Throws a
   * {@link KcpProtocolError} with `BadCmd` on firmware predating the zone ops, so the
   * UI can hide the Zones panel.
   */
  async rgbGetZones(): Promise<ZonesInfo> {
    const reply = await this.transactChecked(Cmd.RgbGetZones);
    return parseZones(reply.payload);
  }

  /** RGB 0x69 — read one zone's state (an out-of-range id throws BadArg). */
  async rgbGetZone(id: number): Promise<ZoneState> {
    const reply = await this.transactChecked(Cmd.RgbGetZone, [id & 0xff]);
    return parseZone(reply.payload);
  }

  /** RGB 0x6A — set one zone's effect (an out-of-range id or mode throws BadArg). */
  async rgbSetZone(zone: ZoneState): Promise<void> {
    await this.transactChecked(Cmd.RgbSetZone, packZoneArgs(zone));
  }

  /**
   * RGB 0x6B — set one zone's LED range. The range must fit the chain and stay
   * disjoint from the other lit zones; an out-of-range id, an over-long range or an
   * overlap throws BadArg.
   */
  async rgbSetZoneRange(id: number, start: number, count: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetZoneRange, [
      id & 0xff,
      start & 0xff,
      (start >> 8) & 0xff,
      count & 0xff,
      (count >> 8) & 0xff,
    ]);
  }

  /**
   * RGB 0x6D — set one zone's sync source: zone `id` mirrors zone `target`'s effect in
   * its own LED range, or `target = 0xFF` (`ZONE_SYNC_NONE`) clears the link. An
   * out-of-range id/target, a self-sync or a link that would close a sync cycle throws
   * BadArg.
   */
  async rgbSetZoneSync(id: number, target: number): Promise<void> {
    await this.transactChecked(Cmd.RgbSetZoneSync, [id & 0xff, target & 0xff]);
  }

  // === CONFIG group (0x4x) =================================================

  /** CONFIG 0x40 — persist the complete live config to flash (throws Busy on failure). */
  async configSave(): Promise<void> {
    await this.transactChecked(Cmd.ConfigSave);
  }

  /** CONFIG 0x41 — reset the complete live config to the firmware defaults (RAM-only). */
  async configLoadDefaults(): Promise<void> {
    await this.transactChecked(Cmd.ConfigLoadDefaults);
  }

  /** CONFIG 0x42 — describe the persistence region and stored blob. */
  async getStorageInfo(): Promise<StorageInfo> {
    const reply = await this.transactChecked(Cmd.ConfigGetStorageInfo);
    return parseStorageInfo(reply.payload);
  }

  /** CONFIG 0x43 — read the matrix debounce configuration. */
  async getDebounce(): Promise<DebounceConfig> {
    const reply = await this.transactChecked(Cmd.ConfigGetDebounce);
    return parseDebounce(reply.payload);
  }

  /** CONFIG 0x44 — set the matrix debounce config (applied live on the next scan). */
  async setDebounce(cfg: DebounceConfig): Promise<void> {
    await this.transactChecked(Cmd.ConfigSetDebounce, encodeSetDebounceArgs(cfg));
  }

  /** CONFIG 0x45 — read the runtime tunables (auto-shift on/off + timeout, leader timeout). */
  async getTuning(): Promise<TuningConfig> {
    const reply = await this.transactChecked(Cmd.ConfigGetTuning);
    return parseTuning(reply.payload);
  }

  /** CONFIG 0x46 — set the runtime tunables (a zero timeout throws BadArg); applied live. */
  async setTuning(cfg: TuningConfig): Promise<void> {
    await this.transactChecked(Cmd.ConfigSetTuning, encodeSetTuningArgs(cfg));
  }

  // === MACRO group (0x5x) ==================================================

  /** MACRO 0x50 — read the macro table capacities and the used-slot bitmap. */
  async macroInfo(): Promise<MacroInfo> {
    const reply = await this.transactChecked(Cmd.MacroInfo);
    return parseMacroInfo(reply.payload);
  }

  /** MACRO 0x52 — read one macro step and the macro's active length. */
  async macroGetStep(macro: number, step: number): Promise<MacroStepReadback> {
    const reply = await this.transactChecked(Cmd.MacroGetStep, encodeMacroGetStepArgs(macro, step));
    return parseMacroStep(reply.payload);
  }

  /** MACRO 0x51 — set one macro step (growing the macro to cover it); applied live. */
  async macroSetStep(macro: number, step: number, ev: MacroStep): Promise<void> {
    await this.transactChecked(Cmd.MacroSetStep, encodeMacroSetStepArgs(macro, step, ev));
  }

  /** MACRO 0x53 — clear a single macro (length to zero). */
  async macroClear(macro: number): Promise<void> {
    await this.transactChecked(Cmd.MacroClear, encodeIndexArg(macro));
  }

  /** MACRO 0x53 — clear every macro (index `0xFF`). */
  async macroClearAll(): Promise<void> {
    await this.transactChecked(Cmd.MacroClear, encodeIndexArg(CLEAR_ALL));
  }

  /** MACRO 0x54 — play a macro now (an out-of-range or empty macro throws BadArg). */
  async macroPlay(macro: number): Promise<void> {
    await this.transactChecked(Cmd.MacroPlay, encodeIndexArg(macro));
  }

  /**
   * MACRO 0x55 — start on-board recording into a macro slot. The slot is cleared
   * and subsequent key presses/releases are captured (with their timing) until
   * {@link macroRecordStop}; the keys still type live while recording. An
   * out-of-range macro throws BadArg.
   */
  async macroRecordStart(macro: number): Promise<void> {
    await this.transactChecked(Cmd.MacroRecordStart, encodeMacroRecordStartArgs(macro));
  }

  /** MACRO 0x56 — stop on-board recording (a no-op success if none is in progress). */
  async macroRecordStop(): Promise<void> {
    await this.transactChecked(Cmd.MacroRecordStop);
  }

  // === BEHAVIOR group (0x7x) ===============================================

  /** BEHAVIOR 0x76 — read the SOCD / override table capacities. */
  async behaviorInfo(): Promise<BehaviorInfo> {
    const reply = await this.transactChecked(Cmd.BehaviorInfo);
    return parseBehaviorInfo(reply.payload);
  }

  /** BEHAVIOR 0x72 — read a SOCD slot (`null` for an empty slot). */
  async socdGet(index: number): Promise<SocdPair | null> {
    const reply = await this.transactChecked(Cmd.SocdGet, encodeIndexArg(index));
    return parseSocdPair(reply.payload);
  }

  /** BEHAVIOR 0x70 — configure a SOCD slot; applied live. */
  async socdSet(index: number, pair: SocdPair): Promise<void> {
    await this.transactChecked(Cmd.SocdSet, encodeSocdSetArgs(index, pair));
  }

  /** BEHAVIOR 0x71 — clear a single SOCD slot. */
  async socdClear(index: number): Promise<void> {
    await this.transactChecked(Cmd.SocdClear, encodeIndexArg(index));
  }

  /** BEHAVIOR 0x71 — clear every SOCD slot (index `0xFF`). */
  async socdClearAll(): Promise<void> {
    await this.transactChecked(Cmd.SocdClear, encodeIndexArg(CLEAR_ALL));
  }

  /** BEHAVIOR 0x75 — read a key-override slot (`null` for an empty slot). */
  async overrideGet(index: number): Promise<KeyOverride | null> {
    const reply = await this.transactChecked(Cmd.OverrideGet, encodeIndexArg(index));
    return parseOverride(reply.payload);
  }

  /** BEHAVIOR 0x73 — configure a key-override slot; applied live. */
  async overrideSet(index: number, override: KeyOverride): Promise<void> {
    await this.transactChecked(Cmd.OverrideSet, encodeOverrideSetArgs(index, override));
  }

  /** BEHAVIOR 0x74 — clear a single key-override slot. */
  async overrideClear(index: number): Promise<void> {
    await this.transactChecked(Cmd.OverrideClear, encodeIndexArg(index));
  }

  /** BEHAVIOR 0x74 — clear every key-override slot (index `0xFF`). */
  async overrideClearAll(): Promise<void> {
    await this.transactChecked(Cmd.OverrideClear, encodeIndexArg(CLEAR_ALL));
  }

  /** BEHAVIOR 0x7D — read the timed-engine table capacities (tap-dance, combo, macro). */
  async timedInfo(): Promise<TimedInfo> {
    const reply = await this.transactChecked(Cmd.TimedInfo);
    return parseTimedInfo(reply.payload);
  }

  /** BEHAVIOR 0x78 — read a tap-dance slot (`null` for an empty slot). */
  async tapdanceGet(index: number): Promise<TapDance | null> {
    const reply = await this.transactChecked(Cmd.TapdanceGet, encodeIndexArg(index));
    return parseTapdance(reply.payload);
  }

  /** BEHAVIOR 0x77 — configure a tap-dance slot; applied live. */
  async tapdanceSet(index: number, td: TapDance): Promise<void> {
    await this.transactChecked(Cmd.TapdanceSet, encodeTapdanceSetArgs(index, td));
  }

  /** BEHAVIOR 0x79 — clear a single tap-dance slot. */
  async tapdanceClear(index: number): Promise<void> {
    await this.transactChecked(Cmd.TapdanceClear, encodeIndexArg(index));
  }

  /** BEHAVIOR 0x79 — clear every tap-dance slot (index `0xFF`). */
  async tapdanceClearAll(): Promise<void> {
    await this.transactChecked(Cmd.TapdanceClear, encodeIndexArg(CLEAR_ALL));
  }

  /** BEHAVIOR 0x7B — read a combo slot (`null` for an empty slot). */
  async comboGet(index: number): Promise<Combo | null> {
    const reply = await this.transactChecked(Cmd.ComboGet, encodeIndexArg(index));
    return parseCombo(reply.payload);
  }

  /** BEHAVIOR 0x7A — configure a combo slot (a bad key count throws BadArg); applied live. */
  async comboSet(index: number, combo: Combo): Promise<void> {
    await this.transactChecked(Cmd.ComboSet, encodeComboSetArgs(index, combo));
  }

  /** BEHAVIOR 0x7C — clear a single combo slot. */
  async comboClear(index: number): Promise<void> {
    await this.transactChecked(Cmd.ComboClear, encodeIndexArg(index));
  }

  /** BEHAVIOR 0x7C — clear every combo slot (index `0xFF`). */
  async comboClearAll(): Promise<void> {
    await this.transactChecked(Cmd.ComboClear, encodeIndexArg(CLEAR_ALL));
  }

  /** BEHAVIOR 0x7F — read a leader-sequence slot (`null` for an empty slot). */
  async leaderGet(index: number): Promise<Leader | null> {
    const reply = await this.transactChecked(Cmd.LeaderGet, encodeIndexArg(index));
    return parseLeader(reply.payload);
  }

  /** BEHAVIOR 0x7E — configure a leader-sequence slot; applied live. */
  async leaderSet(index: number, leader: Leader): Promise<void> {
    await this.transactChecked(Cmd.LeaderSet, encodeLeaderSetArgs(index, leader));
  }

  /** BEHAVIOR 0x7E — clear a single leader-sequence slot (send a zero-length sequence). */
  async leaderClear(index: number): Promise<void> {
    await this.transactChecked(Cmd.LeaderSet, encodeLeaderSetArgs(index, { seq: [], action: 0 }));
  }

  /**
   * Clear every leader-sequence slot. Leader has no whole-table clear opcode (unlike
   * SOCD / override / tap-dance / combo, whose CLEAR ops take the `0xFF` sentinel), so
   * this empties each slot in turn — its count read from TIMED_INFO — the host-side
   * equivalent of the firmware's `timed::leader_clear_all`.
   */
  async leaderClearAll(): Promise<void> {
    const { maxLeader } = await this.timedInfo();
    for (let index = 0; index < maxLeader; index += 1) {
      await this.leaderClear(index);
    }
  }

  // === WIRELESS group (0x8x) ===============================================

  /** WIRELESS 0x80 — read the link snapshot (transport, state, battery, version). */
  async wirelessGetState(): Promise<WirelessState> {
    const reply = await this.transactChecked(Cmd.WlsGetState);
    return parseWirelessState(reply.payload);
  }

  /** WIRELESS 0x81 — select the output transport (an unknown code throws BadArg). */
  async wirelessSetMode(devs: number): Promise<void> {
    await this.transactChecked(Cmd.WlsSetMode, [devs & 0xff]);
  }

  /** WIRELESS 0x82 — (re)pair the current transport. */
  async wirelessPair(): Promise<void> {
    await this.transactChecked(Cmd.WlsPair);
  }

  /** WIRELESS 0x83 — clear the active channel's bond. */
  async wirelessUnpair(): Promise<void> {
    await this.transactChecked(Cmd.WlsUnpair);
  }

  /** WIRELESS 0x84 — set the radio idle-sleep policy for BT and 2.4 GHz. */
  async wirelessSetSleepPolicy(enableBt: boolean, enable2g4: boolean): Promise<void> {
    await this.transactChecked(Cmd.WlsSetSleepPolicy, encodeSleepPolicyArgs(enableBt, enable2g4));
  }

  /** WIRELESS 0x85 — read the battery level and trigger a fresh measurement. */
  async wirelessGetBattery(): Promise<number> {
    const reply = await this.transactChecked(Cmd.WlsGetBattery);
    return parseBattery(reply.payload);
  }

  // === TEXT group (0x9x) ===================================================

  /** TEXT 0x90 — read the autocorrect state (enabled flag + compiled-in dictionary size). */
  async getAutocorrect(): Promise<AutocorrectInfo> {
    const reply = await this.transactChecked(Cmd.TextAutocorrectInfo);
    return parseAutocorrectInfo(reply.payload);
  }

  /** TEXT 0x91 — enable or disable autocorrect; applied live, persisted by the next CONFIG.SAVE. */
  async setAutocorrect(enabled: boolean): Promise<void> {
    await this.transactChecked(Cmd.TextAutocorrectSet, encodeSetAutocorrectArgs(enabled));
  }

  // === UNICODE group (0xAx) ================================================
  //
  // The active OS input mode plus the host-uploaded codepoint map. The map is
  // RAM-only on the device (no CONFIG.SAVE persistence), so these setters are
  // live-but-not-persisted — deliberately absent from PERSISTED_WRITE_CMDS — and
  // the panel re-uploads the map on every connect.

  /** UNICODE 0xA0 — read the active OS input mode and the slot / mode counts. */
  async unicodeGet(): Promise<UnicodeInfo> {
    const reply = await this.transactChecked(Cmd.UnicodeGet);
    return parseUnicodeInfo(reply.payload);
  }

  /** UNICODE 0xA1 — select the active OS input mode (an out-of-range mode throws BadArg). */
  async unicodeSetMode(mode: UnicodeMode): Promise<void> {
    await this.transactChecked(Cmd.UnicodeSetMode, encodeSetModeArgs(mode));
  }

  /**
   * UNICODE 0xA2 — upload one codepoint slot. A `0` codepoint clears the slot
   * (it then types nothing); an out-of-range slot throws BadArg.
   */
  async unicodeSetMap(slot: number, codepoint: number): Promise<void> {
    await this.transactChecked(Cmd.UnicodeSetMap, encodeSetMapArgs(slot, codepoint));
  }

  // === FEATURES group (0xDx) ===============================================

  /**
   * FEATURES 0xD0 — enumerate every registered feature and its runtime enable. The
   * firmware pages the records (the set outgrows one reply), so request successive pages
   * from where we left off until we have all `count`; the stable `id` keys the records,
   * never their position.
   */
  async listFeatures(): Promise<FeatureRecord[]> {
    const records: FeatureRecord[] = [];
    for (;;) {
      const reply = await this.transactChecked(
        Cmd.GetFeatures,
        encodeGetFeaturesArgs(records.length),
      );
      const page = parseFeaturesPage(reply.payload);
      records.push(...page.records);
      if (page.records.length === 0 || records.length >= page.count) {
        return records;
      }
    }
  }

  /**
   * FEATURES 0xD1 — switch one feature on or off by its stable id; applied live,
   * persisted by the next CONFIG.SAVE. Disabling an always-on (structural) feature, or
   * an unknown id, throws BadArg.
   */
  async setFeatureEnabled(id: number, enabled: boolean): Promise<void> {
    await this.transactChecked(Cmd.SetFeatureEnabled, encodeSetFeatureEnabledArgs(id, enabled));
  }

  // === SYSTEM group (0xFx) =================================================
  //
  // Both ops reset the MCU, which never replies (it resets before it could), so
  // these are fire-and-forget: we issue the command and let the ensuing USB
  // disconnect stand in as the acknowledgement (see {@link issueReset}). Using
  // `transact` here would always time out, since no reply is ever sent. The
  // disconnect drives `onDisconnect`, which the UI uses to return to idle.

  /**
   * SYSTEM 0xF0 — reset into the wb32-dfu bootloader for re-flashing. The device
   * drops off USB to enter DFU; that disconnect is the acknowledgement, so this
   * resolves without waiting for (or surfacing the absence of) a reply.
   */
  async enterDfu(): Promise<void> {
    await issueReset((cmd) => this.connection.send(cmd), Cmd.SystemEnterDfu);
  }

  /** SYSTEM 0xF1 — reboot the firmware (the device disconnects, then re-enumerates). */
  async reboot(): Promise<void> {
    await issueReset((cmd) => this.connection.send(cmd), Cmd.SystemReboot);
  }

  /**
   * SYSTEM 0xF2 — select the USB personality (Normal / MIDI / XInput). A change
   * re-enumerates the device, dropping it off USB, so this is fire-and-forget like
   * the reset ops: the disconnect is the acknowledgement (and drives `onDisconnect`,
   * which the UI uses to return to idle). Re-selecting the current mode replies
   * normally and stays connected.
   */
  async setUsbMode(mode: UsbMode): Promise<void> {
    await issueReset((cmd) => this.connection.send(cmd, [mode]), Cmd.SystemSetUsbMode);
  }

  /** SYSTEM 0xF3 — read the current USB personality. */
  async getUsbMode(): Promise<UsbMode> {
    const reply = await this.transactChecked(Cmd.SystemGetUsbMode);
    return parseUsbMode(reply.payload);
  }

  /**
   * SYSTEM 0xF4 — set the HID digitizer's absolute pointer position (a host/test
   * control). `x`/`y` are in `0..=32767`; `tip` is the tip switch (touching) and
   * `inRange` whether the pointer is in sensing range. Reported on the shared
   * interface's digitizer report in the normal personality.
   */
  async setDigitizer(x: number, y: number, tip: boolean, inRange: boolean): Promise<void> {
    await this.transactChecked(Cmd.SystemSetDigitizer, encodeDigitizerArgs(x, y, tip, inRange));
  }

  // === Generic op path (the descriptor SDK) ================================

  /**
   * Run an arbitrary kcp op by its raw command byte and request args, resolving the decoded
   * reply payload (a non-OK status throws {@link KcpProtocolError}). The data-driven
   * `DescriptorPanel` dispatches a `FeatureDescriptor`'s get/set ops through this generic path
   * — it carries command numbers, not typed wrappers — so a feature's whole config surface can
   * render from data. The typed group wrappers above stay the preferred call site for
   * hand-built panels; unsaved-state tracking still applies, so a descriptor SET of a persisted
   * op marks the client dirty exactly like its typed equivalent.
   */
  async runOp(cmd: number, args?: number[]): Promise<Uint8Array> {
    const reply = await this.transactChecked(cmd, args);
    return reply.payload;
  }

  private cleanup(): void {
    if (this.closed) {
      return;
    }
    this.closed = true;
    this.unsubscribeDisconnect();
    this.connection.close();
  }

  /** Detach listeners and close the underlying device. */
  async close(): Promise<void> {
    this.cleanup();
    await this.device.close();
  }
}
