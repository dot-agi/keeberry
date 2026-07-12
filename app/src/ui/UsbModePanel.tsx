// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import { type KcpClient, UsbMode, usbModeLabel } from '../kcp';
import { ErrorBanner, InfoHint, Panel, StatusBanner } from './Panel';
import { friendlyPanelError } from './panelError';

interface UsbModePanelProps {
  client: KcpClient;
  /**
   * Arm the connection state machine for the imminent re-enumeration, so the drop is
   * read as this switch's acknowledgement (the mode-specific waiting panel) rather
   * than a connection error. Called immediately before `SET_USB_MODE` is issued.
   */
  onUsbModeSwitch: (mode: UsbMode) => void;
}

/** The selectable personalities, in display order. */
const MODES: readonly UsbMode[] = [UsbMode.Normal, UsbMode.Midi, UsbMode.XInput];

/**
 * USB personality selector (the SYSTEM group's `GET_USB_MODE` / `SET_USB_MODE`).
 * The keyboard can re-enumerate as a USB-MIDI controller or an Xbox 360 (XInput)
 * gamepad — single-purpose USB devices that free the endpoints those classes need —
 * and back to the normal keyboard. MIDI keeps the kcp control interface reachable, so
 * it is reversible from here. XInput is not: once the host OS recognises the Xbox 360
 * controller it claims the whole device, kcp included, so neither this app nor kcp can
 * switch back. Leaving XInput is a firmware-side escape — hold Fn + Right Ctrl (the two
 * adjacent bottom-right keys) for about 1.5 seconds, the panel glowing red while held —
 * or an unplug/replug; either returns to Normal, since the mode is not persisted.
 *
 * The panel reflects `GET_USB_MODE` on load. Selecting a *different* personality
 * re-enumerates the device, which drops it off USB (the disconnect is the
 * acknowledgement, exactly like the maintenance resets), so the connected view
 * unmounts and the board reconnects in the chosen mode. Leaving the keyboard
 * personality is confirm-guarded since the board stops typing until it is returned.
 */
export function UsbModePanel({ client, onUsbModeSwitch }: UsbModePanelProps) {
  const [mode, setMode] = useState<UsbMode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const value = await client.getUsbMode();
        if (!cancelled) setMode(value);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function choose(next: UsbMode) {
    if (next === mode || busy) return;
    // Leaving the keyboard personality stops the board typing until it is switched
    // back, so guard it the way entering recovery is guarded.
    if (
      next !== UsbMode.Normal &&
      !window.confirm(
        `Switch to ${usbModeLabel(next)}? The keyboard will reconnect as that device and stop typing until you switch it back.`,
      )
    ) {
      return;
    }
    setError(null);
    setStatus(null);
    setBusy(true);
    try {
      // Arm the disconnect handler before issuing the switch: the re-enumeration can
      // drop the link the instant the command lands, so the state machine must
      // already know this drop is a mode switch, not an unplug.
      onUsbModeSwitch(next);
      await client.setUsbMode(next);
      // A real switch re-enumerates and disconnects (this panel then unmounts as the
      // connection moves to the 'switched' waiting state); re-selecting the current
      // mode is filtered out above, so a resolved call here means the drop is coming.
      setMode(next);
      setStatus(
        next === UsbMode.Normal
          ? 'Returning to keyboard mode… the board will reconnect shortly.'
          : `Switching to ${usbModeLabel(next)}… the board will reconnect in that mode.`,
      );
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel
      title="USB mode"
      description="Re-enumerate the keyboard as a MIDI controller or an XInput gamepad, or back to a keyboard."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {status && <StatusBanner>{status}</StatusBanner>}

      {mode === null ? (
        !error && <p className="text-sm text-slate-400">Reading USB mode…</p>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="inline-grid w-fit grid-cols-3 border border-[#26313c] bg-[#020304] p-1">
            {MODES.map((m) => (
              <ModeButton
                key={m}
                label={usbModeLabel(m)}
                active={m === mode}
                disabled={busy}
                onClick={() => void choose(m)}
              />
            ))}
          </div>
          <div className="flex items-center gap-2">
            <span className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-400">
              {usbModeLabel(mode)}
            </span>
            <InfoHint label="USB mode details">
              MIDI and XInput re-enumerate the board as a single-purpose USB device. MIDI keeps the
              kcp control interface live, so you can switch back from here. XInput cannot be
              switched back from here: once the host OS recognises the Xbox 360 controller it claims
              the whole device — kcp included — on Windows, Linux and macOS alike. To leave XInput,
              hold Fn + Right Ctrl (the two adjacent bottom-right keys) for about 1.5 seconds — the
              keyboard glows solid red while both are held — or unplug and replug; either
              re-enumerates as a keyboard, since the mode isn't
              persisted. XInput is first-class on Windows and Linux; macOS has no native XInput
              driver, so for a gamepad there prefer the always-on HID gamepad (the Gamepad/Joystick
              keycodes) instead.
            </InfoHint>
          </div>
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
