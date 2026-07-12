// SPDX-License-Identifier: GPL-2.0-or-later
import { useState } from 'react';
import {
  AUTOCORRECT_OFF,
  AUTOCORRECT_ON,
  AUTOCORRECT_TOGGLE,
  AUTO_SHIFT_OFF,
  AUTO_SHIFT_ON,
  AUTO_SHIFT_TOGGLE,
  CATEGORY_LABELS,
  defaultLayer,
  BOOTLOADER,
  MODIFIERS,
  GRAVE_ESCAPE,
  LEADER,
  LAYER_LOCK,
  SPACE_CADET_PAREN_LEFT,
  SPACE_CADET_PAREN_RIGHT,
  SPACE_CADET_ENTER,
  UNICODE_MAP_COUNT,
  altRepeat,
  capsWord,
  keyLock,
  keycodeLabel,
  keycodesByCategory,
  layerTap,
  macro,
  modTap,
  momentary,
  oneShot,
  oneShotMod,
  repeatKey,
  tapdance,
  tapToggle,
  toggle,
  toLayer,
  unicodeMap,
  unicodeModeCycle,
  type NamedKeycode,
} from '../kcp';
import { keyName } from './keyDisplay';

/**
 * The tap keys offered by the mod-tap / layer-tap builders — letters, numbers and the
 * whitespace keys, the usages dual-role keys almost always tap. Built once from the
 * keycode catalogue; a tap-hold's tap usage is the keycode's low byte, so any basic
 * usage works.
 */
const TAP_HOLD_KEYS: readonly NamedKeycode[] = [
  ...keycodesByCategory('letters'),
  ...keycodesByCategory('numbers'),
  ...keycodesByCategory('whitespace'),
];

/**
 * Where a {@link KeycodePicker} is mounted, which bounds the keycodes it offers.
 * The keymap grid resolves any keycode, so it gets the full set (basic keys,
 * modifiers, consumer/media, mouse, gamepad, and the `MO`/`TO`/`TG`/`TT`/`OSL`/`TD`/`MACRO`/`Boot`
 * engine codes); the tap-dance / combo / macro editors only ever re-emit their output
 * as a plain HID usage, so they get basic keys + modifiers and nothing that would
 * silently no-op there (consumer, mouse, gamepad, layer, tap-dance, macro or boot codes).
 */
export type KeycodeContext = 'keymap' | 'basic';

interface KeycodePickerProps {
  /** Where the picker is mounted; bounds the offered keycodes (see {@link KeycodeContext}). */
  context: KeycodeContext;
  /** Number of keymap layers, for the `MO(n)` momentary-switch options. */
  layerCount: number;
  /** Tap-dance table capacity, for the `TD(n)` options (keymap context only). */
  tapDanceCount?: number;
  /** Macro table capacity, for the `MACRO(n)` options (keymap context only). */
  macroCount?: number;
  /** The keycode currently bound at the position being edited. */
  currentRaw: number;
  /** Short, user-facing context for the item being edited. */
  contextLabel?: string;
  /** Heading override for non-keymap callers (e.g. the behaviours editor). */
  title?: string;
  onPick: (raw: number) => void;
  onClose: () => void;
}

/**
 * A categorised keycode chooser (letters / numbers / symbols / F-keys / nav /
 * modifiers / media / mouse / gamepad, plus — in the keymap {@link KeycodeContext} — a
 * Special / Layers group of the `MO`/`TO`/`TG`/`TT`/`OSL`/`TD`/`MACRO`/`Boot`
 * engine codes). Picking a
 * keycode hands its raw `u16` back to the caller, which writes it live. Shared by
 * the keymap grid and the tap-dance / combo / macro / override editors; `context`
 * narrows the offered set so an editor never shows a code its firmware path would
 * silently drop.
 */
