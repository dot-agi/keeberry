// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import { KcpProtocolError, Status, type FeatureRecord, type KcpClient } from '../kcp';
import { ErrorBanner, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface FeaturesPanelProps {
  client: KcpClient;
}

/**
 * Features control (the FEATURES group): a master on/off switch for every firmware
 * feature, rendered purely from the device's own enumeration — there is no per-feature
 * code here. On load the panel lists every registered feature (GET_FEATURES) and draws
 * one toggle each; a flip writes SET_FEATURE_ENABLED live (persisted by the next save) with
 * an optimistic update that reverts on error. Structural features are always on: the
 * firmware refuses to switch them off (BadArg), so the toggle reverts and the panel says so.
 */
export function FeaturesPanel({ client }: FeaturesPanelProps) {
  const [features, setFeatures] = useState<FeatureRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await client.listFeatures();
        if (!cancelled) setFeatures(list);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function toggle(feature: FeatureRecord, next: boolean) {
    if (next === feature.enabled || !features) return;
    const previous = features;
    setFeatures(features.map((f) => (f.id === feature.id ? { ...f, enabled: next } : f)));
    setError(null);
    setBusyId(feature.id);
    try {
      await client.setFeatureEnabled(feature.id, next);
    } catch (err) {
      setFeatures(previous);
      // An always-on (structural) feature refuses to switch off (BadArg); explain rather
      // than surface the raw protocol error.
      if (err instanceof KcpProtocolError && err.status === Status.BadArg) {
        setError(`${feature.name} is always on and can't be turned off.`);
      } else {
        setError(friendlyPanelError(err));
      }
    } finally {
      setBusyId(null);
    }
  }

  return (
    <Panel
      title="Features"
      description="Switch any firmware feature on or off. Changes apply immediately and are kept on the next save."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {features === null ? (
        !error && <p className="text-sm text-slate-400">Reading features…</p>
      ) : features.length === 0 ? (
        <p className="text-sm text-slate-400">This board reports no features.</p>
      ) : (
        <div className="flex flex-col gap-2">
          {features.map((feature) => (
            <FeatureRow
              key={feature.id}
              feature={feature}
              busy={busyId === feature.id}
              onChange={(next) => void toggle(feature, next)}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

interface FeatureRowProps {
  feature: FeatureRecord;
  busy: boolean;
  onChange: (on: boolean) => void;
}

/** One feature's label plus an Off/On segmented control, matching the other panels' style. */
function FeatureRow({ feature, busy, onChange }: FeatureRowProps) {
  return (
    <div className="flex items-center justify-between gap-3 border border-[#1b2530] bg-[#020304] px-3 py-2">
      <span className="font-mono text-xs uppercase tracking-wide text-slate-300">{feature.name}</span>
      <div className="inline-grid grid-cols-2 border border-[#26313c] bg-[#020304] p-1">
        {[
          { value: false, text: 'Off' },
          { value: true, text: 'On' },
        ].map(({ value, text }) => (
          <button
            key={text}
            type="button"
            aria-pressed={feature.enabled === value}
            disabled={busy}
            onClick={() => onChange(value)}
            className={[
              'min-h-8 px-4 py-1.5 font-mono text-xs font-semibold uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-50',
              feature.enabled === value
                ? 'bg-sky-500/20 text-sky-100'
                : 'text-slate-400 hover:bg-[#111820]',
            ].join(' ')}
          >
            {text}
          </button>
        ))}
      </div>
    </div>
  );
}
