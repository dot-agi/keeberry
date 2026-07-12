// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import {
  hsvToRgb,
  LED_COUNT,
  rgbModeLabel,
  rgbToCss,
  withZoneFlag,
  zoneEnabled,
  zoneLabel,
  zoneLinked,
  zoneRangeOverlaps,
  zoneSynced,
  ZONE_FLAG_ENABLED,
  ZONE_FLAG_LINKED,
  ZONE_SYNC_NONE,
  type KcpClient,
  type RgbState,
  type ZoneState,
} from '../kcp';
import { ErrorBanner, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

/**
 * The Rainbow effect (`rgb.rs` MODE_RAINBOW) sweeps the hue wheel itself, so a fixed
 * hue is inert under it. Keyed on the mode id rather than its label, so renaming the
 * label can never silently bring the (useless) Hue slider back.
 */
const RGB_MODE_RAINBOW = 2;

interface RgbPanelProps {
  client: KcpClient;
}

/**
 * Live RGB control: an effect-mode dropdown (from `listModes`, including the
 * keypress-reactive effects), an HSV colour picker with a device-matched preview
 * swatch, master-brightness and speed sliders, an on/off toggle and a
 * status-indicator overlay toggle, plus a Zones list (Keys / Right / Left) — each
 * zone matches the base effect, runs its own effect, or is blanked, can be resized to
 * a sub-range of the chain, and (the side strips) can live-mirror another zone via a
 * "Sync to…" control. The panel reflects `getState` (and the zone table) on load and
 * writes every change live over the RGB ops. The
 * firmware clamps emitted brightness to 84 but keeps the full 0..=255 stored range,
 * so the slider spans 0..255 and notes the cap.
 */
export function RgbPanel({ client }: RgbPanelProps) {
  const [state, setState] = useState<RgbState | null>(null);
  const [modes, setModes] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [zones, setZones] = useState<ZoneState[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [modeIds, current] = await Promise.all([client.rgbListModes(), client.rgbGetState()]);
        if (cancelled) return;
        setModes(modeIds);
        setState(current);
        const info = await client.rgbGetZones();
        const list = await Promise.all(
          Array.from({ length: info.zoneCount }, (_unused, id) => client.rgbGetZone(id)),
        );
        if (cancelled) return;
        setZones(list);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  /** Apply a local state patch immediately, then push it to the device. */
  async function patch(next: Partial<RgbState>, write: () => Promise<void>) {
    const previous = state;
    setState((prev) => (prev ? { ...prev, ...next } : prev));
    setError(null);
    try {
      await write();
    } catch (err) {
      setError(friendlyPanelError(err));
      // The optimistic value was rejected — resync from the device so a failed
      // write never sticks, falling back to the pre-edit snapshot.
      try {
        setState(await client.rgbGetState());
      } catch {
        setState(previous);
      }
    }
  }

  /** Update a zone's local state only (no write) — for live slider dragging. */
  function previewZone(id: number, next: Partial<ZoneState>) {
    setZones((prev) => (prev ? prev.map((z) => (z.id === id ? { ...z, ...next } : z)) : prev));
  }

  /**
   * Apply a zone patch optimistically, then run `write` with the merged zone. Resyncs
   * just that zone from the device on failure, so a rejected write never sticks. The
   * three zone ops (effect, range, sync) layer on this one helper.
   */
  async function commitZone(
    id: number,
    next: Partial<ZoneState>,
    write: (merged: ZoneState) => Promise<void>,
  ) {
    if (!zones) return;
    const previous = zones;
    const merged = zones.map((z) => (z.id === id ? { ...z, ...next } : z));
    setZones(merged);
    setError(null);
    const target = merged.find((z) => z.id === id);
    if (!target) return;
    try {
      await write(target);
    } catch (err) {
      setError(friendlyPanelError(err));
      try {
        const fresh = await client.rgbGetZone(id);
        setZones((prev) => (prev ? prev.map((z) => (z.id === id ? fresh : z)) : prev));
      } catch {
        setZones(previous);
      }
    }
  }

  /** Write a zone's whole effect block (the firmware's SET_ZONE is one atomic update). */
  const patchZone = async (id: number, next: Partial<ZoneState>) => {
    const current = zones?.find((z) => z.id === id);
    // Mirror the firmware `set_zone` guard: enabling a zone (an off->on transition of
    // ENABLED) must keep the lit ranges disjoint. SET_ZONE leaves the range untouched,
    // so block a re-enable that would overlap another lit zone before the round-trip;
    // the firmware re-checks authoritatively across the whole table.
    if (
      zones &&
      current &&
      next.flags !== undefined &&
      (next.flags & ZONE_FLAG_ENABLED) !== 0 &&
      !zoneEnabled(current) &&
      zoneRangeOverlaps(zones, id, current.start, current.count)
    ) {
      setError('Enabling this zone would overlap another lit zone.');
      return;
    }
    await commitZone(id, next, (z) => client.rgbSetZone(z));
  };

  /** Resize a zone's LED range (SET_ZONE_RANGE). */
  const patchZoneRange = (id: number, start: number, count: number) =>
    commitZone(id, { start, count }, (z) => client.rgbSetZoneRange(z.id, z.start, z.count));

  /** Link or unlink a zone's sync source (SET_ZONE_SYNC; `ZONE_SYNC_NONE` clears). */
  const patchZoneSync = (id: number, target: number) =>
    commitZone(id, { syncTo: target }, (z) => client.rgbSetZoneSync(z.id, z.syncTo));

  if (!state) {
    return (
      <Panel
        title="Lighting"
        description="Pick an effect, tune color, and set brightness for the lighting rail."
      >
        {error ? (
          <ErrorBanner>{error}</ErrorBanner>
        ) : (
          <p className="text-sm text-slate-400">Checking lighting state…</p>
        )}
      </Panel>
    );
  }

  const swatch = rgbToCss(hsvToRgb(state.hue, state.sat, state.val));
  // Only the hue is inert under Rainbow — saturation and intensity still shape the sweep.
  const isRainbow = state.mode === RGB_MODE_RAINBOW;

  return (
    <Panel
      title="Lighting"
      description="Pick an effect, tune color, and set brightness for the lighting rail."
      headerExtra={
        <Toggle
          label="Lighting"
          checked={state.enabled}
          onChange={(next) => void patch({ enabled: next }, () => client.rgbSetEnabled(next))}
        />
      }
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <div className="grid gap-5 sm:grid-cols-[minmax(0,1fr)_9rem]">
        <div className="flex flex-col gap-4">
          <label className="flex flex-col gap-1.5">
            <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
              Effect
            </span>
            <select
              value={state.mode}
              onChange={(event) => {
                const mode = Number(event.target.value);
                void patch({ mode }, () => client.rgbSetMode(mode));
              }}
              className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
            >
              {modes.map((id) => (
                <option key={id} value={id}>
                  {rgbModeLabel(id)}
                </option>
              ))}
            </select>
          </label>

          {!isRainbow && (
            <Slider
              label="Hue"
              value={state.hue}
              onPreview={(hue) => setState((prev) => (prev ? { ...prev, hue } : prev))}
              onCommit={(hue) =>
                void patch({ hue }, () => client.rgbSetHsv(hue, state.sat, state.val))
              }
            />
          )}
          <Slider
            label="Saturation"
            value={state.sat}
            onPreview={(sat) => setState((prev) => (prev ? { ...prev, sat } : prev))}
            onCommit={(sat) =>
              void patch({ sat }, () => client.rgbSetHsv(state.hue, sat, state.val))
            }
          />
          <Slider
            label="Intensity"
            value={state.val}
            onPreview={(val) => setState((prev) => (prev ? { ...prev, val } : prev))}
            onCommit={(val) =>
              void patch({ val }, () => client.rgbSetHsv(state.hue, state.sat, val))
            }
          />
        </div>

        <div className="flex flex-col items-stretch justify-start gap-2">
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
            Color
          </span>
          <div
            className="aspect-square w-full border border-[#26313c]"
            style={{ backgroundColor: swatch }}
          />
          <span className="border border-[#1b222a] bg-[#020304] px-2 py-1 text-center font-mono text-xs text-slate-500">
            preview
          </span>
        </div>
      </div>

      <div className="mt-5 flex flex-col gap-4 border-t border-[#1b222a] pt-5">
        <Slider
          label="Brightness"
          value={state.brightness}
          onPreview={(brightness) => setState((prev) => (prev ? { ...prev, brightness } : prev))}
          onCommit={(brightness) =>
            void patch({ brightness }, () => client.rgbSetBrightness(brightness))
          }
        />
        <Slider
          label="Speed"
          value={state.speed}
          onPreview={(speed) => setState((prev) => (prev ? { ...prev, speed } : prev))}
          onCommit={(speed) => void patch({ speed }, () => client.rgbSetSpeed(speed))}
        />
        <div className="flex items-center justify-between gap-3">
          <div className="flex flex-col">
            <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
              Status indicators
            </span>
            <span className="text-xs text-slate-400">
              Host, link, battery &amp; layer dots overlaid on the side LEDs.
            </span>
          </div>
          <Toggle
            label="Status indicators"
            checked={state.indicators}
            onChange={(next) =>
              void patch({ indicators: next }, () => client.rgbSetIndicators(next))
            }
          />
        </div>
      </div>

      {zones && (
        <div className="mt-5 flex flex-col gap-4 border-t border-[#1b222a] pt-5">
          <div className="flex flex-col">
            <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
              Zones
            </span>
            <span className="text-xs text-slate-400">
              Light the keys and each side strip together, or give each its own effect.
            </span>
          </div>

          {zones.map((zone) => (
            <ZoneRow
              key={zone.id}
              zone={zone}
              modes={modes}
              zones={zones}
              onPreview={previewZone}
              onPatch={(id, next) => void patchZone(id, next)}
              onResize={(id, start, count) => void patchZoneRange(id, start, count)}
              onSetSync={(id, target) => void patchZoneSync(id, target)}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

interface ZoneRowProps {
  zone: ZoneState;
  modes: number[];
  zones: ZoneState[];
  onPreview: (id: number, next: Partial<ZoneState>) => void;
  onPatch: (id: number, next: Partial<ZoneState>) => void;
  onResize: (id: number, start: number, count: number) => void;
  onSetSync: (id: number, target: number) => void;
}

/**
 * One zone's row: an LED-range readout + numeric resize control, a "Sync to…" control
 * on the side strips (live-mirror another zone's effect in this zone's own range), and,
 * when the zone is not synced, a "Match base" toggle plus —
 * when independent and lit — the effect dropdown, an HSV picker with a device-matched
 * swatch, and brightness/speed. A synced zone shows a note instead of its own effect
 * controls, since its target drives the look.
 */
function ZoneRow({ zone, modes, zones, onPreview, onPatch, onResize, onSetSync }: ZoneRowProps) {
  const linked = zoneLinked(zone);
  const enabled = zoneEnabled(zone);
  const synced = zoneSynced(zone);
  const swatch = rgbToCss(hsvToRgb(zone.hue, zone.sat, zone.val));
  const isRainbow = zone.mode === RGB_MODE_RAINBOW;
  // "Sync to…" mirrors one side strip onto another. The keys zone is
  // the base reference and is not a mirror source (mirroring the base is "Match base"); a
  // target that already syncs to this zone is omitted so the UI never offers a cycle.
  const syncTargets =
    zone.id === 0
      ? []
      : zones.filter((z) => z.id !== zone.id && z.id !== 0 && z.syncTo !== zone.id);
  const canSync = zone.id !== 0 && (synced || syncTargets.length > 0);

  return (
    <div className="flex flex-col gap-4 border border-[#1b222a] p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="font-mono text-xs uppercase tracking-wide text-slate-300">
          {zoneLabel(zone.id)}
        </span>
        <span className="font-mono text-[0.65rem] text-slate-400">
          {zone.count > 0 ? `LEDs ${zone.start}–${zone.start + zone.count - 1}` : 'empty'}
        </span>
      </div>

      <ZoneResize zone={zone} zones={zones} onResize={onResize} />

      {canSync && (
        <label className="flex items-center justify-between gap-2">
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
            Sync to…
          </span>
          <select
            value={synced ? String(zone.syncTo) : ''}
            onChange={(event) =>
              onSetSync(
                zone.id,
                event.target.value === '' ? ZONE_SYNC_NONE : Number(event.target.value),
              )
            }
            className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
          >
            <option value="">None</option>
            {syncTargets.map((z) => (
              <option key={z.id} value={z.id}>
                {zoneLabel(z.id)}
              </option>
            ))}
          </select>
        </label>
      )}

      {synced ? (
        <p className="text-xs text-slate-400">
          Mirrors the {zoneLabel(zone.syncTo)} zone&rsquo;s effect.
        </p>
      ) : (
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between gap-3">
            <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
              Match base
            </span>
            <Toggle
              label={`${zoneLabel(zone.id)} match base`}
              checked={linked}
              onChange={(next) =>
                onPatch(zone.id, { flags: withZoneFlag(zone.flags, ZONE_FLAG_LINKED, next) })
              }
            />
          </div>

          {!linked && (
            <div className="flex flex-col gap-4">
              <div className="flex items-center justify-between">
                <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                  Lit
                </span>
                <Toggle
                  label={`${zoneLabel(zone.id)} lit`}
                  checked={enabled}
                  onChange={(next) =>
                    onPatch(zone.id, { flags: withZoneFlag(zone.flags, ZONE_FLAG_ENABLED, next) })
                  }
                />
              </div>

              {enabled && (
                <>
                  <div className="grid gap-5 sm:grid-cols-[minmax(0,1fr)_9rem]">
                    <div className="flex flex-col gap-4">
                      <label className="flex flex-col gap-1.5">
                        <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                          Effect
                        </span>
                        <select
                          value={zone.mode}
                          onChange={(event) =>
                            onPatch(zone.id, { mode: Number(event.target.value) })
                          }
                          className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
                        >
                          {modes.map((id) => (
                            <option key={id} value={id}>
                              {rgbModeLabel(id)}
                            </option>
                          ))}
                        </select>
                      </label>

                      {!isRainbow && (
                        <Slider
                          label="Hue"
                          value={zone.hue}
                          onPreview={(hue) => onPreview(zone.id, { hue })}
                          onCommit={(hue) => onPatch(zone.id, { hue })}
                        />
                      )}
                      <Slider
                        label="Saturation"
                        value={zone.sat}
                        onPreview={(sat) => onPreview(zone.id, { sat })}
                        onCommit={(sat) => onPatch(zone.id, { sat })}
                      />
                      <Slider
                        label="Intensity"
                        value={zone.val}
                        onPreview={(val) => onPreview(zone.id, { val })}
                        onCommit={(val) => onPatch(zone.id, { val })}
                      />
                    </div>

                    <div className="flex flex-col items-stretch justify-start gap-2">
                      <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                        Color
                      </span>
                      <div
                        className="aspect-square w-full border border-[#26313c]"
                        style={{ backgroundColor: swatch }}
                      />
                      <span className="border border-[#1b222a] bg-[#020304] px-2 py-1 text-center font-mono text-xs text-slate-500">
                        preview
                      </span>
                    </div>
                  </div>

                  <Slider
                    label="Brightness"
                    value={zone.brightness}
                    onPreview={(brightness) => onPreview(zone.id, { brightness })}
                    onCommit={(brightness) => onPatch(zone.id, { brightness })}
                  />
                  <Slider
                    label="Speed"
                    value={zone.speed}
                    onPreview={(speed) => onPreview(zone.id, { speed })}
                    onCommit={(speed) => onPatch(zone.id, { speed })}
                  />
                </>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

interface ZoneResizeProps {
  zone: ZoneState;
  zones: ZoneState[];
  onResize: (id: number, start: number, count: number) => void;
}

/**
 * A modest numeric resize control for one zone: start + count number inputs and an
 * Apply button, gated by client-side validation that mirrors the firmware (the range
 * must end by [`LED_COUNT`] and not overlap another lit zone). The firmware re-checks
 * authoritatively across the whole table, so this is a fast-fail convenience, not the
 * authority. The inputs re-seed from the committed zone, so a rejected write that
 * reverts is reflected back.
 */
function ZoneResize({ zone, zones, onResize }: ZoneResizeProps) {
  const [start, setStart] = useState(zone.start);
  const [count, setCount] = useState(zone.count);

  useEffect(() => {
    setStart(zone.start);
    setCount(zone.count);
  }, [zone.start, zone.count]);

  const error = rangeError(zones, zone.id, start, count);
  const changed = start !== zone.start || count !== zone.count;

  return (
    <div className="flex flex-col gap-1.5">
      <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">Range</span>
      <div className="flex items-end gap-2">
        <NumberField label="Start" value={start} onChange={setStart} />
        <NumberField label="Count" value={count} onChange={setCount} />
        <button
          type="button"
          disabled={!changed || error !== null}
          onClick={() => onResize(zone.id, start, count)}
          className="inline-flex min-h-8 items-center border border-[#26313c] bg-[#020304] px-3 py-1.5 font-mono text-xs font-semibold uppercase text-slate-300 transition-colors enabled:hover:border-sky-400 disabled:cursor-not-allowed disabled:text-slate-600"
        >
          Apply
        </button>
      </div>
      {error && <span className="text-xs text-red-200">{error}</span>}
    </div>
  );
}

/** Validate a proposed zone range against the chain bound and the other lit zones
 * (mirrors the firmware `set_zone_range` + `zone_range_overlaps`); `null` = valid. */
function rangeError(zones: ZoneState[], id: number, start: number, count: number): string | null {
  if (start < 0 || count < 0) {
    return 'Start and count must be ≥ 0.';
  }
  if (start + count > LED_COUNT) {
    return `Range must end by LED ${LED_COUNT}.`;
  }
  if (zoneRangeOverlaps(zones, id, start, count)) {
    return 'Overlaps another lit zone.';
  }
  return null;
}

interface NumberFieldProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
}

/** A small labelled integer input (0..[`LED_COUNT`]) for the zone resize control. */
function NumberField({ label, value, onChange }: NumberFieldProps) {
  return (
    <label className="flex flex-1 flex-col gap-1">
      <span className="font-mono text-[0.6rem] uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <input
        type="number"
        min={0}
        max={LED_COUNT}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="w-full border border-[#26313c] bg-[#020304] px-2 py-1.5 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
      />
    </label>
  );
}

interface ToggleProps {
  /** Accessible name describing what the switch controls (it shows only "On"/"Off"). */
  label: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}

/** A compact On/Off switch shared by the RGB enable and status-indicator controls. */
function Toggle({ label, checked, onChange }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={[
        'inline-flex min-h-8 items-center gap-2 border px-3 py-1.5 font-mono text-xs font-semibold uppercase transition-colors',
        checked
          ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-200'
          : 'border-[#26313c] bg-[#020304] text-slate-400',
      ].join(' ')}
    >
      <span className={['h-1.5 w-1.5', checked ? 'bg-emerald-400' : 'bg-slate-600'].join(' ')} />
      {checked ? 'On' : 'Off'}
    </button>
  );
}

interface SliderProps {
  label: string;
  value: number;
  onPreview: (value: number) => void;
  onCommit: (value: number) => void;
}

function Slider({ label, value, onPreview, onCommit }: SliderProps) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="flex items-baseline justify-between font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {label}
        <span className="font-mono text-slate-400">{value}</span>
      </span>
      <input
        type="range"
        min={0}
        max={255}
        value={value}
        onChange={(event) => onPreview(Number(event.target.value))}
        onPointerUp={(event) => onCommit(Number(event.currentTarget.value))}
        onKeyUp={(event) => {
          if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
            onCommit(Number(event.currentTarget.value));
          }
        }}
        onBlur={(event) => onCommit(Number(event.currentTarget.value))}
        className="h-3 w-full cursor-pointer appearance-none bg-transparent accent-sky-400"
      />
    </label>
  );
}
