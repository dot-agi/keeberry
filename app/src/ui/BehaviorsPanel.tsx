// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useState, type ReactNode } from 'react';
import {
  NONE,
  MIN_COMBO_KEYS,
  MODIFIER_BITS,
  SOCD_MODES,
  SocdMode,
  formatModifiers,
  fromUsage,
  type BehaviorInfo,
  type Combo,
  type KcpClient,
  type KeyOverride,
  type SocdPair,
  type TapDance,
  type TimedInfo,
} from '../kcp';
import { ErrorBanner, Panel, Tooltip } from './Panel';
import { friendlyPanelError } from './panelError';
import { KeycodePicker } from './KeycodePicker';
import { keyLabel } from './keyDisplay';

interface BehaviorsPanelProps {
  client: KcpClient;
  /** Keymap layer count, for the override layer-mask toggles and `MO(n)` picks. */
  layerCount: number;
}

/** Which keycode field a popped {@link KeycodePicker} is editing. */
type PickTarget =
  | { kind: 'socd'; index: number; field: 'a' | 'b' }
  | { kind: 'override'; index: number; field: 'trigger' | 'replacement' }
  | { kind: 'tapdance'; index: number; field: 'tap' | 'hold' | 'double' }
  | { kind: 'combo'; index: number }
  | { kind: 'comboKey'; index: number; keyIndex: number };

/** Firmware default tap-dance decision window (`timed::DEFAULT_TAP_TERM_MS`). */
const DEFAULT_TAP_TERM_MS = 200;
/** Firmware default combo recognition window (`timed::DEFAULT_COMBO_TERM_MS`). */
const DEFAULT_COMBO_TERM_MS = 50;

const DEFAULT_SOCD: SocdPair = { a: NONE, b: NONE, mode: SocdMode.LastWins };
const DEFAULT_OVERRIDE: KeyOverride = {
  trigger: NONE,
  triggerMods: 0,
  replacement: NONE,
  replacementMods: 0,
  layerMask: 0x0001,
  enabled: true,
};
const DEFAULT_TAPDANCE: TapDance = {
  tap: NONE,
  hold: NONE,
  double: NONE,
  termMs: DEFAULT_TAP_TERM_MS,
};
// Seed two distinct member keys (A, B): the firmware rejects a combo whose members are not
// all distinct, so the old `[NONE, NONE]` default made every "Add combo" fail instantly.
const DEFAULT_COMBO: Combo = {
  keys: [fromUsage(0x04), fromUsage(0x05)],
  action: NONE,
  termMs: DEFAULT_COMBO_TERM_MS,
  mustHold: false,
  mustTap: false,
  inOrder: false,
};

/**
 * Behaviour table editors for the BEHAVIOR group: SOCD cleanup and key overrides
 * (bounded by BEHAVIOR_INFO) plus the timed-engine tap-dance and combo tables
 * (bounded by TIMED_INFO). Every capacity is read live and each edit is written
 * through its SET op so it applies on the next scan. Keys are chosen with the
 * shared {@link KeycodePicker}.
 */
