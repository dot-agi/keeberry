// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState, type ReactNode } from 'react';
import {
  DebounceAlgorithm,
  type DebounceConfig,
  type KcpClient,
  type LayerConfig,
  type TuningConfig,
} from '../kcp';
import { ErrorBanner, InfoHint, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface TuningPanelProps {
  client: KcpClient;
}

/** Practical millisecond bounds for the UI; the firmware accepts any non-zero value. */
const AUTO_SHIFT_MIN = 50;
const AUTO_SHIFT_MAX = 1000;
const LEADER_MIN = 100;
const LEADER_MAX = 2000;
/** Tap-hold decision term bounds (the firmware accepts any non-zero value). */
const TAP_HOLD_MIN = 50;
const TAP_HOLD_MAX = 1000;
/** Quick-tap window bounds; `0` disables quick-tap, so the minimum is zero. */
const QUICK_TAP_MIN = 0;
const QUICK_TAP_MAX = 1000;
/** Practical debounce-interval bounds for the UI (the firmware accepts any `>= 1`). */
const DEBOUNCE_INTERVAL_MIN = 1;
const DEBOUNCE_INTERVAL_MAX = 50;

/** Clamp a typed value into `[min, max]`, mapping empty / NaN to `min`. */
function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Math.round(value) || min));
}

/**
 * The layer config to send when enabling tri-layer. The firmware rejects `l1 == l2` (and
 * any out-of-range layer), so the 0,0,0 default would fail the moment the toggle is flipped
 * on — a chicken-and-egg with the selectors that only appear once enabled. Keep the user's
 * trigger layers when they are already valid; otherwise seed two distinct in-range layers
 * (0 and 1) and a target clamped to `layerCount`. The toggle is gated to `layerCount >= 2`,
 * so layers 0 and 1 are always valid here.
 */
function triLayerOnEnable(cfg: LayerConfig, layerCount: number): LayerConfig {
  const inRange = (n: number) => n < layerCount;
  const alreadyValid =
    cfg.triL1 !== cfg.triL2 && inRange(cfg.triL1) && inRange(cfg.triL2) && inRange(cfg.triL3);
  if (alreadyValid) return { ...cfg, triEnabled: true };
  const triL3 = layerCount > 2 ? 2 : 1;
  return { ...cfg, triEnabled: true, triL1: 0, triL2: 1, triL3 };
}

/**
 * Runtime-tuning panel (the CONFIG and BEHAVIOR live tunables): the
 * auto-shift feature (on/off + hold timeout), the leader-sequence timeout and the
 * matrix debounce algorithm/interval. Every control reflects its `GET` on load and
 * writes live (optimistic, reverting the control on failure); the firmware persists
 * them into the config blob on the next `CONFIG.SAVE`, exactly like the keymap and
 * behaviour tables.
 */
