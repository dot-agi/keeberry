// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useState } from 'react';
import { NONE, type KcpClient, type MacroInfo, type MacroStep } from '../kcp';
import { ActionMenu, ErrorBanner, Panel, Tooltip } from './Panel';
import { friendlyPanelError } from './panelError';
import { KeycodePicker } from './KeycodePicker';
import { keyLabel } from './keyDisplay';

interface MacroPanelProps {
  client: KcpClient;
  /** Keymap layer count, for the `MO(n)` options in the step keycode picker. */
  layerCount: number;
}

/** The editable view of one macro: its ordered list of press/release steps. */
interface MacroView {
  steps: MacroStep[];
}

const NEW_STEP: MacroStep = { keycode: NONE, down: true, delayMs: 0 };

/**
 * Macro-table editor (the MACRO group). Each macro is a sequence of key
 * press/release steps with an inter-step delay, replayed by the firmware's timed
 * engine. Capacities (`MAX_MACRO`, `MAX_MACRO_STEPS`) are read live from
 * MACRO_INFO; every edit is written through SET_STEP and takes effect at once.
 * Steps can be appended and edited in place; the firmware can only grow or wipe a
 * macro, so shortening one is done with Clear (then re-add). Play fires a macro
 * immediately without a keymap binding.
 */
export function MacroPanel({ client, layerCount }: MacroPanelProps) {
  const [info, setInfo] = useState<MacroInfo | null>(null);
  const [macros, setMacros] = useState<MacroView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pick, setPick] = useState<{ macro: number; step: number } | null>(null);
  /** Slot currently recording on-device (only one at a time), or null when idle. */
  const [recording, setRecording] = useState<number | null>(null);

  // Read back one macro's full step list (GET_STEP is per-step, so walk to `len`).
  const readMacroSteps = useCallback(
    async (macro: number): Promise<MacroStep[]> => {
      const head = await client.macroGetStep(macro, 0);
      const steps: MacroStep[] = [];
      if (head.len > 0) {
        steps.push(head.step);
        for (let s = 1; s < head.len; s += 1) {
          steps.push((await client.macroGetStep(macro, s)).step);
        }
      }
      return steps;
    },
    [client],
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const mi = await client.macroInfo();
        const views = await Promise.all(
          Array.from({ length: mi.maxMacro }, async (_, macro) => ({
            steps: await readMacroSteps(macro),
          })),
        );
        if (cancelled) return;
        setInfo(mi);
        setMacros(views);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, readMacroSteps]);

  const guard = useCallback(async (run: () => Promise<void>) => {
    setError(null);
    try {
      await run();
    } catch (err) {
      setError(friendlyPanelError(err));
    }
  }, []);

  const writeStep = (macro: number, step: number, ev: MacroStep) =>
    guard(async () => {
      await client.macroSetStep(macro, step, ev);
      setMacros((prev) => {
        const next = prev.slice();
        const steps = next[macro].steps.slice();
        steps[step] = ev;
        next[macro] = { steps };
        return next;
      });
    });

  const addStep = (macro: number) => writeStep(macro, macros[macro].steps.length, { ...NEW_STEP });

  const clearMacro = (macro: number) =>
    guard(async () => {
      await client.macroClear(macro);
      setMacros((prev) => {
        const next = prev.slice();
        next[macro] = { steps: [] };
        return next;
      });
    });

  const playMacro = (macro: number) => guard(() => client.macroPlay(macro));

  // On-board recording: START clears the slot and the device captures the keys
  // pressed thereafter (they still type live); STOP ends capture and we read the
  // recorded steps back so the editor reflects what was captured.
  const startRecord = (macro: number) =>
    guard(async () => {
      await client.macroRecordStart(macro);
      setRecording(macro);
      setMacros((prev) => {
        const next = prev.slice();
        next[macro] = { steps: [] };
        return next;
      });
    });

  const stopRecord = (macro: number) =>
    guard(async () => {
      await client.macroRecordStop();
      const steps = await readMacroSteps(macro);
      setRecording(null);
      setMacros((prev) => {
        const next = prev.slice();
        next[macro] = { steps };
        return next;
      });
    });

  function handlePick(raw: number) {
    const target = pick;
    setPick(null);
    if (!target) return;
    const ev = macros[target.macro]?.steps[target.step];
    if (ev) void writeStep(target.macro, target.step, { ...ev, keycode: raw });
  }

  const pickerRaw = pick ? (macros[pick.macro]?.steps[pick.step]?.keycode ?? NONE) : NONE;

  return (
    <Panel
      title="Macros"
      description="Each macro is an ordered press/release sequence with an optional inter-step delay. Edits write live."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {!info ? (
        !error && <p className="text-sm text-slate-400">Reading macros…</p>
      ) : (
        <ul className="flex flex-col gap-3">
          {macros.map((macro, index) => (
            <li key={index} className="border border-[#1b222a] bg-[#020304] p-3">
              <div className="mb-2 flex items-center justify-between gap-2">
                <h3 className="font-mono text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">
                  Macro {index}
                  <span className="ml-2 font-mono text-slate-400">
                    {macro.steps.length} / {info.maxSteps}
                  </span>
                </h3>
                <div className="flex items-center gap-1.5">
                  <Tooltip
                    content={
                      recording === index
                        ? 'Stop recording and read back the captured steps.'
                        : 'Record the keys you press into this macro (clears it first).'
                    }
                    align="right"
                  >
                    <button
                      type="button"
                      aria-label={
                        recording === index
                          ? `Stop recording macro ${index}`
                          : `Record macro ${index}`
                      }
                      aria-pressed={recording === index}
                      disabled={recording !== null && recording !== index}
                      onClick={() =>
                        void (recording === index ? stopRecord(index) : startRecord(index))
                      }
                      className={[
                        'kb-control kb-control-sm font-mono uppercase disabled:cursor-not-allowed disabled:opacity-40',
                        recording === index
                          ? 'border-rose-400/50 bg-rose-500/15 text-rose-200'
                          : '',
                      ].join(' ')}
                    >
                      {recording === index ? '■ stop' : '● rec'}
                    </button>
                  </Tooltip>
                  <Tooltip content="Append a press step to this macro." align="right">
                    <button
                      type="button"
                      aria-label={`Add step to macro ${index}`}
                      disabled={macro.steps.length >= info.maxSteps || recording === index}
                      onClick={() => void addStep(index)}
                      className="kb-control kb-control-sm font-mono uppercase disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      + step
                    </button>
                  </Tooltip>
                  <ActionMenu
                    label="Macro"
                    align="left"
                    actions={[
                      {
                        id: `play-${index}`,
                        label: 'Play',
                        detail: 'Fire without binding',
                        disabled: macro.steps.length === 0 || recording === index,
                        onSelect: () => void playMacro(index),
                      },
                      {
                        id: `clear-${index}`,
                        label: 'Clear',
                        detail: 'Remove every step',
                        tone: 'danger',
                        disabled: macro.steps.length === 0 || recording === index,
                        onSelect: () => void clearMacro(index),
                      },
                    ]}
                  />
                </div>
              </div>

              {macro.steps.length === 0 ? (
                <p className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-400">
                  {recording === index ? 'Recording — press keys, then Stop' : 'Empty'}
                </p>
              ) : (
                <ol className="flex flex-col gap-1.5">
                  {macro.steps.map((step, s) => (
                    <StepRow
                      key={s}
                      ordinal={s + 1}
                      step={step}
                      onPickKey={() => setPick({ macro: index, step: s })}
                      onChange={(next) => void writeStep(index, s, next)}
                    />
                  ))}
                </ol>
              )}
            </li>
          ))}
        </ul>
      )}

      {pick && (
        <KeycodePicker
          context="basic"
          layerCount={layerCount}
          currentRaw={pickerRaw}
          title="Macro step · key"
          onPick={handlePick}
          onClose={() => setPick(null)}
        />
      )}
    </Panel>
  );
}