export function BehaviorsPanel({ client, layerCount }: BehaviorsPanelProps) {
  const [info, setInfo] = useState<BehaviorInfo | null>(null);
  const [timed, setTimed] = useState<TimedInfo | null>(null);
  const [socd, setSocd] = useState<(SocdPair | null)[]>([]);
  const [overrides, setOverrides] = useState<(KeyOverride | null)[]>([]);
  const [tapdance, setTapdance] = useState<(TapDance | null)[]>([]);
  const [combos, setCombos] = useState<(Combo | null)[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pick, setPick] = useState<PickTarget | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [bi, ti] = await Promise.all([client.behaviorInfo(), client.timedInfo()]);
        const [socdSlots, overrideSlots, tapdanceSlots, comboSlots] = await Promise.all([
          Promise.all(Array.from({ length: bi.maxSocd }, (_, i) => client.socdGet(i))),
          Promise.all(Array.from({ length: bi.maxOverrides }, (_, i) => client.overrideGet(i))),
          Promise.all(Array.from({ length: ti.maxTapDance }, (_, i) => client.tapdanceGet(i))),
          Promise.all(Array.from({ length: ti.maxCombo }, (_, i) => client.comboGet(i))),
        ]);
        if (cancelled) return;
        setInfo(bi);
        setTimed(ti);
        setSocd(socdSlots);
        setOverrides(overrideSlots);
        setTapdance(tapdanceSlots);
        setCombos(comboSlots);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  const guard = useCallback(async (run: () => Promise<void>) => {
    setError(null);
    try {
      await run();
    } catch (err) {
      setError(friendlyPanelError(err));
    }
  }, []);

  // --- SOCD ---------------------------------------------------------------

  const writeSocd = (index: number, pair: SocdPair) =>
    guard(async () => {
      await client.socdSet(index, pair);
      setSocd((prev) => replaceAt(prev, index, pair));
    });

  const addSocd = () => {
    const index = socd.findIndex((slot) => slot === null);
    if (index >= 0) void writeSocd(index, { ...DEFAULT_SOCD });
  };

  const removeSocd = (index: number) =>
    guard(async () => {
      await client.socdClear(index);
      setSocd((prev) => replaceAt(prev, index, null));
    });

  // --- Overrides ----------------------------------------------------------

  const writeOverride = (index: number, ov: KeyOverride) =>
    guard(async () => {
      await client.overrideSet(index, ov);
      setOverrides((prev) => replaceAt(prev, index, ov));
    });

  const addOverride = () => {
    const index = overrides.findIndex((slot) => slot === null);
    if (index >= 0) void writeOverride(index, { ...DEFAULT_OVERRIDE });
  };

  const removeOverride = (index: number) =>
    guard(async () => {
      await client.overrideClear(index);
      setOverrides((prev) => replaceAt(prev, index, null));
    });

  // --- Tap-dance ----------------------------------------------------------

  const writeTapdance = (index: number, td: TapDance) =>
    guard(async () => {
      await client.tapdanceSet(index, td);
      setTapdance((prev) => replaceAt(prev, index, td));
    });

  const addTapdance = () => {
    const index = tapdance.findIndex((slot) => slot === null);
    if (index >= 0) void writeTapdance(index, { ...DEFAULT_TAPDANCE });
  };

  const removeTapdance = (index: number) =>
    guard(async () => {
      await client.tapdanceClear(index);
      setTapdance((prev) => replaceAt(prev, index, null));
    });

  // --- Combos -------------------------------------------------------------

  const writeCombo = (index: number, combo: Combo) =>
    guard(async () => {
      await client.comboSet(index, combo);
      setCombos((prev) => replaceAt(prev, index, combo));
    });

  const addCombo = () => {
    const index = combos.findIndex((slot) => slot === null);
    if (index >= 0) void writeCombo(index, { ...DEFAULT_COMBO, keys: [...DEFAULT_COMBO.keys] });
  };

  const removeCombo = (index: number) =>
    guard(async () => {
      await client.comboClear(index);
      setCombos((prev) => replaceAt(prev, index, null));
    });

  // --- Keycode picker -----------------------------------------------------

  function handlePick(raw: number) {
    const target = pick;
    setPick(null);
    if (!target) return;
    if (target.kind === 'socd') {
      const current = socd[target.index];
      if (current) void writeSocd(target.index, { ...current, [target.field]: raw });
    } else if (target.kind === 'override') {
      const current = overrides[target.index];
      if (current) void writeOverride(target.index, { ...current, [target.field]: raw });
    } else if (target.kind === 'tapdance') {
      const current = tapdance[target.index];
      if (current) void writeTapdance(target.index, { ...current, [target.field]: raw });
    } else if (target.kind === 'combo') {
      const current = combos[target.index];
      if (current) void writeCombo(target.index, { ...current, action: raw });
    } else {
      const current = combos[target.index];
      if (current) {
        const keys = current.keys.slice();
        keys[target.keyIndex] = raw;
        void writeCombo(target.index, { ...current, keys });
      }
    }
  }

  const pickerRaw = currentPickRaw(pick, { socd, overrides, tapdance, combos });

  const socdCount = socd.filter((s) => s !== null).length;
  const overrideCount = overrides.filter((o) => o !== null).length;
  const tapdanceCount = tapdance.filter((t) => t !== null).length;
  const comboCount = combos.filter((c) => c !== null).length;

  return (
    <Panel
      title="Behaviors"
      description="Conflict cleanup, key overrides, tap-dance, and combo rules apply immediately."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {!info || !timed ? (
        !error && <p className="text-sm text-slate-400">Reading behaviour tables…</p>
      ) : (
        <div className="flex flex-col gap-8">
          <Subsection
            title="Conflict cleanup"
            count={socdCount}
            max={info.maxSocd}
            addLabel="Add pair"
            onAdd={addSocd}
          >
            {socdCount === 0 ? (
              <EmptyHint>None</EmptyHint>
            ) : (
              <ul className="flex flex-col gap-2">
                {socd.map((pair, index) =>
                  pair ? (
                    <li
                      key={index}
                      className="flex flex-wrap items-center gap-2 border border-[#1b222a] bg-[#020304] p-2"
                    >
                      <KeyButton
                        label={keyLabel(pair.a)}
                        onClick={() => setPick({ kind: 'socd', index, field: 'a' })}
                      />
                      <span className="font-mono text-xs text-slate-500">vs</span>
                      <KeyButton
                        label={keyLabel(pair.b)}
                        onClick={() => setPick({ kind: 'socd', index, field: 'b' })}
                      />
                      <select
                        value={pair.mode}
                        aria-label={`Conflict cleanup mode for pair ${index + 1}`}
                        onChange={(event) =>
                          void writeSocd(index, { ...pair, mode: Number(event.target.value) })
                        }
                        className="border border-[#26313c] bg-[#020304] px-2 py-1.5 text-xs text-slate-100 focus:border-sky-400 focus:outline-none"
                      >
                        {SOCD_MODES.map((m) => (
                          <option key={m.mode} value={m.mode}>
                            {m.label}
                          </option>
                        ))}
                      </select>
                      <RemoveButton onClick={() => void removeSocd(index)} />
                    </li>
                  ) : null,
                )}
              </ul>
            )}
          </Subsection>

          {/* Overrides -------------------------------------------------- */}
          <Subsection
            title="Key overrides"
            count={overrideCount}
            max={info.maxOverrides}
            addLabel="Add override"
            onAdd={addOverride}
            bordered
          >
            {overrideCount === 0 ? (
              <EmptyHint>None</EmptyHint>
            ) : (
              <ul className="flex flex-col gap-3">
                {overrides.map((ov, index) =>
                  ov ? (
                    <OverrideRow
                      key={index}
                      override={ov}
                      layerCount={layerCount}
                      onPickTrigger={() => setPick({ kind: 'override', index, field: 'trigger' })}
                      onPickReplacement={() =>
                        setPick({ kind: 'override', index, field: 'replacement' })
                      }
                      onChange={(next) => void writeOverride(index, next)}
                      onRemove={() => void removeOverride(index)}
                    />
                  ) : null,
                )}
              </ul>
            )}
          </Subsection>

          {/* Tap-dance -------------------------------------------------- */}
          <Subsection
            title="Tap-dance"
            count={tapdanceCount}
            max={timed.maxTapDance}
            addLabel="Add tap-dance"
            onAdd={addTapdance}
            bordered
          >
            {tapdanceCount === 0 ? (
              <EmptyHint>None</EmptyHint>
            ) : (
              <ul className="flex flex-col gap-3">
                {tapdance.map((td, index) =>
                  td ? (
                    <TapDanceRow
                      key={index}
                      entry={td}
                      onPick={(field) => setPick({ kind: 'tapdance', index, field })}
                      onChange={(next) => void writeTapdance(index, next)}
                      onRemove={() => void removeTapdance(index)}
                    />
                  ) : null,
                )}
              </ul>
            )}
          </Subsection>

          {/* Combos ----------------------------------------------------- */}
          <Subsection
            title="Combos"
            count={comboCount}
            max={timed.maxCombo}
            addLabel="Add combo"
            onAdd={addCombo}
            bordered
          >
            {comboCount === 0 ? (
              <EmptyHint>
                None / {MIN_COMBO_KEYS}-{timed.maxComboKeys} keys
              </EmptyHint>
            ) : (
              <ul className="flex flex-col gap-3">
                {combos.map((combo, index) =>
                  combo ? (
                    <ComboRow
                      key={index}
                      combo={combo}
                      maxKeys={timed.maxComboKeys}
                      onPickKey={(keyIndex) => setPick({ kind: 'comboKey', index, keyIndex })}
                      onPickAction={() => setPick({ kind: 'combo', index })}
                      onChange={(next) => void writeCombo(index, next)}
                      onRemove={() => void removeCombo(index)}
                    />
                  ) : null,
                )}
              </ul>
            )}
          </Subsection>
        </div>
      )}

      {pick && (
        <KeycodePicker
          context="basic"
          layerCount={layerCount}
          currentRaw={pickerRaw}
          title={pickerTitle(pick)}
          onPick={handlePick}
          onClose={() => setPick(null)}
        />
      )}
    </Panel>
  );
}