export function TuningPanel({ client }: TuningPanelProps) {
  const [tuning, setTuning] = useState<TuningConfig | null>(null);
  const [debounce, setDebounce] = useState<DebounceConfig | null>(null);
  const [layerCfg, setLayerCfg] = useState<LayerConfig | null>(null);
  const [layerCount, setLayerCount] = useState(1);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [tune, deb, layers, count] = await Promise.all([
          client.getTuning(),
          client.getDebounce(),
          client.getLayerConfig(),
          client.getLayerCount(),
        ]);
        if (!cancelled) {
          setTuning(tune);
          setDebounce(deb);
          setLayerCfg(layers);
          setLayerCount(count);
        }
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  /** Apply a tunables change live, reverting the controls if the write fails. */
  async function applyTuning(next: TuningConfig) {
    const prev = tuning;
    setTuning(next);
    setError(null);
    try {
      await client.setTuning(next);
    } catch (err) {
      setTuning(prev);
      setError(friendlyPanelError(err));
    }
  }

  /** Apply a debounce change live, reverting the controls if the write fails. */
  async function applyDebounce(next: DebounceConfig) {
    const prev = debounce;
    setDebounce(next);
    setError(null);
    try {
      await client.setDebounce(next);
    } catch (err) {
      setDebounce(prev);
      setError(friendlyPanelError(err));
    }
  }

  /** Apply a layer-config change live, reverting the controls if the write fails. */
  async function applyLayerConfig(next: LayerConfig) {
    const prev = layerCfg;
    setLayerCfg(next);
    setError(null);
    try {
      await client.setLayerConfig(next);
    } catch (err) {
      setLayerCfg(prev);
      setError(friendlyPanelError(err));
    }
  }

  return (
    <Panel
      title="Tuning"
      description="Layers, auto-shift, the leader key, the tap-hold tapping term and key debounce all apply immediately — no reflash. Save settings to keep them after a restart."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {layerCfg && (
        <Section
          title="Layers"
          hint="The default layer is the base every transparent key falls through to — set it here, or with a DF key on the keymap. Tri-layer auto-activates a target layer whenever its two trigger layers are both active (the classic 'adjust' layer); the two triggers must differ."
        >
          <div className="grid gap-4 sm:grid-cols-[10rem_minmax(0,1fr)]">
            <LayerSelect
              label="Default layer"
              value={layerCfg.defaultLayer}
              count={layerCount}
              onChange={(defaultLayer) => void applyLayerConfig({ ...layerCfg, defaultLayer })}
            />
          </div>
          {/* Tri-layer needs two distinct trigger layers, which a single-layer keymap
              cannot provide (the firmware rejects l1 == l2), so hide the control there. */}
          {layerCount >= 2 && (
            <div className="mt-4 flex flex-wrap items-end gap-4">
              <Toggle
                label="Tri-layer"
                on={layerCfg.triEnabled}
                onChange={(on) =>
                  void applyLayerConfig(
                    on ? triLayerOnEnable(layerCfg, layerCount) : { ...layerCfg, triEnabled: false },
                  )
                }
              />
              {layerCfg.triEnabled && (
                <>
                  <LayerSelect
                    label="When layer"
                    value={layerCfg.triL1}
                    count={layerCount}
                    onChange={(triL1) => void applyLayerConfig({ ...layerCfg, triL1 })}
                  />
                  <LayerSelect
                    label="and layer"
                    value={layerCfg.triL2}
                    count={layerCount}
                    onChange={(triL2) => void applyLayerConfig({ ...layerCfg, triL2 })}
                  />
                  <LayerSelect
                    label="activate layer"
                    value={layerCfg.triL3}
                    count={layerCount}
                    onChange={(triL3) => void applyLayerConfig({ ...layerCfg, triL3 })}
                  />
                </>
              )}
            </div>
          )}
        </Section>
      )}

      {tuning ? (
        <>
          <Section
            title="Auto-shift"
            hint="Holding a key past the timeout sends its shifted form; a quick tap sends the plain key. Affects letters, numbers and symbols only."
          >
            <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_8rem]">
              <Toggle
                label="Auto-shift"
                on={tuning.autoShiftEnabled}
                onChange={(on) => void applyTuning({ ...tuning, autoShiftEnabled: on })}
              />
              <NumberField
                label="Timeout (ms)"
                value={tuning.autoShiftTimeoutMs}
                min={AUTO_SHIFT_MIN}
                max={AUTO_SHIFT_MAX}
                onCommit={(ms) =>
                  void applyTuning({
                    ...tuning,
                    autoShiftTimeoutMs: clamp(ms, AUTO_SHIFT_MIN, AUTO_SHIFT_MAX),
                  })
                }
              />
            </div>
          </Section>

          <Section
            title="Leader key"
            hint="After the leader key, each captured key restarts this timeout; the sequence ends when it elapses. Configure sequences as macros from the host."
          >
            <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_8rem]">
              <p className="self-center text-xs text-slate-400">
                Time allowed between leader-sequence keys.
              </p>
              <NumberField
                label="Timeout (ms)"
                value={tuning.leaderTimeoutMs}
                min={LEADER_MIN}
                max={LEADER_MAX}
                onCommit={(ms) =>
                  void applyTuning({
                    ...tuning,
                    leaderTimeoutMs: clamp(ms, LEADER_MIN, LEADER_MAX),
                  })
                }
              />
            </div>
          </Section>

          <Section
            title="Mod-tap & layer-tap"
            hint="Dual-role keys (MT/LT): a tap sends the key, a hold applies the modifier (MT) or layer (LT). Held past the term resolves to a hold. Hold-on-other-press is the safest flavour for layer-taps; permissive hold resolves to a hold on a nested press-and-release; retro tapping emits the tap when a lone hold is released; chordal hold (bilateral) keeps a same-hand roll a tap, so only an opposite-hand press can trigger the hold. Quick-tap repeats the tap when you re-press within its window (0 = off)."
          >
            <div className="grid gap-4 sm:grid-cols-[8rem_8rem]">
              <NumberField
                label="Tapping term (ms)"
                value={tuning.tapHoldTermMs}
                min={TAP_HOLD_MIN}
                max={TAP_HOLD_MAX}
                onCommit={(ms) =>
                  void applyTuning({
                    ...tuning,
                    tapHoldTermMs: clamp(ms, TAP_HOLD_MIN, TAP_HOLD_MAX),
                  })
                }
              />
              <NumberField
                label="Quick-tap (ms)"
                value={tuning.quickTapTermMs}
                min={QUICK_TAP_MIN}
                max={QUICK_TAP_MAX}
                onCommit={(ms) =>
                  void applyTuning({
                    ...tuning,
                    quickTapTermMs: clamp(ms, QUICK_TAP_MIN, QUICK_TAP_MAX),
                  })
                }
              />
            </div>
            <div className="mt-4 grid gap-4 sm:grid-cols-2">
              <Toggle
                label="Hold on other press"
                on={tuning.holdOnOtherKeyPress}
                onChange={(on) => void applyTuning({ ...tuning, holdOnOtherKeyPress: on })}
              />
              <Toggle
                label="Permissive hold"
                on={tuning.permissiveHold}
                onChange={(on) => void applyTuning({ ...tuning, permissiveHold: on })}
              />
              <Toggle
                label="Retro tapping"
                on={tuning.retroTapping}
                onChange={(on) => void applyTuning({ ...tuning, retroTapping: on })}
              />
              <Toggle
                label="Chordal hold (bilateral)"
                on={tuning.chordalHold}
                onChange={(on) => void applyTuning({ ...tuning, chordalHold: on })}
              />
            </div>
          </Section>
        </>
      ) : (
        !error && <p className="text-sm text-slate-400">Reading tuning…</p>
      )}

      {debounce && (
        <Section
          title="Key debounce"
          hint="Standard defers every edge for the interval, rejecting contact chatter on both press and release. Gaming reports a press on the first scan that sees it and only debounces the release — snappier, still chatter-free. The interval is how long a change must hold, in milliseconds."
        >
          <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_8rem]">
            <label className="flex flex-col gap-1.5">
              <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                Algorithm
              </span>
              <select
                value={debounce.algorithm}
                onChange={(event) =>
                  void applyDebounce({ ...debounce, algorithm: Number(event.target.value) })
                }
                className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
              >
                <option value={DebounceAlgorithm.SymmetricDefer}>Standard (noise-resistant)</option>
                <option value={DebounceAlgorithm.AsymmetricEager}>Gaming (eager press)</option>
              </select>
            </label>
            <NumberField
              label="Interval (ms)"
              value={debounce.interval}
              min={DEBOUNCE_INTERVAL_MIN}
              max={DEBOUNCE_INTERVAL_MAX}
              onCommit={(interval) =>
                void applyDebounce({
                  ...debounce,
                  interval: clamp(interval, DEBOUNCE_INTERVAL_MIN, DEBOUNCE_INTERVAL_MAX),
                })
              }
            />
          </div>
        </Section>
      )}
    </Panel>
  );
}

