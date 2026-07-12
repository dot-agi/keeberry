// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useRef, useState } from 'react';
import { Devs, activeLayerList, connectionLabel, type KcpClient, type Telemetry } from '../kcp';
import { ErrorBanner, Field, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface TelemetryDashboardProps {
  client: KcpClient;
}

/** The fields needed to derive scan/report rates between two polls. */
interface Counters {
  uptimeMs: number;
  scanCount: number;
  reportCount: number;
}

interface Rates {
  scanHz: number;
  reportHz: number;
}

const POLL_MS = 500;

/** Format a millisecond uptime as `1d 02h 03m 04s` (dropping leading zeroes). */
function formatUptime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const parts: string[] = [];
  if (days) parts.push(`${days}d`);
  if (days || hours) parts.push(`${hours.toString().padStart(2, '0')}h`);
  parts.push(`${minutes.toString().padStart(2, '0')}m`);
  parts.push(`${seconds.toString().padStart(2, '0')}s`);
  return parts.join(' ');
}

/**
 * Live telemetry dashboard: polls `getTelemetry` at ~2 Hz and derives the scan
 * and report rates from the change in the firmware's monotonic counters over
 * the device's own uptime clock (so the rate is accurate regardless of GUI
 * jitter). Shows uptime, the derived rates, last-iteration latency, the active
 * layer mask, battery and the connection.
 */
export function TelemetryDashboard({ client }: TelemetryDashboardProps) {
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [rates, setRates] = useState<Rates | null>(null);
  const [error, setError] = useState<string | null>(null);
  const prev = useRef<Counters | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    prev.current = null;

    const tick = async () => {
      try {
        const t = await client.getTelemetry();
        if (cancelled) return;
        const last = prev.current;
        if (last && t.uptimeMs > last.uptimeMs) {
          const dt = (t.uptimeMs - last.uptimeMs) / 1000;
          setRates({
            scanHz: (t.scanCount - last.scanCount) / dt,
            reportHz: (t.reportCount - last.reportCount) / dt,
          });
        }
        prev.current = {
          uptimeMs: t.uptimeMs,
          scanCount: t.scanCount,
          reportCount: t.reportCount,
        };
        setTelemetry(t);
        setError(null);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      } finally {
        if (!cancelled) timer = setTimeout(() => void tick(), POLL_MS);
      }
    };

    void tick();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [client]);

  const activeLayers = telemetry ? activeLayerList(telemetry.activeLayers) : [];

  const fields: { label: string; value: string }[] = telemetry
    ? [
        { label: 'Uptime', value: formatUptime(telemetry.uptimeMs) },
        {
          label: 'Input cadence',
          value: rates ? `${Math.round(rates.scanHz)} Hz` : `~${telemetry.scanRateHz} Hz`,
        },
        { label: 'Output cadence', value: rates ? `${Math.round(rates.reportHz)} Hz` : '—' },
        { label: 'Response window', value: `${telemetry.lastProcUs} µs` },
        {
          label: 'Active layers',
          value: activeLayers.length ? activeLayers.join(', ') : 'none',
        },
        {
          label: 'Battery',
          // A wired board reports a placeholder 100% with no radio, so show n/a
          // on the USB transport rather than a misleading full charge.
          value:
            telemetry.battery === null
              ? 'n/a'
              : telemetry.connection === Devs.Usb
                ? 'n/a (wired)'
                : `${telemetry.battery}%`,
        },
        { label: 'Connection', value: connectionLabel(telemetry.connection) },
      ]
    : [];

  return (
    <Panel
      title="Live status"
      headerExtra={
        <span
          className={[
            'inline-flex items-center gap-2 font-mono text-xs uppercase',
            error ? 'text-amber-300' : 'text-slate-400',
          ].join(' ')}
        >
          <span
            className={[
              'h-1.5 w-1.5',
              error ? 'bg-amber-400' : 'animate-pulse bg-emerald-400',
            ].join(' ')}
          />
          {error ? 'attention' : telemetry ? 'live' : 'syncing'}
        </span>
      }
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {telemetry ? (
        <dl className="kb-field-grid sm:grid-cols-3">
          {fields.map((field) => (
            <Field key={field.label} label={field.label} value={field.value} />
          ))}
        </dl>
      ) : (
        !error && <p className="text-sm text-slate-400">Reading live status…</p>
      )}
    </Panel>
  );
}