/** Replace one slot of a table immutably, returning a new array. */
function replaceAt<T>(table: T[], index: number, value: T): T[] {
  const next = table.slice();
  next[index] = value;
  return next;
}

interface PickTables {
  socd: (SocdPair | null)[];
  overrides: (KeyOverride | null)[];
  tapdance: (TapDance | null)[];
  combos: (Combo | null)[];
}

/** The raw keycode the picker should highlight for the active pick target. */
function currentPickRaw(pick: PickTarget | null, tables: PickTables): number {
  if (!pick) return NONE;
  switch (pick.kind) {
    case 'socd':
      return tables.socd[pick.index]?.[pick.field] ?? NONE;
    case 'override':
      return tables.overrides[pick.index]?.[pick.field] ?? NONE;
    case 'tapdance':
      return tables.tapdance[pick.index]?.[pick.field] ?? NONE;
    case 'combo':
      return tables.combos[pick.index]?.action ?? NONE;
    case 'comboKey':
      return tables.combos[pick.index]?.keys[pick.keyIndex] ?? NONE;
  }
}

function pickerTitle(target: PickTarget): string {
  switch (target.kind) {
    case 'socd':
      return `Conflict pair · key ${target.field.toUpperCase()}`;
    case 'override':
      return `Override · ${target.field}`;
    case 'tapdance':
      return `Tap-dance · ${target.field}`;
    case 'combo':
      return 'Combo · action';
    case 'comboKey':
      return `Combo · key ${target.keyIndex + 1}`;
  }
}

