// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState, type ReactNode } from 'react';
import { hsvToRgb, rgbToCss } from '../kcp/rgb';
import {
  controlKey,
  defaultValue,
  evalShowIf,
  readControlValue,
  writeControlValue,
  type ControlValue,
} from '../featureDescriptors/runtime';
import type {
  ColorControl,
  Control,
  EnumControl,
  FeatureDescriptor,
  NumberControl,
  OpRunner,
  SliderControl,
  ToggleControl,
} from '../featureDescriptors/types';
import { ErrorBanner, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface DescriptorPanelProps {
  descriptor: FeatureDescriptor;
  client: OpRunner;
}

/**
 * The one generic renderer for a `FeatureDescriptor` (planning §6, "Design 3"): it draws each
 * control by its `kind` to the matching widget, seeds every control from its `get` op on
 * mount, writes each change live through its `set` op (optimistic, reverting on failure — the
 * same idiom as TuningPanel / RgbPanel), and hides controls whose `showIf` is false. There is
 * no per-feature code here: a feature's whole config panel is its data.
 */
export function DescriptorPanel({ descriptor, client }: DescriptorPanelProps) {
  const [values, setValues] = useState<Record<string, ControlValue>>(() =>
    seedDefaults(descriptor),
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setValues(seedDefaults(descriptor));
    (async () => {
      try {
        const seeded = await Promise.all(
          descriptor.controls.map((control) => readControlValue(client, control)),
        );
        if (cancelled) return;
        const next: Record<string, ControlValue> = {};
        descriptor.controls.forEach((control, index) => {
          next[controlKey(control, index)] = seeded[index];
        });
        setValues(next);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [descriptor, client]);

  /** Apply a control change live, reverting just that control if the write fails. */
  async function commit(key: string, control: Control, value: ControlValue) {
    const previous = values[key];
    setValues((current) => ({ ...current, [key]: value }));
    setError(null);
    try {
      await writeControlValue(client, control, value);
    } catch (err) {
      setValues((current) => ({ ...current, [key]: previous }));
      setError(friendlyPanelError(err));
    }
  }

  const scalars = scalarValues(descriptor, values);

  return (
    <Panel
      title={descriptor.title}
      description={`Rendered from a feature descriptor (fid ${descriptor.fid}) — no per-feature code.`}
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <div className="flex flex-col gap-4">
        {descriptor.controls.map((control, index) => {
          if (control.showIf && !visible(control.showIf, scalars)) return null;
          const key = controlKey(control, index);
          return (
            <ControlField
              key={key}
              control={control}
              value={values[key] ?? defaultValue(control)}
              onPreview={(value) => setValues((current) => ({ ...current, [key]: value }))}
              onCommit={(value) => void commit(key, control, value)}
            />
          );
        })}
      </div>
    </Panel>
  );
}

/** The initial value map (defaults), shown until the `get` ops resolve. */
function seedDefaults(descriptor: FeatureDescriptor): Record<string, ControlValue> {
  const seed: Record<string, ControlValue> = {};
  descriptor.controls.forEach((control, index) => {
    seed[controlKey(control, index)] = defaultValue(control);
  });
  return seed;
}

/** The token→number view of the live values that `showIf` reads (colours excluded). */
function scalarValues(
  descriptor: FeatureDescriptor,
  values: Record<string, ControlValue>,
): Record<string, number> {
  const scalars: Record<string, number> = {};
  descriptor.controls.forEach((control, index) => {
    if (!control.token) return;
    const value = values[controlKey(control, index)];
    if (typeof value === 'number') scalars[control.token] = value;
  });
  return scalars;
}

/** Evaluate a `showIf`, failing open (show) on a malformed expression per the LSP rule. */
function visible(showIf: string, scalars: Record<string, number>): boolean {
  try {
    return evalShowIf(showIf, scalars);
  } catch {
    return true;
  }
}

interface ControlFieldProps {
  control: Control;
  value: ControlValue;
  onPreview: (value: ControlValue) => void;
  onCommit: (value: ControlValue) => void;
}

/** Render one control by its kind — the kind→widget table that makes the panel generic. */
function ControlField({ control, value, onPreview, onCommit }: ControlFieldProps) {
  switch (control.kind) {
    case 'toggle':
      return (
        <ToggleField control={control} on={value === 1} onCommit={(on) => onCommit(on ? 1 : 0)} />
      );
    case 'slider':
      return (
        <SliderField
          control={control}
          value={asNumber(value)}
          onPreview={onPreview}
          onCommit={onCommit}
        />
      );
    case 'number':
      return <NumberField control={control} value={asNumber(value)} onCommit={onCommit} />;
    case 'enum':
      return <EnumField control={control} value={asNumber(value)} onCommit={onCommit} />;
    case 'color':
      return (
        <ColorField
          control={control}
          value={asColor(value)}
          onPreview={onPreview}
          onCommit={onCommit}
        />
      );
  }
}

function asNumber(value: ControlValue): number {
  return typeof value === 'number' ? value : 0;
}

function asColor(value: ControlValue): [number, number, number] {
  return Array.isArray(value) ? [value[0] ?? 0, value[1] ?? 0, value[2] ?? 0] : [0, 0, 0];
}

function FieldLabel({ children }: { children: ReactNode }) {
  return (
    <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
      {children}
    </span>
  );
}

function ToggleField({
  control,
  on,
  onCommit,
}: {
  control: ToggleControl;
  on: boolean;
  onCommit: (on: boolean) => void;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <FieldLabel>{control.label}</FieldLabel>
      <div className="inline-grid w-fit grid-cols-2 border border-[#26313c] bg-[#020304] p-1">
        {[
          { value: false, text: 'Off' },
          { value: true, text: 'On' },
        ].map(({ value, text }) => (
          <button
            key={text}
            type="button"
            aria-pressed={on === value}
            onClick={() => onCommit(value)}
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

function SliderField({
  control,
  value,
  onPreview,
  onCommit,
}: {
  control: SliderControl;
  value: number;
  onPreview: (value: number) => void;
  onCommit: (value: number) => void;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="flex items-baseline justify-between font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {control.label}
        <span className="font-mono text-slate-400">{value}</span>
      </span>
      <input
        type="range"
        min={control.min}
        max={control.max}
        step={control.step ?? 1}
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

function NumberField({
  control,
  value,
  onCommit,
}: {
  control: NumberControl;
  value: number;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));

  // Re-sync the draft when the committed value changes (e.g. a reverted write or a seed).
  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  const commit = () => {
    const parsed = Math.round(Number(draft));
    const clamped = Number.isFinite(parsed)
      ? Math.max(control.min, Math.min(control.max, parsed))
      : control.min;
    onCommit(clamped);
  };

  return (
    <label className="flex flex-col gap-1.5">
      <FieldLabel>{control.label}</FieldLabel>
      <input
        type="number"
        min={control.min}
        max={control.max}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') event.currentTarget.blur();
        }}
        className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
      />
    </label>
  );
}

function EnumField({
  control,
  value,
  onCommit,
}: {
  control: EnumControl;
  value: number;
  onCommit: (value: number) => void;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <FieldLabel>{control.label}</FieldLabel>
      <select
        value={value}
        onChange={(event) => onCommit(Number(event.target.value))}
        className="border border-[#26313c] bg-[#020304] px-3 py-2 text-sm text-slate-100 focus:border-sky-400 focus:outline-none"
      >
        {control.options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function ColorField({
  control,
  value,
  onPreview,
  onCommit,
}: {
  control: ColorControl;
  value: [number, number, number];
  onPreview: (value: ControlValue) => void;
  onCommit: (value: ControlValue) => void;
}) {
  const [h, s, v] = value;
  const swatch = rgbToCss(hsvToRgb(h, s, v));
  const withChannel = (index: 0 | 1 | 2, next: number): [number, number, number] => {
    const updated: [number, number, number] = [value[0], value[1], value[2]];
    updated[index] = next;
    return updated;
  };

  return (
    <div className="flex flex-col gap-2">
      <FieldLabel>{control.label}</FieldLabel>
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_6rem]">
        <div className="flex flex-col gap-3">
          <ChannelSlider
            label="Hue"
            value={h}
            onPreview={(next) => onPreview(withChannel(0, next))}
            onCommit={(next) => onCommit(withChannel(0, next))}
          />
          <ChannelSlider
            label="Saturation"
            value={s}
            onPreview={(next) => onPreview(withChannel(1, next))}
            onCommit={(next) => onCommit(withChannel(1, next))}
          />
          <ChannelSlider
            label="Intensity"
            value={v}
            onPreview={(next) => onPreview(withChannel(2, next))}
            onCommit={(next) => onCommit(withChannel(2, next))}
          />
        </div>
        <div
          className="aspect-square w-full border border-[#26313c]"
          style={{ backgroundColor: swatch }}
          aria-label="Colour preview"
        />
      </div>
    </div>
  );
}

function ChannelSlider({
  label,
  value,
  onPreview,
  onCommit,
}: {
  label: string;
  value: number;
  onPreview: (value: number) => void;
  onCommit: (value: number) => void;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="flex items-baseline justify-between font-mono text-[0.6rem] uppercase tracking-wide text-slate-500">
        {label}
        <span className="text-slate-400">{value}</span>
      </span>
      <input
        type="range"
        min={0}
        max={255}
        value={value}
        onChange={(event) => onPreview(Number(event.target.value))}
        onPointerUp={(event) => onCommit(Number(event.currentTarget.value))}
        onBlur={(event) => onCommit(Number(event.currentTarget.value))}
        className="h-3 w-full cursor-pointer appearance-none bg-transparent accent-sky-400"
      />
    </label>
  );
}
