// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import type { AutocorrectInfo, KcpClient } from '../kcp';
import { ErrorBanner, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface TextPanelProps {
  client: KcpClient;
}

/**
 * Text-input control (the TEXT group): the autocorrect toggle. The firmware matches
 * a rolling buffer of typed letters against a compiled-in typo→correction dictionary
 * and, on a whole-word match, injects backspaces + the correction. The panel reflects
 * AUTOCORRECT_INFO on load (the enable flag and the compiled-in dictionary size) and
 * writes AUTOCORRECT_SET live; the change applies immediately and is persisted into the
 * config blob by the next save.
 */
export function TextPanel({ client }: TextPanelProps) {
  const [info, setInfo] = useState<AutocorrectInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const value = await client.getAutocorrect();
        if (!cancelled) setInfo(value);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function choose(next: boolean) {
    if (!info || next === info.enabled) return;
    const previous = info;
    setInfo({ ...info, enabled: next });
    setError(null);
    setBusy(true);
    try {
      await client.setAutocorrect(next);
    } catch (err) {
      setError(friendlyPanelError(err));
      setInfo(previous);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel
      title="Autocorrect"
      description={
        info
          ? `Fixes ${info.entryCount} common typos as you type, the moment a word is finished.`
          : 'Fixes common typos as you type, the moment a word is finished.'
      }
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {info === null ? (
        !error && <p className="text-sm text-slate-400">Reading autocorrect state…</p>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="inline-grid w-fit grid-cols-2 border border-[#26313c] bg-[#020304] p-1">
            <ModeButton
              label="Off"
              active={!info.enabled}
              disabled={busy}
              onClick={() => void choose(false)}
            />
            <ModeButton
              label="On"
              active={info.enabled}
              disabled={busy}
              onClick={() => void choose(true)}
            />
          </div>
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-400">
            {info.enabled ? 'autocorrect on' : 'autocorrect off'} / {info.entryCount} entries
          </span>
        </div>
      )}
    </Panel>
  );
}

interface ModeButtonProps {
  label: string;
  active: boolean;
  disabled: boolean;
  onClick: () => void;
}

function ModeButton({ label, active, disabled, onClick }: ModeButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      aria-pressed={active}
      onClick={onClick}
      className={[
        'min-h-8 px-4 py-1.5 font-mono text-xs font-semibold uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-50',
        active ? 'bg-sky-500/20 text-sky-100' : 'text-slate-400 hover:bg-[#111820]',
      ].join(' ')}
    >
      {label}
    </button>
  );
}