/** Clamp a term to the u16 wire range, treating a blank/NaN input as 0. */
function clampTerm(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(65535, Math.floor(value)));
}

interface SubsectionProps {
  title: string;
  count: number;
  max: number;
  addLabel: string;
  onAdd: () => void;
  bordered?: boolean;
  children: ReactNode;
}

/** A titled table sub-section with a `used / max` counter and an Add button. */
function Subsection({ title, count, max, addLabel, onAdd, bordered, children }: SubsectionProps) {
  return (
    <div className={bordered ? 'border-t border-[#1b222a] pt-6' : undefined}>
      <div className="mb-3 flex items-center justify-between">
        <h3 className="font-mono text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">
          {title}
          <span className="ml-2 font-mono text-slate-400">
            {count} / {max}
          </span>
        </h3>
        <button
          type="button"
          onClick={onAdd}
          disabled={count >= max}
          className="kb-control kb-control-sm text-xs disabled:cursor-not-allowed disabled:opacity-40"
        >
          {addLabel}
        </button>
      </div>
      {children}
    </div>
  );
}

function EmptyHint({ children }: { children: ReactNode }) {
  return <p className="text-xs text-slate-400">{children}</p>;
}

interface OverrideRowProps {
  override: KeyOverride;
  layerCount: number;
  onPickTrigger: () => void;
  onPickReplacement: () => void;
  onChange: (next: KeyOverride) => void;
  onRemove: () => void;
}

function OverrideRow({
  override: ov,
  layerCount,
  onPickTrigger,
  onPickReplacement,
  onChange,
  onRemove,
}: OverrideRowProps) {
  return (
    <li className="flex flex-col gap-3 border border-[#1b222a] bg-[#020304] p-3">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex flex-col gap-1">
          <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Trigger</span>
          <div className="flex items-center gap-1.5">
            <KeyButton label={keyLabel(ov.trigger)} onClick={onPickTrigger} />
            <ModBits
              value={ov.triggerMods}
              onChange={(triggerMods) => onChange({ ...ov, triggerMods })}
            />
          </div>
        </div>

        <span className="mt-4 font-mono text-slate-500">-&gt;</span>

        <div className="flex flex-col gap-1">
          <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Replacement</span>
          <div className="flex items-center gap-1.5">
            <KeyButton label={keyLabel(ov.replacement)} onClick={onPickReplacement} />
            <ModBits
              value={ov.replacementMods}
              onChange={(replacementMods) => onChange({ ...ov, replacementMods })}
            />
          </div>
        </div>

        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            role="switch"
            aria-checked={ov.enabled}
            aria-label={`Enable override ${keyLabel(ov.trigger)} to ${keyLabel(ov.replacement)}`}
            onClick={() => onChange({ ...ov, enabled: !ov.enabled })}
            className={[
              'min-h-8 border px-2.5 py-1 font-mono text-xs font-medium uppercase transition-colors',
              ov.enabled
                ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-200'
                : 'border-[#26313c] bg-[#020304] text-slate-400',
            ].join(' ')}
          >
            {ov.enabled ? 'On' : 'Off'}
          </button>
          <RemoveButton onClick={onRemove} />
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Layers</span>
        {Array.from({ length: layerCount }, (_, n) => {
          const on = (ov.layerMask & (1 << n)) !== 0;
          return (
            <button
              key={n}
              type="button"
              aria-label={`Layer ${n}`}
              aria-pressed={on}
              onClick={() => onChange({ ...ov, layerMask: ov.layerMask ^ (1 << n) })}
              className={[
                'border px-2 py-1 font-mono text-xs font-medium transition-colors',
                on
                  ? 'border-amber-400/50 bg-amber-500/15 text-amber-200'
                  : 'border-[#26313c] text-slate-400 hover:bg-[#111820]',
              ].join(' ')}
            >
              {n}
            </button>
          );
        })}
        <span className="font-mono text-[0.65rem] text-slate-400">
          trigger {formatModifiers(ov.triggerMods)} · replace {formatModifiers(ov.replacementMods)}
        </span>
      </div>
    </li>
  );
}

