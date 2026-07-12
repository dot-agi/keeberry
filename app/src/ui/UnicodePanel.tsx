// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useState } from 'react';
import { UNICODE_MAP_SLOTS, UnicodeMode, unicodeModeLabel, type KcpClient } from '../kcp';
import { ErrorBanner, InfoHint, Panel } from './Panel';
import { friendlyPanelError } from './panelError';
import { isScalar, readStoredMap, restoreUnicodeMap, writeStoredMap } from './unicodeMap';

interface UnicodePanelProps {
  client: KcpClient;
}

/** The OS modes offered, with the entry recipe the firmware plays back for each. */
const MODE_OPTIONS: readonly { mode: UnicodeMode; recipe: string }[] = [
  { mode: UnicodeMode.Linux, recipe: 'Ctrl + Shift + U, the hex digits, then Space.' },
  { mode: UnicodeMode.MacOS, recipe: 'Hold Option while typing the hex (enable “Unicode Hex Input”).' },
  { mode: UnicodeMode.Windows, recipe: 'Right Alt, then U, the hex digits, then Enter (needs WinCompose).' },
];

/**
 * Unicode-input control: pick the active OS entry mode (UNICODE_SET_MODE) and fill the
 * codepoint table the `UM(n)` keycodes type (UNICODE_SET_MAP). Each slot accepts a
 * hex codepoint (`1F600`, `U+00E9`) or a single pasted character/emoji.
 *
 * The device keeps the map in RAM only — there is no flash persistence — so this panel
 * owns the authoritative copy in `localStorage` and re-uploads every slot on connect, the
 * same way a freshly-powered keyboard expects the host to restore it.
 */
export function UnicodePanel({ client }: UnicodePanelProps) {
  const [mode, setMode] = useState<UnicodeMode | null>(null);
  const [slots, setSlots] = useState(UNICODE_MAP_SLOTS);
  const [codepoints, setCodepoints] = useState<number[]>(() => readStoredMap());
  const [inputs, setInputs] = useState<string[]>(() => readStoredMap().map(formatCodepoint));
  const [invalid, setInvalid] = useState<ReadonlySet<number>>(() => new Set());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // The connection lifecycle restores the cached map to device RAM on every connect; the
  // panel re-runs the same restore on mount, both as a fallback and to read the active mode
  // and slot count for display.
  useEffect(() => {
    let cancelled = false;

    const sync = async () => {
      setBusy(true);
      try {
        const info = await restoreUnicodeMap(client);
        if (cancelled) return;
        setMode(info.mode);
        setSlots(Math.min(info.slots, UNICODE_MAP_SLOTS));
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      } finally {
        if (!cancelled) setBusy(false);
      }
    };

    void sync();
    return () => {
      cancelled = true;
    };
  }, [client]);

  const selectMode = useCallback(
    async (next: UnicodeMode) => {
      setBusy(true);
      setError(null);
      try {
        await client.unicodeSetMode(next);
        setMode(next);
      } catch (err) {
        setError(friendlyPanelError(err));
      } finally {
        setBusy(false);
      }
    },
    [client],
  );

  /** Parse and upload a slot's input; an unparseable entry flags the slot and uploads nothing. */
  const commitSlot = useCallback(
    async (slot: number, raw: string) => {
      const cp = parseCodepoint(raw);
      if (cp === null) {
        setInvalid((prev) => new Set(prev).add(slot));
        return;
      }
      setInvalid((prev) => {
        const next = new Set(prev);
        next.delete(slot);
        return next;
      });
      setCodepoints((prev) => {
        const next = [...prev];
        next[slot] = cp;
        writeStoredMap(next);
        return next;
      });
      setInputs((prev) => {
        const next = [...prev];
        next[slot] = formatCodepoint(cp);
        return next;
      });
      try {
        await client.unicodeSetMap(slot, cp);
        setError(null);
      } catch (err) {
        setError(friendlyPanelError(err));
      }
    },
    [client],
  );

  const activeRecipe = MODE_OPTIONS.find((option) => option.mode === mode)?.recipe;

  return (
    <Panel
      title="Unicode input"
      description="Type any codepoint through the active OS input method, via the UM(n) keycodes."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <div className="mb-5">
        <div className="mb-2 flex items-center gap-2">
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
            OS input mode
          </span>
          <InfoHint label="OS input mode details">
            The firmware plays back this OS&apos;s own Unicode entry sequence. The map below is
            kept on this computer and re-sent to the keyboard each time it connects.
          </InfoHint>
        </div>
        <div className="grid grid-cols-[repeat(auto-fit,minmax(8rem,1fr))] gap-1.5">
          {MODE_OPTIONS.map((option) => (
            <button
              key={option.mode}
              type="button"
              disabled={busy}
              aria-pressed={mode === option.mode}
              onClick={() => void selectMode(option.mode)}
              className={[
                'min-h-9 border px-3 py-2 font-mono text-xs font-semibold uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-50',
                mode === option.mode
                  ? 'border-sky-400 bg-sky-500/20 text-sky-100'
                  : 'border-[#26313c] bg-[#020304] text-slate-400 hover:border-sky-400/50 hover:bg-[#111820]',
              ].join(' ')}
            >
              {unicodeModeLabel(option.mode)}
            </button>
          ))}
        </div>
        {activeRecipe && (
          <p className="mt-2 font-mono text-xs text-slate-400">{activeRecipe}</p>
        )}
      </div>

      <div>
        <div className="mb-2 flex items-center gap-2">
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
            Codepoint slots
          </span>
          <InfoHint label="Codepoint slot details">
            Bind <span className="font-mono">UM(0)</span>…
            <span className="font-mono">UM({slots - 1})</span> in the keymap; pressing one types the
            matching slot. Enter a hex codepoint (e.g.
            <span className="font-mono"> 1F600</span>) or paste a character.
          </InfoHint>
        </div>
        <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
          {Array.from({ length: slots }, (_, slot) => (
            <SlotRow
              key={slot}
              slot={slot}
              value={inputs[slot] ?? ''}
              glyph={glyphOf(codepoints[slot] ?? 0)}
              invalid={invalid.has(slot)}
              disabled={busy}
              onInput={(text) =>
                setInputs((prev) => {
                  const next = [...prev];
                  next[slot] = text;
                  return next;
                })
              }
              onCommit={(text) => void commitSlot(slot, text)}
            />
          ))}
        </div>
      </div>
    </Panel>
  );
}