interface StepRowProps {
  ordinal: number;
  step: MacroStep;
  onPickKey: () => void;
  onChange: (next: MacroStep) => void;
}

function StepRow({ ordinal, step, onPickKey, onChange }: StepRowProps) {
  return (
    <li className="flex flex-wrap items-center gap-2">
      <span className="w-5 text-right font-mono text-[0.65rem] text-slate-600">{ordinal}</span>
      <button
        type="button"
        onClick={onPickKey}
        className="min-w-[3rem] border border-[#26313c] bg-[#111820] px-2 py-1.5 font-mono text-xs font-medium text-slate-100 transition-colors hover:border-sky-400/60 hover:bg-sky-500/10"
      >
        {keyLabel(step.keycode)}
      </button>
      <Tooltip content={step.down ? 'Key press event' : 'Key release event'} align="left">
        <button
          type="button"
          role="switch"
          aria-label={step.down ? 'Press event' : 'Release event'}
          aria-checked={step.down}
          onClick={() => onChange({ ...step, down: !step.down })}
          className={[
            'min-h-8 w-8 border px-2 py-1 font-mono text-xs font-medium uppercase transition-colors',
            step.down
              ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-200'
              : 'border-amber-400/40 bg-amber-500/15 text-amber-200',
          ].join(' ')}
        >
          {step.down ? 'P' : 'R'}
        </button>
      </Tooltip>
      <label className="flex items-center gap-1.5 font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        delay
        <input
          type="number"
          min={0}
          max={65535}
          value={step.delayMs}
          onChange={(event) =>
            onChange({ ...step, delayMs: clampDelay(Number(event.target.value)) })
          }
          className="w-20 border border-[#26313c] bg-[#020304] px-2 py-1 text-xs text-slate-100 focus:border-sky-400 focus:outline-none"
        />
        ms
      </label>
    </li>
  );
}

/** Clamp a delay to the u16 wire range, treating a blank/NaN input as 0. */
function clampDelay(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(65535, Math.floor(value)));
}
