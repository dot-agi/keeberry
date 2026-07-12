// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useState } from 'react';
import {
  ConnectionState,
  Devs,
  WIRELESS_MODES,
  connectionStateLabel,
  type KcpClient,
  type WirelessState,
} from '../kcp';
import { ActionMenu, ErrorBanner, Field, InfoHint, Panel } from './Panel';
import { friendlyPanelError } from './panelError';

interface WirelessPanelProps {
  client: KcpClient;
}

const POLL_MS = 2000;

/**
 * Live wireless control: shows the active transport, connection state, battery
 * and radio firmware version (polled over WLS_GET_STATE), with buttons to switch
 * the output transport (USB / BT1-3 / 2.4 GHz), (re)pair or unpair the current
 * channel, refresh the battery reading (WLS_GET_BATTERY) and toggle the BT /
 * 2.4 GHz idle-sleep policy.
 *
 * The sleep policy is write-only on the firmware (it enqueues `DEVCTRL` frames
 * with no read-back), so its toggles reflect local intent, defaulting to the
 * power-on `enabled` state; every other field is read live from the device.
 */
export function WirelessPanel({ client }: WirelessPanelProps) {
  const [state, setState] = useState<WirelessState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sleepBt, setSleepBt] = useState(true);
  const [sleep2g4, setSleep2g4] = useState(true);

  const refresh = useCallback(async () => {
    setState(await client.wirelessGetState());
  }, [client]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const next = await client.wirelessGetState();
        if (cancelled) return;
        setState(next);
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

  /** Run a control op, then refresh the snapshot so the panel stays live. */
  const action = useCallback(
    async (run: () => Promise<void>) => {
      setBusy(true);
      setError(null);
      try {
        await run();
        await refresh();
      } catch (err) {
        setError(friendlyPanelError(err));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  /**
   * Read the battery level and trigger a fresh measurement (WLS_GET_BATTERY).
   * The reply carries the last reported level (shown at once); the enqueued
   * refresh lands on a subsequent poll. Kept off {@link action} so the snapshot
   * resync doesn't clobber the just-read level with a stale GET_STATE value.
   */
  const refreshBattery = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const level = await client.wirelessGetBattery();
      setState((prev) => (prev ? { ...prev, battery: level } : prev));
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(false);
    }
  }, [client]);

  async function applySleepPolicy(bt: boolean, g2g4: boolean) {
    setSleepBt(bt);
    setSleep2g4(g2g4);
    await action(() => client.wirelessSetSleepPolicy(bt, g2g4));
  }

  return (
    <Panel
      title="Wireless"
      description="Choose the active connection, pair channels, refresh battery, and tune idle sleep."
      headerExtra={
        state && (
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-2 font-mono text-xs uppercase text-slate-400">
              <span
                className={[
                  'h-1.5 w-1.5',
                  state.state === ConnectionState.Connected
                    ? 'animate-pulse bg-emerald-400'
                    : 'bg-slate-600',
                ].join(' ')}
              />
              {connectionStateLabel(state.state)}
            </span>
            <ActionMenu
              label="Radio"
              align="left"
              actions={[
                {
                  id: 'pair-current-mode',
                  label: 'Pair current mode',
                  detail: 'Start pairing for this channel',
                  tone: 'ok',
                  disabled: busy,
                  onSelect: () => void action(() => client.wirelessPair()),
                },
                {
                  id: 'unpair-current-mode',
                  label: 'Unpair',
                  detail: 'Forget this channel',
                  disabled: busy,
                  onSelect: () => void action(() => client.wirelessUnpair()),
                },
                {
                  id: 'refresh-battery',
                  label: 'Refresh battery',
                  detail: 'Update the battery level',
                  disabled: busy,
                  onSelect: () => void refreshBattery(),
                },
              ]}
            />
          </div>
        )
      }
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {state ? (
        <>
          <dl className="kb-field-grid mb-5 sm:grid-cols-4">
            <Field label="Mode" value={modeLabel(state.devs)} />
            <Field label="State" value={connectionStateLabel(state.state)} />
            <Field
              label="Battery"
              value={state.devs === Devs.Usb ? 'n/a (wired)' : `${state.battery}%`}
            />
            <Field label="Radio" value={`v${state.version}`} />
          </dl>

          <div className="flex flex-col gap-4">
            <div>
              <span className="mb-2 block font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                Connection mode
              </span>
              <div className="grid grid-cols-[repeat(auto-fit,minmax(5.5rem,1fr))] gap-1.5">
                {WIRELESS_MODES.map((mode) => (
                  <button
                    key={mode.code}
                    type="button"
                    disabled={busy}
                    aria-pressed={mode.code === state.devs}
                    onClick={() => void action(() => client.wirelessSetMode(mode.code))}
                    className={[
                      'min-h-9 border px-3 py-2 font-mono text-xs font-semibold uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-50',
                      mode.code === state.devs
                        ? 'border-sky-400 bg-sky-500/20 text-sky-100'
                        : 'border-[#26313c] bg-[#020304] text-slate-400 hover:border-sky-400/50 hover:bg-[#111820]',
                    ].join(' ')}
                  >
                    {mode.label}
                  </button>
                ))}
              </div>
            </div>

            <div className="border-t border-[#1b222a] pt-5">
              <div className="mb-2 flex items-center gap-2">
                <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
                  Idle-sleep policy
                </span>
                <InfoHint label="Idle sleep details">
                  Sleep settings are applied when changed and kept for this session.
                </InfoHint>
              </div>
              <div className="flex flex-wrap gap-2">
                <Toggle
                  label="Bluetooth"
                  on={sleepBt}
                  disabled={busy}
                  onClick={() => void applySleepPolicy(!sleepBt, sleep2g4)}
                />
                <Toggle
                  label="2.4 GHz"
                  on={sleep2g4}
                  disabled={busy}
                  onClick={() => void applySleepPolicy(sleepBt, !sleep2g4)}
                />
              </div>
            </div>
          </div>
        </>
      ) : (
        !error && <p className="text-sm text-slate-400">Checking wireless state…</p>
      )}
    </Panel>
  );
}

function modeLabel(devs: number): string {
  return WIRELESS_MODES.find((m) => m.code === devs)?.label ?? 'Unknown';
}

interface ToggleProps {
  label: string;
  on: boolean;
  disabled: boolean;
  onClick: () => void;
}

function Toggle({ label, on, disabled, onClick }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={disabled}
      onClick={onClick}
      className={[
        'inline-flex min-h-8 items-center gap-2 border px-3 py-1.5 font-mono text-xs font-semibold uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-50',
        on
          ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-200'
          : 'border-[#26313c] bg-[#020304] text-slate-400',
      ].join(' ')}
    >
      <span className={['h-1.5 w-1.5', on ? 'bg-emerald-400' : 'bg-slate-600'].join(' ')} />
      {label} {on ? 'on' : 'off'}
    </button>
  );
}