export function KeycodePicker({
  context,
  layerCount,
  tapDanceCount,
  macroCount,
  currentRaw,
  contextLabel,
  title,
  onPick,
  onClose,
}: KeycodePickerProps) {
  // The selected modifier (mod-tap) and layer (layer-tap) for the tap-hold builders;
  // clicking a tap key below wraps it with the current selection. Keymap context only.
  const [mtMod, setMtMod] = useState(MODIFIERS[0].bit);
  const [ltLayer, setLtLayer] = useState(layerCount > 1 ? 1 : 0);

  // Consumer/media, mouse and gamepad keycodes ride separate HID reports the timed
  // engine never emits, and the Special / Layers codes resolve to layers/behaviours
  // rather than a usage, so all are offered only where the keymap engine reads them.
  const categories =
    context === 'keymap'
      ? CATEGORY_LABELS
      : CATEGORY_LABELS.filter(
          (c) => c.category !== 'media' && c.category !== 'mouse' && c.category !== 'gamepad',
        );
  const specialOptions =
    context === 'keymap'
      ? [
          BOOTLOADER,
          LAYER_LOCK,
          AUTO_SHIFT_TOGGLE,
          AUTO_SHIFT_ON,
          AUTO_SHIFT_OFF,
          LEADER,
          capsWord(),
          keyLock(),
          repeatKey(),
          altRepeat(),
          AUTOCORRECT_TOGGLE,
          AUTOCORRECT_ON,
          AUTOCORRECT_OFF,
          // One per HID modifier (`OSM` index 0..=7), like the per-layer codes below.
          ...MODIFIERS.map((_, n) => oneShotMod(n)),
          GRAVE_ESCAPE,
          SPACE_CADET_PAREN_LEFT,
          SPACE_CADET_PAREN_RIGHT,
          SPACE_CADET_ENTER,
          // Unicode input: the OS mode-cycle key, then one `UM(n)` per codepoint slot.
          unicodeModeCycle(),
          ...Array.from({ length: UNICODE_MAP_COUNT }, (_, n) => unicodeMap(n)),
          ...Array.from({ length: layerCount }, (_, n) => momentary(n)),
          ...Array.from({ length: layerCount }, (_, n) => toLayer(n)),
          ...Array.from({ length: layerCount }, (_, n) => toggle(n)),
          ...Array.from({ length: layerCount }, (_, n) => tapToggle(n)),
          ...Array.from({ length: layerCount }, (_, n) => oneShot(n)),
          ...Array.from({ length: layerCount }, (_, n) => defaultLayer(n)),
          ...Array.from({ length: tapDanceCount ?? 0 }, (_, n) => tapdance(n)),
          ...Array.from({ length: macroCount ?? 0 }, (_, n) => macro(n)),
        ]
      : [];

  return (
    <div
      className="fixed inset-0 z-20 flex items-start justify-center bg-[#020304]/82 px-3 py-[8vh] backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label={title ?? 'Choose a key'}
      onClick={onClose}
    >
      <div
        className="flex max-h-[84vh] w-full max-w-5xl flex-col overflow-hidden border border-sky-400/40 bg-[#050608]"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[#1b222a] bg-[#020304] px-4 py-3">
          <div>
            <p className="font-mono text-[0.65rem] uppercase tracking-[0.22em] text-sky-300">
              key library
            </p>
            <h3 className="mt-1 text-sm font-semibold text-slate-100">
              {title ?? 'Choose a key'}
              {contextLabel && (
                <span className="ml-2 font-mono text-xs text-slate-500">{contextLabel}</span>
              )}
            </h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="border border-[#26313c] px-3 py-1.5 font-mono text-xs uppercase text-slate-300 transition-colors hover:border-red-400/50 hover:bg-red-500/10 hover:text-red-200"
          >
            Close
          </button>
        </div>

        <div className="grid gap-3 overflow-y-auto p-4 md:grid-cols-2">
          {categories.map(({ category, label }) => (
            <section key={category} className="border border-[#1b222a] bg-[#090c10] p-3">
              <h4 className="mb-3 font-mono text-[0.65rem] font-semibold uppercase tracking-[0.2em] text-slate-500">
                {label}
              </h4>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(3rem,1fr))] gap-1.5">
                {keycodesByCategory(category).map((kc) => (
                  <KeyOption
                    key={kc.raw}
                    label={kc.label}
                    active={kc.raw === currentRaw}
                    onClick={() => onPick(kc.raw)}
                  />
                ))}
              </div>
            </section>
          ))}

          {specialOptions.length > 0 && (
            <section className="border border-amber-400/30 bg-amber-500/10 p-3 md:col-span-2">
              <h4 className="mb-3 font-mono text-[0.65rem] font-semibold uppercase tracking-[0.2em] text-amber-200">
                Special / Layers
              </h4>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(4rem,1fr))] gap-1.5">
                {specialOptions.map((raw) => (
                  <KeyOption
                    key={raw}
                    label={keycodeLabel(raw)}
                    active={raw === currentRaw}
                    onClick={() => onPick(raw)}
                  />
                ))}
              </div>
            </section>
          )}

          {context === 'keymap' && (
            <>
              <TapHoldSection
                title="Mod-Tap"
                selectorLabel="Hold modifier"
                selector={MODIFIERS.map((m) => ({ value: m.bit, label: m.short }))}
                selected={mtMod}
                onSelect={setMtMod}
                build={(usage) => modTap(mtMod, usage)}
                currentRaw={currentRaw}
                onPick={onPick}
              />
              <TapHoldSection
                title="Layer-Tap"
                selectorLabel="Hold layer"
                selector={Array.from({ length: layerCount }, (_, n) => ({
                  value: n,
                  label: `L${n}`,
                }))}
                selected={ltLayer}
                onSelect={setLtLayer}
                build={(usage) => layerTap(ltLayer, usage)}
                currentRaw={currentRaw}
                onPick={onPick}
              />
            </>
          )}
        </div>

        <div className="border-t border-[#1b222a] bg-[#020304] px-4 py-2 font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
          {keyName(currentRaw)} selected / choose a binding
        </div>
      </div>
    </div>
  );
}