interface TapDanceRowProps {
  entry: TapDance;
  onPick: (field: 'tap' | 'hold' | 'double') => void;
  onChange: (next: TapDance) => void;
  onRemove: () => void;
}

function TapDanceRow({ entry, onPick, onChange, onRemove }: TapDanceRowProps) {
  return (
    <li className="flex flex-wrap items-end gap-3 border border-[#1b222a] bg-[#020304] p-3">
      <LabelledKey label="Tap" raw={entry.tap} onClick={() => onPick('tap')} />
      <LabelledKey label="Hold" raw={entry.hold} onClick={() => onPick('hold')} />
      <LabelledKey label="Double" raw={entry.double} onClick={() => onPick('double')} />
      <TermInput value={entry.termMs} onChange={(termMs) => onChange({ ...entry, termMs })} />
      <RemoveButton onClick={onRemove} />
    </li>
  );
}

interface ComboRowProps {
  combo: Combo;
  maxKeys: number;
  onPickKey: (keyIndex: number) => void;
  onPickAction: () => void;
  onChange: (next: Combo) => void;
  onRemove: () => void;
}

function ComboRow({ combo, maxKeys, onPickKey, onPickAction, onChange, onRemove }: ComboRowProps) {
  const addKey = () => onChange({ ...combo, keys: [...combo.keys, NONE] });
  const removeKey = (keyIndex: number) =>
    onChange({ ...combo, keys: combo.keys.filter((_, i) => i !== keyIndex) });

  return (
    <li className="flex flex-wrap items-end gap-3 border border-[#1b222a] bg-[#020304] p-3">
      <div className="flex flex-col gap-1">
        <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Keys</span>
        <div className="flex flex-wrap items-center gap-1.5">
          {combo.keys.map((raw, keyIndex) => (
            <div key={keyIndex} className="flex items-center">
              {keyIndex > 0 && <span className="mr-1.5 text-xs text-slate-500">+</span>}
              <KeyButton label={keyLabel(raw)} onClick={() => onPickKey(keyIndex)} />
              {combo.keys.length > MIN_COMBO_KEYS && (
                <button
                  type="button"
                  aria-label="Remove key"
                  onClick={() => removeKey(keyIndex)}
                  className="ml-0.5 text-slate-600 transition-colors hover:text-red-300"
                >
                  x
                </button>
              )}
            </div>
          ))}
          {combo.keys.length < maxKeys && (
            <button
              type="button"
              onClick={addKey}
              className="border border-dashed border-[#26313c] px-2 py-1.5 text-xs text-slate-400 transition-colors hover:border-slate-500 hover:text-slate-200"
            >
              + key
            </button>
          )}
        </div>
      </div>

      <span className="mb-2 font-mono text-slate-500">-&gt;</span>

      <LabelledKey label="Action" raw={combo.action} onClick={onPickAction} />
      <TermInput value={combo.termMs} onChange={(termMs) => onChange({ ...combo, termMs })} />
      <ComboFlags combo={combo} onChange={onChange} />
      <RemoveButton onClick={onRemove} />
    </li>
  );
}

interface ComboFlagsProps {
  combo: Combo;
  onChange: (next: Combo) => void;
}