interface SlotRowProps {
  slot: number;
  value: string;
  glyph: string;
  invalid: boolean;
  disabled: boolean;
  onInput: (text: string) => void;
  onCommit: (text: string) => void;
}

/** One `UM(n)` slot: a hex/character input, its rendered glyph, and an invalid-entry cue. */
function SlotRow({ slot, value, glyph, invalid, disabled, onInput, onCommit }: SlotRowProps) {
  return (
    <label className="flex items-center gap-2 border border-[#1b222a] bg-[#040608] px-2.5 py-1.5">
      <span className="w-12 shrink-0 font-mono text-[0.7rem] uppercase text-slate-500">
        UM{slot}
      </span>
      <span
        className="grid h-7 w-7 shrink-0 place-items-center border border-[#26313c] bg-[#020304] text-base text-slate-100"
        aria-hidden
      >
        {glyph}
      </span>
      <input
        type="text"
        inputMode="text"
        spellCheck={false}
        autoComplete="off"
        disabled={disabled}
        value={value}
        aria-label={`Codepoint for UM ${slot}`}
        aria-invalid={invalid || undefined}
        placeholder="hex or char"
        onChange={(event) => onInput(event.target.value)}
        onBlur={(event) => onCommit(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault();
            onCommit(event.currentTarget.value);
          }
        }}
        className={[
          'min-w-0 flex-1 border bg-[#020304] px-2 py-1 font-mono text-xs text-slate-100 placeholder:text-slate-600 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50',
          invalid ? 'border-red-400/60 text-red-200' : 'border-[#26313c] focus:border-sky-400/60',
        ].join(' ')}
      />
    </label>
  );
}

/**
 * Parse a slot input to a codepoint: empty clears the slot (`0`); a single pasted non-ASCII
 * character is taken literally; anything else is a hex codepoint with an optional `U+`/`0x`
 * prefix. Returns `null` for an entry that is neither a character nor a valid scalar hex.
 */
function parseCodepoint(input: string): number | null {
  const trimmed = input.trim();
  if (trimmed === '') return 0;

  const chars = [...trimmed];
  if (chars.length === 1) {
    const cp = chars[0].codePointAt(0) ?? 0;
    // A literal character (emoji, CJK, accented) is unambiguous; bare ASCII falls through
    // so a digit like `5` or letter `a` reads as hex, the slot's primary entry form.
    if (cp > 0x7f) return isScalar(cp) ? cp : null;
  }

  const hex = trimmed.replace(/^(u\+|0x)/i, '');
  if (!/^[0-9a-f]{1,6}$/i.test(hex)) return null;
  const cp = parseInt(hex, 16);
  return isScalar(cp) ? cp : null;
}

/** Format a stored codepoint back into its hex input text (`0` is an empty slot). */
function formatCodepoint(cp: number): string {
  return cp > 0 ? cp.toString(16).toUpperCase() : '';
}

/** Render a codepoint's glyph for the preview, or a dot for an empty / unrenderable slot. */
function glyphOf(cp: number): string {
  if (cp <= 0 || !isScalar(cp)) return '·';
  try {
    return String.fromCodePoint(cp);
  } catch {
    return '·';
  }
}
