// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import type { KcpClient } from '../kcp';
import { ErrorBanner, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface HidKroPanelProps {
  client: KcpClient;
}

/**
 * Rollover-mode control (the HID_KRO group): a single toggle between boot 6-key
 * rollover (the default, for maximum BIOS/compatibility) and full N-key
 * rollover. The panel reflects GET_KRO on load and writes SET_KRO live; the
 * change takes effect on the next report and resets to the firmware default
 * (6KRO) on reboot until persistence folds it into the config blob.
 */
export function HidKroPanel({ client }: HidKroPanelProps) {
  const [nkro, setNkro] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const value = await client.getKro();
        if (!cancelled) setNkro(value);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function choose(next: boolean) {
    if (next === nkro) return;
    const previous = nkro;
    setNkro(next);
    setError(null);
    setBusy(true);
    try {
      await client.setKro(next);
    } catch (err) {
      setError(friendlyPanelError(err));
      setNkro(previous);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel
      title="Rollover"
      description={
        nkro
          ? 'Maximum rollover keeps simultaneous key presses active.'
          : 'Compatibility mode works best with setup screens, older hosts, and switching hardware.'
      }
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {nkro === null ? (
        !error && <p className="text-sm text-slate-400">Reading rollover mode…</p>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="inline-grid w-fit grid-cols-2 border border-[#26313c] bg-[#020304] p-1">
            <ModeButton
              label="Compatible"
              active={!nkro}
              disabled={busy}
              onClick={() => void choose(false)}
            />
            <ModeButton
              label="Maximum"
              active={nkro}
              disabled={busy}
              onClick={() => void choose(true)}
            />
          </div>
          <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-400">
            {nkro ? 'maximum rollover' : 'compatibility mode'}
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