interface SectionProps {
  title: string;
  hint: string;
  children: ReactNode;
}

function Section({ title, hint, children }: SectionProps) {
  return (
    <div className="mb-5 border-t border-[#1b222a] pt-5 first:mt-0 first:border-t-0 first:pt-0">
      <div className="mb-3 flex items-center gap-2">
        <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
          {title}
        </span>
        <InfoHint label={`${title} details`}>{hint}</InfoHint>
      </div>
      {children}
    </div>
  );
}

interface LayerSelectProps {
  label: string;
  value: number;
  count: number;
  onChange: (layer: number) => void;
}

/** A labelled layer chooser (`0..count-1`), for the default layer and tri-layer rule. */
function LayerSelect({ label, value, count, onChange }: LayerSelectProps) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <select
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
      >
        {Array.from({ length: count }, (_, n) => (
          <option key={n} value={n}>
            Layer {n}
          </option>
        ))}
      </select>
    </label>
  );
}

interface ToggleProps {
  label: string;
  on: boolean;
  onChange: (on: boolean) => void;
}

/** A two-button on/off segmented control, matching the rollover panel's style. */
function Toggle({ label, on, onChange }: ToggleProps) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <div className="inline-grid w-fit grid-cols-2 border border-[#26313c] bg-[#020304] p-1">
        {[
          { value: false, text: 'Off' },
          { value: true, text: 'On' },
        ].map(({ value, text }) => (
          <button
            key={text}
            type="button"
            aria-pressed={on === value}
            onClick={() => onChange(value)}
            className={[
              'min-h-8 px-4 py-1.5 font-mono text-xs font-semibold uppercase transition-colors',
              on === value ? 'bg-sky-500/20 text-sky-100' : 'text-slate-400 hover:bg-[#111820]',
            ].join(' ')}
          >
            {text}
          </button>
        ))}
      </div>
    </label>
  );
}

interface NumberFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  onCommit: (value: number) => void;
}

/**
 * A labelled numeric input that commits on blur / Enter rather than on every
 * keystroke, so a value can be typed without each intermediate digit firing a
 * (clamped) live write.
 */
function NumberField({ label, value, min, max, onCommit }: NumberFieldProps) {
  const [draft, setDraft] = useState(String(value));

  // Re-sync the draft when the committed value changes (e.g. a reverted write).
  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  return (
    <label className="flex flex-col gap-1.5">
      <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <input
        type="number"
        min={min}
        max={max}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => onCommit(Number(draft))}
        onKeyDown={(event) => {
          if (event.key === 'Enter') event.currentTarget.blur();
        }}
        className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
      />
    </label>
  );
}