/**
 * The per-combo behaviour flags: must-hold / must-tap (mutually exclusive — picking one
 * clears the other) and in-order. The term doubles as the hold / tap decision window
 * when must-hold / must-tap is on. Mirrors `timed::ComboCfg`'s `FLAG_*`.
 */
function ComboFlags({ combo, onChange }: ComboFlagsProps) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Rules</span>
      <div className="flex flex-wrap gap-0.5">
        <FlagChip
          label="Hold"
          hint="Fire only after the chord is held for the term; a quick tap types the keys individually."
          on={combo.mustHold}
          onClick={() => onChange({ ...combo, mustHold: !combo.mustHold, mustTap: false })}
        />
        <FlagChip
          label="Tap"
          hint="Fire only when the chord is tapped (released within the term); held longer, the keys type individually."
          on={combo.mustTap}
          onClick={() => onChange({ ...combo, mustTap: !combo.mustTap, mustHold: false })}
        />
        <FlagChip
          label="Order"
          hint="The keys must be pressed in the listed order for the chord to fire."
          on={combo.inOrder}
          onClick={() => onChange({ ...combo, inOrder: !combo.inOrder })}
        />
      </div>
    </div>
  );
}

/** A small on/off chip for one combo flag, with a hover hint. */
function FlagChip({
  label,
  hint,
  on,
  onClick,
}: {
  label: string;
  hint: string;
  on: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip content={hint} align="center">
      <button
        type="button"
        aria-pressed={on}
        onClick={onClick}
        className={[
          'border px-2 py-1.5 font-mono text-[0.6rem] font-medium uppercase transition-colors',
          on
            ? 'border-sky-400/50 bg-sky-500/20 text-sky-100'
            : 'border-[#26313c] text-slate-500 hover:bg-[#111820]',
        ].join(' ')}
      >
        {label}
      </button>
    </Tooltip>
  );
}

interface ModBitsProps {
  value: number;
  onChange: (value: number) => void;
}

/** A compact toggle group over the eight HID modifier bits. */
function ModBits({ value, onChange }: ModBitsProps) {
  return (
    <div className="flex flex-wrap gap-0.5">
      {MODIFIER_BITS.map((mod) => {
        const on = (value & (1 << mod.bit)) !== 0;
        return (
          <Tooltip key={mod.bit} content={mod.name} align="center">
            <button
              type="button"
              aria-label={mod.name}
              aria-pressed={on}
              onClick={() => onChange(value ^ (1 << mod.bit))}
              className={[
                'border px-1 py-0.5 font-mono text-[0.6rem] font-medium transition-colors',
                on
                  ? 'border-sky-400/50 bg-sky-500/20 text-sky-100'
                  : 'border-[#26313c] text-slate-500 hover:bg-[#111820]',
              ].join(' ')}
            >
              {mod.label}
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
}

/** A labelled keycode button (a small caption above the key chooser). */
function LabelledKey({ label, raw, onClick }: { label: string; raw: number; onClick: () => void }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">{label}</span>
      <KeyButton label={keyLabel(raw)} onClick={onClick} />
    </div>
  );
}

/** A labelled millisecond term input (tap / combo decision window). */
function TermInput({ value, onChange }: { value: number; onChange: (value: number) => void }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[0.65rem] uppercase tracking-wide text-slate-500">Term (ms)</span>
      <input
        type="number"
        min={0}
        max={65535}
        value={value}
        onChange={(event) => onChange(clampTerm(Number(event.target.value)))}
        className="w-24 border border-[#26313c] bg-[#020304] px-2 py-1.5 text-xs text-slate-100 focus:border-sky-400 focus:outline-none"
      />
    </label>
  );
}

function KeyButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="min-w-[3rem] border border-[#26313c] bg-[#111820] px-2 py-1.5 font-mono text-xs font-medium text-slate-100 transition-colors hover:border-sky-400/60 hover:bg-sky-500/10"
    >
      {label}
    </button>
  );
}

function RemoveButton({ onClick }: { onClick: () => void }) {
  return (
    <Tooltip content="Remove entry" align="right">
      <button
        type="button"
        onClick={onClick}
        aria-label="Remove"
        className="border border-[#26313c] px-2 py-1.5 text-xs text-slate-400 transition-colors hover:border-red-500/40 hover:bg-red-500/10 hover:text-red-200"
      >
        x
      </button>
    </Tooltip>
  );
}