interface TapHoldSectionProps {
  /** Section heading, e.g. `Mod-Tap`. */
  title: string;
  /** Label for the hold selector, e.g. `Hold modifier`. */
  selectorLabel: string;
  /** The hold options (a modifier bit or a layer index) and their short labels. */
  selector: { value: number; label: string }[];
  /** The currently selected hold value. */
  selected: number;
  onSelect: (value: number) => void;
  /** Wrap a tap usage into the encoded `MT`/`LT` keycode under the current selection. */
  build: (usage: number) => number;
  /** The keycode bound at the position being edited, to highlight a matching tap key. */
  currentRaw: number;
  onPick: (raw: number) => void;
}

/**
 * The mod-tap / layer-tap builder: pick a hold (a modifier or a layer), then a tap key
 * — the two halves of a dual-role key. Clicking a tap key emits the encoded `MT`/`LT`
 * keycode, the same way the Special / Layers section emits its codes.
 */
function TapHoldSection({
  title,
  selectorLabel,
  selector,
  selected,
  onSelect,
  build,
  currentRaw,
  onPick,
}: TapHoldSectionProps) {
  return (
    <section className="border border-emerald-400/30 bg-emerald-500/10 p-3 md:col-span-2">
      <h4 className="mb-3 font-mono text-[0.65rem] font-semibold uppercase tracking-[0.2em] text-emerald-200">
        {title}
      </h4>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <span className="font-mono text-[0.6rem] uppercase tracking-wide text-slate-500">
          {selectorLabel}
        </span>
        {selector.map((opt) => (
          <KeyOption
            key={opt.value}
            label={opt.label}
            active={opt.value === selected}
            onClick={() => onSelect(opt.value)}
          />
        ))}
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(3rem,1fr))] gap-1.5">
        {TAP_HOLD_KEYS.map((kc) => {
          const raw = build(kc.raw);
          return (
            <KeyOption
              key={kc.raw}
              label={kc.label}
              active={raw === currentRaw}
              onClick={() => onPick(raw)}
            />
          );
        })}
      </div>
    </section>
  );
}

interface KeyOptionProps {
  label: string;
  active: boolean;
  onClick: () => void;
}

function KeyOption({ label, active, onClick }: KeyOptionProps) {
  return (
    <button
      type="button"
      aria-label={`Choose ${label}`}
      aria-pressed={active}
      onClick={onClick}
      className={[
        'min-h-9 min-w-[2.75rem] border px-2 py-1.5 font-mono text-xs font-medium leading-tight transition-colors',
        active
          ? 'border-sky-300 bg-sky-500/25 text-sky-50 shadow-[0_0_18px_rgba(56,189,248,0.22)]'
          : 'border-[#26313c] bg-[#020304] text-slate-300 hover:border-sky-400/60 hover:bg-[#111820] hover:text-sky-100',
      ].join(' ')}
    >
      {label}
    </button>
  );
}
