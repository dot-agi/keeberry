// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  Devs,
  UsbMode,
  usbModeLabel,
  type GroupName,
  type NativeHidDevice,
  type Telemetry,
} from './kcp';
import { BehaviorsPanel } from './ui/BehaviorsPanel';
import { ConfigPanel } from './ui/ConfigPanel';
import { BoardStatusCard } from './ui/BoardStatusCard';
import { FeaturesPanel } from './ui/FeaturesPanel';
import { HidKroPanel } from './ui/HidKroPanel';
import { KeymapEditor } from './ui/KeymapEditor';
import { MacroPanel } from './ui/MacroPanel';
import { RgbPanel } from './ui/RgbPanel';
import { SystemPanel } from './ui/SystemPanel';
import { TelemetryDashboard } from './ui/TelemetryDashboard';
import { TextPanel } from './ui/TextPanel';
import { TuningPanel } from './ui/TuningPanel';
import { UnicodePanel } from './ui/UnicodePanel';
import { UsbModePanel } from './ui/UsbModePanel';
import { WirelessPanel } from './ui/WirelessPanel';
import { DescriptorPanel } from './ui/DescriptorPanel';
import { descriptorGroup, featureDescriptors } from './featureDescriptors';
import { useKcpDevice, type ConnectionState, type DeviceSnapshot } from './ui/useKcpDevice';
import { useFirmwareFlash } from './ui/useFirmwareFlash';
import type { BundledFirmware, FlashProgress } from './ui/nativeFlash';
import { ActionMenu } from './ui/Panel';
import {
  Activity,
  BatteryFull,
  BatteryLow,
  BatteryMedium,
  BatteryWarning,
  Cable,
  CheckCircle2,
  Keyboard,
  ListTree,
  Power,
  Radio,
  Save,
  Search,
  Settings,
  SlidersHorizontal,
  Sparkles,
  SquareTerminal,
  ToggleRight,
  Unplug,
  Usb,
  X,
  type LucideIcon,
} from 'lucide-react';

interface PageSpec {
  id: string;
  groups: readonly GroupName[];
  label: string;
  short: string;
  accent: 'blue' | 'green' | 'yellow' | 'violet';
  icon: LucideIcon;
}

const PAGES: PageSpec[] = [
  {
    id: 'keymap',
    groups: ['keymap'],
    label: 'Keymap',
    short: 'KM',
    accent: 'blue',
    icon: Keyboard,
  },
  { id: 'rgb', groups: ['rgb'], label: 'Lighting', short: 'LT', accent: 'violet', icon: Sparkles },
  { id: 'macro', groups: ['macro'], label: 'Macro', short: 'MC', accent: 'yellow', icon: ListTree },
  {
    id: 'advanced',
    groups: ['hidKro', 'behavior', 'wireless', 'text', 'unicode'],
    label: 'Advanced',
    short: 'AD',
    accent: 'yellow',
    icon: SlidersHorizontal,
  },
  // The Features page is driven entirely by the device's FEATURES enumeration: one
  // master on/off switch per registered feature, auto-rendered with no per-feature code.
  {
    id: 'features',
    groups: ['features'],
    label: 'Features',
    short: 'FT',
    accent: 'green',
    icon: ToggleRight,
  },
  // USB mode is its own page rather than a panel buried inside Advanced: the
  // keyboard/MIDI/XInput selector is a top-level personality switch, so it gets a
  // first-class nav entry (and an "Open USB mode" palette command) to stay findable.
  {
    id: 'usb',
    groups: ['system'],
    label: 'USB mode',
    short: 'USB',
    accent: 'green',
    icon: Usb,
  },
  {
    id: 'settings',
    groups: ['config', 'system'],
    label: 'Settings',
    short: 'ST',
    accent: 'blue',
    icon: Settings,
  },
];

interface CommandAction {
  id: string;
  group: string;
  label: string;
  detail: string;
  hint: string;
  icon: LucideIcon;
  keywords?: string[];
  disabled?: boolean;
  run: () => void;
}

type ConnectedClient = NonNullable<ReturnType<typeof useKcpDevice>['client']>;

const COMMAND_USAGE_STORAGE = 'keeberry.command-palette.usage.v1';
const COMMAND_SEARCH_STORAGE = 'keeberry.command-palette.searches.v1';

export default function App() {
  const {
    state,
    snapshot,
    client,
    error,
    clearError,
    restoreToast,
    dismissRestoreToast,
    connect,
    disconnect,
    devices,
    selectDevice,
    switchedMode,
    reconnect,
    beginUsbModeSwitch,
  } = useKcpDevice();
  const connecting = state === 'connecting';
  // The flash round-trip lives here (not in SystemPanel): entering DFU disconnects
  // the device and unmounts the panels, so the flow, its progress and the
  // post-flash reconnect must survive at the App level.
  const flash = useFirmwareFlash(connect);
  const dockInspector = useMediaQuery('(min-width: 1280px)');
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [pageRailExpanded, setPageRailExpanded] = useState(false);
  const [activePageId, setActivePageId] = useState(PAGES[0].id);
  const activePage = useMemo(() => findPage(activePageId) ?? PAGES[0], [activePageId]);

  useEffect(() => {
    if (window.location.search) {
      window.history.replaceState(null, document.title, window.location.pathname || '/');
    }
  }, []);

  useEffect(() => {
    if (state !== 'connected' || !snapshot) return;
    if (isPageAvailable(snapshot, activePage)) return;

    const nextPage = firstAvailablePage(snapshot);
    if (nextPage) setActivePageId(nextPage.id);
  }, [activePage, snapshot, state]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const commandActions = useMemo<CommandAction[]>(() => {
    const canUseDevice =
      state !== 'unsupported' && state !== 'connected' && state !== 'selecting' && !connecting;
    const pageActions = PAGES.map((page) => ({
      id: `open-${page.id}`,
      group: 'Pages',
      label: `Open ${page.label}`,
      detail:
        snapshot && isPageAvailable(snapshot, page)
          ? `Open the ${page.label.toLowerCase()} command page`
          : `${page.label} is waiting for a compatible board`,
      hint: page.short,
      icon: page.icon,
      keywords: [page.label, page.id, ...page.groups],
      disabled: !snapshot || !isPageAvailable(snapshot, page),
      run: () => setActivePageId(page.id),
    }));
    const canOpen = (id: string) => {
      const page = findPage(id);
      return Boolean(snapshot && page && isPageAvailable(snapshot, page));
    };
    const openPage = (id: string) => () => setActivePageId(id);

    return [
      {
        id: 'connect',
        group: 'Keyboard',
        label: 'Connect keyboard',
        detail: 'Choose a keyboard to control',
        hint: 'Enter',
        icon: Cable,
        keywords: ['pair', 'dock', 'usb', 'hid'],
        disabled: !canUseDevice,
        run: () => void connect(),
      },
      {
        id: 'disconnect',
        group: 'Keyboard',
        label: 'Disconnect keyboard',
        detail: 'Release the keyboard from this session',
        hint: 'D',
        icon: Unplug,
        keywords: ['undock', 'release', 'stop'],
        disabled: state !== 'connected',
        run: () => void disconnect(),
      },
      {
        id: 'save-settings',
        group: 'Keyboard',
        label: 'Save settings',
        detail: 'Open settings controls',
        hint: 'S',
        icon: Save,
        keywords: ['persist', 'flash', 'write', 'config'],
        disabled:
          !snapshot ||
          (!snapshot.capabilities.groups.config && !snapshot.capabilities.groups.system),
        run: () => setActivePageId('settings'),
      },
      {
        id: 'search-keys',
        group: 'Keymap',
        label: 'Search keys',
        detail: 'Open the keymap and choose bindings',
        hint: 'K',
        icon: Search,
        keywords: ['keycode', 'binding', 'layout', 'layer'],
        disabled: !snapshot?.capabilities.groups.keymap,
        run: () => setActivePageId('keymap'),
      },
      {
        id: 'open-macros',
        group: 'Keymap',
        label: 'Search macros',
        detail: 'Open macro slots to upload and play key steps',
        hint: 'M',
        icon: ListTree,
        keywords: ['upload', 'play', 'sequence', 'slot'],
        disabled: !snapshot?.capabilities.groups.macro,
        run: () => setActivePageId('macro'),
      },
      {
        id: 'keymap-layers',
        group: 'Keymap',
        label: 'Layer controls',
        detail: 'Switch layers and inspect key positions',
        hint: 'LYR',
        icon: Keyboard,
        keywords: ['layer', 'base', 'fn', 'matrix'],
        disabled: !canOpen('keymap'),
        run: openPage('keymap'),
      },
      {
        id: 'lighting-effects',
        group: 'Lighting',
        label: 'Lighting effects',
        detail: 'Open effect, color, and brightness controls',
        hint: 'RGB',
        icon: Sparkles,
        keywords: ['rgb', 'effect', 'brightness', 'color', 'hue'],
        disabled: !canOpen('rgb'),
        run: openPage('rgb'),
      },
      {
        id: 'advanced-rollover',
        group: 'Advanced',
        label: 'Rollover and HID mode',
        detail: 'Open keyboard report and rollover controls',
        hint: 'HID',
        icon: SlidersHorizontal,
        keywords: ['nkro', '6kro', 'hid', 'report'],
        disabled: !canOpen('advanced') || !snapshot?.capabilities.groups.hidKro,
        run: openPage('advanced'),
      },
      {
        id: 'advanced-behavior',
        group: 'Advanced',
        label: 'Behavior features',
        detail: 'Open behavior tuning and advanced feature controls',
        hint: 'ADV',
        icon: SlidersHorizontal,
        keywords: ['socd', 'behavior', 'rapid', 'features'],
        disabled: !canOpen('advanced') || !snapshot?.capabilities.groups.behavior,
        run: openPage('advanced'),
      },
      {
        id: 'advanced-wireless',
        group: 'Advanced',
        label: 'Wireless controls',
        detail: 'Open radio, pairing, and power controls',
        hint: 'RAD',
        icon: Radio,
        keywords: ['radio', 'pair', 'battery', 'wireless'],
        disabled: !canOpen('advanced') || !snapshot?.capabilities.groups.wireless,
        run: openPage('advanced'),
      },
      {
        id: 'advanced-text',
        group: 'Advanced',
        label: 'Autocorrect',
        detail: 'Open the autocorrect toggle',
        hint: 'TXT',
        icon: SlidersHorizontal,
        keywords: ['autocorrect', 'typo', 'text', 'spelling'],
        disabled: !canOpen('advanced') || !snapshot?.capabilities.groups.text,
        run: openPage('advanced'),
      },
      {
        id: 'advanced-unicode',
        group: 'Advanced',
        label: 'Unicode input',
        detail: 'Open OS input mode and the codepoint map',
        hint: 'UNI',
        icon: SlidersHorizontal,
        keywords: ['unicode', 'codepoint', 'emoji', 'hex', 'ibus', 'wincompose'],
        disabled: !canOpen('advanced') || !snapshot?.capabilities.groups.unicode,
        run: openPage('advanced'),
      },
      {
        id: 'usb-mode',
        group: 'USB mode',
        label: 'Output mode (keyboard / MIDI / XInput)',
        detail: 'Re-enumerate as a keyboard, MIDI controller, or XInput gamepad',
        hint: 'USB',
        icon: Usb,
        keywords: ['usb', 'midi', 'xinput', 'gamepad', 'controller', 'normal', 'personality'],
        disabled: !canOpen('usb'),
        run: openPage('usb'),
      },
      {
        id: 'settings-backup',
        group: 'Settings',
        label: 'Save and restore',
        detail: 'Open saved setup and restore controls',
        hint: 'BKP',
        icon: Save,
        keywords: ['backup', 'restore', 'config', 'profile'],
        disabled: !canOpen('settings') || !snapshot?.capabilities.groups.config,
        run: openPage('settings'),
      },
      {
        id: 'settings-update',
        group: 'Settings',
        label: 'Firmware update',
        detail: 'Open update package and recovery controls',
        hint: 'UPD',
        icon: Power,
        keywords: ['firmware', 'update', 'flash', 'recovery', 'bootloader'],
        disabled: !canOpen('settings') || !snapshot?.capabilities.groups.system,
        run: openPage('settings'),
      },
      ...pageActions,
    ];
  }, [connect, connecting, disconnect, snapshot, state]);

  const pageCount = snapshot ? PAGES.filter((page) => isPageAvailable(snapshot, page)).length : 0;

  return (
    <div className="kb-shell isolate min-h-screen bg-[#000102] text-slate-100 md:grid md:h-screen md:grid-rows-[auto_minmax(0,1fr)_auto] md:overflow-hidden">
      <TopProtocolBar
        state={state}
        snapshot={snapshot}
        client={state === 'connected' ? client : null}
        connecting={connecting}
        flashBusy={flash.busy}
        activePage={state === 'connected' ? activePage : null}
        onConnect={() => void connect()}
        onDisconnect={() => void disconnect()}
        onOpenPage={setActivePageId}
        onOpenPalette={() => setPaletteOpen(true)}
      />

      <div className="grid min-h-0 grid-cols-1 border-b border-[#121820] md:grid-cols-[4.75rem_minmax(0,1fr)] md:overflow-x-visible md:overflow-y-hidden xl:grid-cols-[4.75rem_minmax(0,1fr)_21rem]">
        <PageRail
          pages={PAGES}
          snapshot={snapshot}
          connected={state === 'connected'}
          activePageId={activePage.id}
          railExpanded={pageRailExpanded}
          onSelect={setActivePageId}
          onExpandedChange={setPageRailExpanded}
        />

        <main className="min-h-0 overflow-y-auto bg-[linear-gradient(180deg,#010203_0%,#000102_100%)]">
          <div className="mx-auto flex w-full max-w-[118rem] flex-col gap-4 px-4 py-4 sm:px-5 lg:px-6">
            <WorkbenchHeader
              state={state}
              snapshot={snapshot}
              pageCount={pageCount}
              activePage={state === 'connected' ? activePage : null}
            />

            <StatusStack>
              {flash.progress && (
                <FlashProgressBanner
                  progress={flash.progress}
                  busy={flash.busy}
                  onDismiss={() => {
                    // A flash that fails after DFU entry leaves a stale "Device
                    // disconnected." connection error behind this banner; clear it on
                    // dismiss so it does not resurface once the banner is gone.
                    flash.dismissProgress();
                    clearError();
                  }}
                />
              )}

              {state === 'unsupported' && (
                <Notice tone="warning" title="Keyboard connection unavailable">
                  This app cannot connect directly to the keyboard from the current session. Open a
                  supported desktop or browser session and try again.
                </Notice>
              )}

              {/* A flash enters DFU on purpose, so the disconnect it causes is expected:
                  the progress banner stands in for it, so suppress the generic error. */}
              {error && state !== 'unsupported' && !flash.progress && (
                <Notice tone="error" title="Connection problem">
                  {error}
                </Notice>
              )}

              {restoreToast && (
                <Toast tone={restoreToast.tone} onDismiss={dismissRestoreToast}>
                  {restoreToast.message}
                </Toast>
              )}
            </StatusStack>

            {!dockInspector && (
              <InspectorTray
                state={state}
                snapshot={snapshot}
                client={state === 'connected' ? client : null}
                error={error}
                progress={flash.progress}
                restoreToast={restoreToast ? 'restore point ready' : null}
              />
            )}

            {state === 'connected' && snapshot && client ? (
              <ConnectedWorkbench
                snapshot={snapshot}
                client={client}
                flash={flash}
                activePage={activePage}
                onUsbModeSwitch={beginUsbModeSwitch}
              />
            ) : state === 'switched' && switchedMode !== null ? (
              <SwitchedModeWorkbench
                mode={switchedMode}
                onReconnect={() => void reconnect()}
                onCancel={() => void disconnect()}
              />
            ) : state === 'selecting' && devices ? (
              <DevicePicker devices={devices} onSelect={(device) => void selectDevice(device)} />
            ) : (
              state !== 'unsupported' && (
                <IdleWorkbench
                  native={flash.native}
                  bundled={flash.bundled}
                  busy={flash.busy}
                  connecting={connecting}
                  onConnect={() => void connect()}
                  onFlash={() => void flash.flashInBootloader()}
                  onReboot={() => void flash.rebootToKeyboard()}
                  onOpenPalette={() => setPaletteOpen(true)}
                />
              )
            )}
          </div>
        </main>

        {dockInspector && (
          <Inspector
            state={state}
            snapshot={snapshot}
            client={state === 'connected' ? client : null}
            error={error}
            progress={flash.progress}
            restoreToast={restoreToast ? 'restore point ready' : null}
          />
        )}
      </div>

      <BottomStatusStrip state={state} snapshot={snapshot} flashBusy={flash.busy} />

      <CommandPalette
        open={paletteOpen}
        actions={commandActions}
        onClose={() => setPaletteOpen(false)}
      />
    </div>
  );
}

function findPage(id: string): PageSpec | undefined {
  return PAGES.find((page) => page.id === id);
}

function firstAvailablePage(snapshot: DeviceSnapshot): PageSpec | undefined {
  return PAGES.find((page) => isPageAvailable(snapshot, page));
}

function isPageAvailable(snapshot: DeviceSnapshot, page: PageSpec): boolean {
  return page.groups.some((group) => snapshot.capabilities.groups[group]);
}

function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window === 'undefined' ? false : window.matchMedia(query).matches,
  );

  useEffect(() => {
    const media = window.matchMedia(query);
    setMatches(media.matches);

    const onChange = () => setMatches(media.matches);
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, [query]);

  return matches;
}

interface TopProtocolBarProps {
  state: ConnectionState;
  snapshot: DeviceSnapshot | null;
  client: ConnectedClient | null;
  connecting: boolean;
  flashBusy: boolean;
  activePage: PageSpec | null;
  onConnect: () => void;
  onDisconnect: () => void;
  onOpenPage: (id: string) => void;
  onOpenPalette: () => void;
}

function TopProtocolBar({
  state,
  snapshot,
  client,
  connecting,
  flashBusy,
  activePage,
  onConnect,
  onDisconnect,
  onOpenPage,
  onOpenPalette,
}: TopProtocolBarProps) {
  const connected = state === 'connected';
  const stateLabel = flashBusy ? 'updating' : connectionDisplayLabel(state);

  return (
    <header className="grid min-h-16 grid-cols-1 items-center gap-3 border-b border-[#121820] bg-[linear-gradient(180deg,#010203_0%,#000102_100%)] px-4 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:px-5 lg:px-6">
      <div className="flex min-w-0 items-center gap-3">
        <div className="grid h-10 w-10 place-items-center border border-cyan-300/50 bg-cyan-400/10 text-cyan-200 shadow-[0_0_18px_rgba(34,211,238,0.22)]">
          <Keyboard className="h-5 w-5" aria-hidden />
        </div>
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <h1 className="truncate text-sm font-semibold uppercase tracking-[0.22em] text-slate-50">
              keeberry control
            </h1>
            <span className="border border-[#202b36] px-2 py-0.5 font-mono text-[0.65rem] uppercase text-slate-400">
              page:{activePage?.label ?? 'standby'}
            </span>
          </div>
          <p className="mt-1 truncate font-mono text-[0.7rem] text-slate-400">
            {snapshot ? 'keyboard docked' : 'keyboard bay standing by'}
          </p>
        </div>
      </div>

      <div className="flex min-w-0 flex-wrap items-center justify-start gap-2 sm:justify-end">
        {connected && client && snapshot?.capabilities.groups.telemetry && (
          <HeaderBattery client={client} />
        )}
        {connected ? (
          <button
            type="button"
            onClick={onDisconnect}
            disabled={flashBusy}
            aria-label="Disconnect keyboard"
            className="kb-control kb-control-connected font-mono text-xs uppercase"
          >
            <Unplug className="h-3.5 w-3.5" aria-hidden />
            <span>{flashBusy ? 'Updating' : 'Disconnect'}</span>
          </button>
        ) : (
          <StatusPill tone={connecting || flashBusy ? 'busy' : 'idle'}>{stateLabel}</StatusPill>
        )}
        <button
          type="button"
          onClick={onOpenPalette}
          className="kb-control kb-control-sm font-mono text-xs text-slate-300"
        >
          <Search className="h-4 w-4 text-sky-300" aria-hidden />
          <span>Commands</span>
        </button>
        {connected ? (
          <ActionMenu
            label="Board"
            actions={[
              ...PAGES.map((page) => ({
                id: `board-${page.id}`,
                label: page.label,
                detail: `Open ${page.label.toLowerCase()} controls`,
                hint: page.short,
                disabled: !snapshot || !isPageAvailable(snapshot, page),
                onSelect: () => onOpenPage(page.id),
              })),
              {
                id: 'board-maintenance',
                label: 'Maintenance tools',
                detail: 'Saved setup, updates, and recovery',
                hint: 'SET',
                disabled:
                  !snapshot?.capabilities.groups.config && !snapshot?.capabilities.groups.system,
                onSelect: () => onOpenPage('settings'),
              },
            ]}
          />
        ) : (
          <button
            type="button"
            onClick={onConnect}
            disabled={state === 'unsupported' || state === 'selecting' || connecting}
            className="kb-control kb-control-primary text-xs"
          >
            <Cable className="h-4 w-4" aria-hidden />
            {connecting
              ? 'Connecting'
              : state === 'selecting'
                ? 'Select keyboard'
                : 'Connect keyboard'}
          </button>
        )}
      </div>
    </header>
  );
}

// Battery drifts over minutes, so the header gauge polls lazily — frequent enough
// to stay current without competing with the editors' sub-second telemetry polls.
const HEADER_BATTERY_POLL_MS = 15_000;

/**
 * Compact battery gauge for the header, shown only while connected. Battery exists
 * only on the radio links, so on the USB cable (transport {@link Devs.Usb}) this
 * shows a plain "USB" tag rather than the firmware's wired placeholder level; on a
 * 2.4 GHz / Bluetooth link it shows the icon and percent.
 */
function HeaderBattery({ client }: { client: ConnectedClient }) {
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const next = await client.getTelemetry();
        if (!cancelled) setTelemetry(next);
      } catch {
        // A passive header readout: a dropped poll just keeps the last value.
      } finally {
        if (!cancelled) timer = setTimeout(() => void tick(), HEADER_BATTERY_POLL_MS);
      }
    };

    void tick();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [client]);

  if (!telemetry) return null;

  if (telemetry.connection === Devs.Usb) {
    return (
      <span className="inline-flex min-h-8 items-center gap-1.5 border border-[#202b36] bg-[#030609] px-2.5 font-mono text-[0.7rem] uppercase text-slate-400">
        <Usb className="h-3.5 w-3.5 text-sky-300" aria-hidden />
        USB
      </span>
    );
  }

  if (telemetry.battery === null) return null;

  const { Icon, tint } = batteryGauge(telemetry.battery);
  return (
    <span
      className="inline-flex min-h-8 items-center gap-1.5 border border-[#202b36] bg-[#030609] px-2.5 font-mono text-[0.7rem] text-slate-300"
      title={`Battery ${telemetry.battery}%`}
      aria-label={`Battery ${telemetry.battery} percent`}
    >
      <Icon className={`h-3.5 w-3.5 ${tint}`} aria-hidden />
      {telemetry.battery}%
    </span>
  );
}

/** Pick the battery icon and tint for a charge percent (low reads red, full green). */
function batteryGauge(level: number): { Icon: LucideIcon; tint: string } {
  if (level <= 15) return { Icon: BatteryWarning, tint: 'text-red-300' };
  if (level <= 40) return { Icon: BatteryLow, tint: 'text-amber-300' };
  if (level <= 75) return { Icon: BatteryMedium, tint: 'text-emerald-300' };
  return { Icon: BatteryFull, tint: 'text-emerald-300' };
}

interface WorkbenchHeaderProps {
  state: ConnectionState;
  snapshot: DeviceSnapshot | null;
  pageCount: number;
  activePage: PageSpec | null;
}

function WorkbenchHeader({ state, snapshot, pageCount, activePage }: WorkbenchHeaderProps) {
  const ActiveIcon = activePage?.icon ?? SquareTerminal;

  return (
    <section className="grid gap-3 border border-[#121820] bg-[linear-gradient(180deg,#05080b_0%,#010203_100%)] p-3 md:grid-cols-[minmax(0,1fr)_auto]">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <ActiveIcon
            className={[
              'h-4 w-4',
              activePage ? accentText(activePage.accent) : 'text-sky-300',
            ].join(' ')}
            aria-hidden
          />
          <p className="font-mono text-[0.65rem] uppercase tracking-[0.24em] text-sky-300/80">
            {activePage ? activePage.label : 'command deck'}
          </p>
        </div>
        <h2 className="mt-1 truncate text-xl font-semibold tracking-tight text-slate-50">
          {snapshot && activePage
            ? `${activePage.label} command deck`
            : snapshot
              ? 'Keyboard command deck'
              : 'Connect a keyboard to arm controls'}
        </h2>
      </div>
      <div className="grid grid-cols-2 gap-2 text-right sm:min-w-[16rem]">
        <MetricTile
          label="state"
          value={connectionDisplayLabel(state)}
          tone={state === 'connected' ? 'ok' : 'idle'}
        />
        <MetricTile label="pages" value={String(pageCount)} tone="blue" />
      </div>
    </section>
  );
}

interface ConnectedWorkbenchProps {
  snapshot: DeviceSnapshot;
  client: ConnectedClient;
  flash: ReturnType<typeof useFirmwareFlash>;
  activePage: PageSpec;
  onUsbModeSwitch: (mode: UsbMode) => void;
}

function ConnectedWorkbench({
  snapshot,
  client,
  flash,
  activePage,
  onUsbModeSwitch,
}: ConnectedWorkbenchProps) {
  const enabled = isPageAvailable(snapshot, activePage);

  if (!enabled) return <UnavailablePage page={activePage} />;

  return (
    <div id={`page-panel-${activePage.id}`} className="flex min-h-[32rem] flex-col gap-3">
      <div className="min-w-0">
        {activePage.id === 'keymap' && (
          <KeymapEditor
            client={client}
            rows={snapshot.deviceInfo.rows}
            cols={snapshot.deviceInfo.cols}
            layerCount={snapshot.deviceInfo.layers}
          />
        )}
        {activePage.id === 'rgb' && <RgbPanel client={client} />}
        {activePage.id === 'macro' && (
          <MacroPanel client={client} layerCount={snapshot.deviceInfo.layers} />
        )}
        {activePage.id === 'advanced' && (
          <AdvancedFeaturesPage snapshot={snapshot} client={client} />
        )}
        {activePage.id === 'features' && <FeaturesPanel client={client} />}
        {activePage.id === 'usb' && (
          <UsbModePanel client={client} onUsbModeSwitch={onUsbModeSwitch} />
        )}
        {activePage.id === 'settings' && (
          <SettingsPage snapshot={snapshot} client={client} flash={flash} />
        )}
      </div>
    </div>
  );
}

/**
 * The descriptor panels to render for the current board: the render-priority's middle tier — every
 * registered FeatureDescriptor whose kcp group the device advertises, each drawn by the one generic
 * DescriptorPanel. The hand-built panels rendered above take precedence (a feature with its own
 * panel registers no descriptor), and a feature with neither a panel nor a descriptor falls through
 * to its generic FEATURES toggle. The registry ships empty, so this yields nothing until a
 * `--kind config` feature is scaffolded — the branch degrades cleanly to that toggle.
 */
function descriptorPanels(snapshot: DeviceSnapshot, client: ConnectedClient): ReactNode[] {
  return [...featureDescriptors.values()]
    .filter((descriptor) => {
      const group = descriptorGroup(descriptor);
      return group !== null && snapshot.capabilities.groups[group];
    })
    .map((descriptor) => (
      <DescriptorPanel key={descriptor.fid} descriptor={descriptor} client={client} />
    ));
}

function AdvancedFeaturesPage({
  snapshot,
  client,
}: {
  snapshot: DeviceSnapshot;
  client: ConnectedClient;
}) {
  return (
    <div className="grid gap-4">
      {snapshot.capabilities.groups.hidKro && <HidKroPanel client={client} />}
      {snapshot.capabilities.groups.behavior && (
        <BehaviorsPanel client={client} layerCount={snapshot.deviceInfo.layers} />
      )}
      {snapshot.capabilities.groups.text && <TextPanel client={client} />}
      {snapshot.capabilities.groups.wireless && <WirelessPanel client={client} />}
      {snapshot.capabilities.groups.unicode && <UnicodePanel client={client} />}
      {descriptorPanels(snapshot, client)}
    </div>
  );
}

function SettingsPage({
  snapshot,
  client,
  flash,
}: {
  snapshot: DeviceSnapshot;
  client: ConnectedClient;
  flash: ReturnType<typeof useFirmwareFlash>;
}) {
  return (
    <div className="grid gap-4">
      {snapshot.capabilities.groups.config && (
        <ConfigPanel client={client} deviceInfo={snapshot.deviceInfo} />
      )}
      {snapshot.capabilities.groups.config && <TuningPanel client={client} />}
      {snapshot.capabilities.groups.system && (
        <SystemPanel
          client={client}
          deviceInfo={snapshot.deviceInfo}
          native={flash.native}
          bundledFirmware={flash.bundled}
          flashBusy={flash.busy}
          onUpdateFirmware={() => void flash.updateFirmware(client, snapshot.deviceInfo)}
          onRebootToKeyboard={() => void flash.rebootToKeyboard()}
        />
      )}
    </div>
  );
}

function UnavailablePage({ page }: { page: PageSpec }) {
  const Icon = page.icon;

  return (
    <section className="border border-[#121820] bg-[linear-gradient(180deg,#05080b_0%,#000102_100%)] p-5">
      <div className="flex items-center gap-3">
        <span className="grid h-9 w-9 place-items-center border border-[#202b36] bg-[#000102] text-slate-600">
          <Icon className="h-4 w-4" aria-hidden />
        </span>
        <div>
          <h2 className="text-base font-semibold text-slate-200">{page.label} offline</h2>
          <p className="mt-1 text-sm text-slate-400">
            Connect a compatible board to unlock this command page.
          </p>
        </div>
      </div>
    </section>
  );
}

interface IdleWorkbenchProps {
  native: boolean;
  bundled: BundledFirmware | null;
  busy: boolean;
  connecting: boolean;
  onConnect: () => void;
  onFlash: () => void;
  onReboot: () => void;
  onOpenPalette: () => void;
}

function IdleWorkbench({
  native,
  bundled,
  busy,
  connecting,
  onConnect,
  onFlash,
  onReboot,
  onOpenPalette,
}: IdleWorkbenchProps) {
  return (
    <div className={native ? 'grid gap-4 xl:grid-cols-[minmax(0,1fr)_24rem]' : 'grid gap-4'}>
      <section className="min-h-[30rem] border border-dashed border-[#202b36] bg-[linear-gradient(180deg,#05080b_0%,#000102_100%)] p-6 lg:min-h-[34rem]">
        <div className="flex h-full flex-col justify-between gap-8">
          <div>
            <div className="flex items-center gap-2 font-mono text-[0.65rem] uppercase tracking-[0.24em] text-slate-500">
              <Power className="h-4 w-4 text-amber-300/80" aria-hidden />
              standby bay
            </div>
            <h2 className="mt-3 max-w-2xl text-3xl font-semibold tracking-tight text-slate-50">
              Dock a keyboard, then drive every control surface from this deck.
            </h2>
            <p className="mt-4 max-w-xl text-sm leading-6 text-slate-400">
              Keymap, lighting, macros, advanced controls, and settings stay separated into fast
              command pages.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={onConnect}
              disabled={connecting}
              className="kb-control kb-control-primary"
            >
              <Cable className="h-4 w-4" aria-hidden />
              {connecting ? 'Connecting' : 'Connect keyboard'}
            </button>
            <button type="button" onClick={onOpenPalette} className="kb-control">
              <Search className="h-4 w-4" aria-hidden />
              Commands
            </button>
          </div>
        </div>
      </section>
      {native && (
        <BootloaderPanel bundled={bundled} busy={busy} onFlash={onFlash} onReboot={onReboot} />
      )}
    </div>
  );
}

interface PageRailProps {
  pages: PageSpec[];
  snapshot: DeviceSnapshot | null;
  connected: boolean;
  activePageId: string;
  railExpanded: boolean;
  onSelect: (id: string) => void;
  onExpandedChange: (expanded: boolean) => void;
}

function PageRail({
  pages,
  snapshot,
  connected,
  activePageId,
  railExpanded,
  onSelect,
  onExpandedChange,
}: PageRailProps) {
  const expandTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const collapseTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);

  function clearRailTimers() {
    if (expandTimer.current) window.clearTimeout(expandTimer.current);
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
    expandTimer.current = null;
    collapseTimer.current = null;
  }

  function scheduleExpand() {
    clearRailTimers();
    expandTimer.current = window.setTimeout(() => onExpandedChange(true), 120);
  }

  function scheduleCollapse() {
    clearRailTimers();
    collapseTimer.current = window.setTimeout(() => onExpandedChange(false), 240);
  }

  useEffect(
    () => () => {
      if (expandTimer.current) window.clearTimeout(expandTimer.current);
      if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
    },
    [],
  );

  return (
    <div className="relative z-30 hidden min-h-0 w-[4.75rem] md:block">
      <nav
        className={[
          'absolute inset-y-0 left-0 flex border-r border-[#121820] bg-[linear-gradient(180deg,#010203_0%,#000102_100%)] p-2 transition-[width,box-shadow] duration-150 ease-out flex-col overflow-y-auto',
          railExpanded
            ? 'w-[11.25rem] shadow-[18px_0_32px_rgba(0,0,0,0.55)]'
            : 'w-[4.75rem] shadow-none',
        ].join(' ')}
        onMouseEnter={scheduleExpand}
        onMouseLeave={scheduleCollapse}
        onFocusCapture={() => {
          clearRailTimers();
          onExpandedChange(true);
        }}
        onBlurCapture={(event) => {
          const nextTarget = event.relatedTarget instanceof Node ? event.relatedTarget : null;
          if (!event.currentTarget.contains(nextTarget)) scheduleCollapse();
        }}
      >
        <div
          className={[
            'mb-3 flex h-12 border-b border-[#121820] pb-3',
            railExpanded ? 'items-center gap-2 px-1' : 'items-center justify-center',
          ].join(' ')}
        >
          <span className="grid h-8 w-8 shrink-0 place-items-center border border-fuchsia-300/40 bg-fuchsia-400/10 text-fuchsia-200">
            <Keyboard className="h-4 w-4" aria-hidden />
          </span>
          {railExpanded && (
            <span className="min-w-0">
              <span className="block truncate font-mono text-[0.68rem] uppercase tracking-[0.18em] text-fuchsia-300/80">
                configure
              </span>
              <span className="block truncate text-sm font-semibold text-slate-100">Pages</span>
            </span>
          )}
        </div>
        <div
          className="flex w-full gap-2 overflow-x-auto md:flex-col md:overflow-x-visible"
          aria-label="Keyboard configuration pages"
        >
          {pages.map((page) => {
            const enabled = connected && Boolean(snapshot && isPageAvailable(snapshot, page));
            const active = enabled && page.id === activePageId;
            const Icon = page.icon;
            return (
              <button
                key={page.id}
                id={`page-tab-${page.id}`}
                type="button"
                disabled={!enabled}
                aria-label={page.label}
                aria-current={active ? 'page' : undefined}
                onClick={() => {
                  if (enabled) onSelect(page.id);
                }}
                className={[
                  railExpanded
                    ? 'group grid min-h-14 w-full min-w-[3.5rem] grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 border px-2 py-2 text-left transition-colors md:min-w-0'
                    : 'group flex h-14 min-w-[3.5rem] flex-col items-center justify-center border px-2 text-center transition-colors md:min-w-0',
                  active
                    ? 'border-sky-400/70 bg-[linear-gradient(180deg,rgba(14,165,233,0.18)_0%,#000102_100%)] text-sky-100 shadow-[inset_3px_0_0_rgba(56,189,248,0.9)]'
                    : enabled
                      ? 'border-[#17202a] bg-[linear-gradient(180deg,#030507_0%,#000102_100%)] text-slate-400 hover:border-sky-400/50 hover:text-sky-100'
                      : 'cursor-default border-[#0b1117] bg-transparent text-slate-700 opacity-80',
                ].join(' ')}
              >
                <Icon
                  className={[
                    'h-4 w-4',
                    enabled
                      ? active
                        ? 'text-sky-100'
                        : accentText(page.accent)
                      : 'text-slate-700',
                  ].join(' ')}
                  aria-hidden
                />
                {railExpanded && (
                  <>
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-medium text-slate-100">
                        {page.label}
                      </span>
                      <span className="mt-0.5 block truncate font-mono text-[0.7rem] uppercase tracking-wide text-slate-500">
                        {enabled ? 'ready' : connected ? 'offline' : 'standby'}
                      </span>
                    </span>
                    {active ? (
                      <CheckCircle2 className="h-4 w-4 text-sky-300" aria-hidden />
                    ) : (
                      <span className="h-1.5 w-1.5 bg-slate-700" aria-hidden />
                    )}
                  </>
                )}
                <span
                  className={[
                    railExpanded ? 'hidden' : 'mt-2 h-0.5 w-6 bg-current',
                    active ? 'opacity-90' : 'opacity-35',
                  ].join(' ')}
                  aria-hidden
                />
              </button>
            );
          })}
        </div>
      </nav>
    </div>
  );
}

interface InspectorProps {
  state: ConnectionState;
  snapshot: DeviceSnapshot | null;
  client: ConnectedClient | null;
  error: string | null;
  progress: FlashProgress | null;
  restoreToast: string | null;
}

function Inspector({ state, snapshot, client, error, progress, restoreToast }: InspectorProps) {
  return (
    <aside className="hidden min-h-0 border-l border-[#121820] bg-[linear-gradient(180deg,#010203_0%,#000102_100%)] p-4 xl:block xl:overflow-y-auto">
      <div className="grid gap-4">
        {snapshot ? (
          <>
            <BoardStatusCard snapshot={snapshot} />
            {client && snapshot.capabilities.groups.telemetry && (
              <TelemetryDashboard client={client} />
            )}
          </>
        ) : (
          <section className="border border-[#121820] bg-[linear-gradient(180deg,#05080b_0%,#010203_100%)] p-4">
            <div className="flex items-center gap-2">
              <Radio className="h-4 w-4 text-fuchsia-300" aria-hidden />
              <h2 className="font-mono text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
                Board bay
              </h2>
            </div>
            <p className="mt-3 font-mono text-[0.65rem] uppercase tracking-wide text-slate-400">
              no board docked
            </p>
          </section>
        )}
        {!snapshot && (
          <ConnectionStatusCard
            state={state}
            error={error}
            progress={progress}
            restoreToast={restoreToast}
          />
        )}
      </div>
    </aside>
  );
}

function InspectorTray({ state, snapshot, client, error, progress, restoreToast }: InspectorProps) {
  if (!snapshot && !error && !progress && !restoreToast) return null;

  return (
    <div className="grid gap-4 xl:hidden">
      {snapshot && (
        <div className="grid gap-4 lg:grid-cols-2">
          <BoardStatusCard snapshot={snapshot} />
          {client && snapshot.capabilities.groups.telemetry && (
            <TelemetryDashboard client={client} />
          )}
        </div>
      )}
      {!snapshot && (
        <ConnectionStatusCard
          state={state}
          error={error}
          progress={progress}
          restoreToast={restoreToast}
        />
      )}
    </div>
  );
}

function ConnectionStatusCard({
  state,
  error,
  progress,
  restoreToast,
}: {
  state: ConnectionState;
  error: string | null;
  progress: FlashProgress | null;
  restoreToast: string | null;
}) {
  const lines = [
    `mode: ${connectionDisplayLabel(state)}`,
    progress ? `update: ${progressMessage(progress)}` : null,
    restoreToast ? 'restore: complete' : null,
    error ? 'attention: check connection' : 'attention: clear',
  ].filter(Boolean);

  return (
    <section className="border border-[#121820] bg-[linear-gradient(180deg,#020407_0%,#000102_100%)] p-4">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <SquareTerminal className="h-4 w-4 text-emerald-300" aria-hidden />
          <h2 className="font-mono text-xs font-semibold uppercase tracking-[0.2em] text-slate-400">
            Connection
          </h2>
        </div>
        <span className="h-2 w-2 bg-emerald-400 shadow-[0_0_16px_rgba(52,211,153,0.8)]" />
      </div>
      <pre className="max-h-72 overflow-auto whitespace-pre-wrap font-mono text-[0.72rem] leading-5 text-slate-400">
        {lines.join('\n')}
      </pre>
    </section>
  );
}

interface BottomStatusStripProps {
  state: ConnectionState;
  snapshot: DeviceSnapshot | null;
  flashBusy: boolean;
}

function BottomStatusStrip({ state, snapshot, flashBusy }: BottomStatusStripProps) {
  return (
    <footer className="flex min-h-10 flex-wrap items-center justify-between gap-2 border-t border-[#121820] bg-[linear-gradient(180deg,#010203_0%,#000000_100%)] px-4 py-2 font-mono text-[0.68rem] uppercase tracking-wide text-slate-400 sm:px-5 lg:px-6">
      <span className="inline-flex items-center gap-2">
        <Radio className="h-3.5 w-3.5 text-fuchsia-300" aria-hidden />
        edits:live
      </span>
      <span className="flex flex-wrap items-center gap-3">
        <span>mode:{flashBusy ? 'updating' : connectionDisplayLabel(state)}</span>
        <span>board:{snapshot ? 'online' : 'standby'}</span>
        <span>save:settings</span>
        <span>update:{flashBusy ? 'running' : 'ready'}</span>
      </span>
    </footer>
  );
}

interface CommandPaletteProps {
  open: boolean;
  actions: CommandAction[];
  onClose: () => void;
}

function CommandPalette({ open, actions, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [usage, setUsage] = useState<Record<string, number>>(() => readCommandUsage());
  const [recentSearches, setRecentSearches] = useState<string[]>(() => readRecentSearches());
  const inputRef = useRef<HTMLInputElement>(null);
  const trimmedQuery = query.trim();

  const filtered = useMemo(() => {
    const needle = trimmedQuery.toLowerCase();
    const ranked = [...actions].sort((a, b) => compareCommandActions(a, b, usage, needle));
    if (!needle) return ranked;
    return ranked.filter((action) => commandMatches(action, needle));
  }, [actions, trimmedQuery, usage]);

  const quickActions = useMemo(() => {
    const used = actions
      .filter((action) => (usage[action.id] ?? 0) > 0)
      .sort((a, b) => (usage[b.id] ?? 0) - (usage[a.id] ?? 0))
      .slice(0, 5);
    if (used.length > 0) return { label: 'Most used', actions: used };
    return {
      label: 'Suggested',
      actions: filtered.filter((action) => !action.disabled).slice(0, 5),
    };
  }, [actions, filtered, usage]);

  const rememberSearch = useCallback((value: string) => {
    const cleaned = value.trim().replace(/\s+/g, ' ');
    if (cleaned.length < 2) return;
    setRecentSearches((current) => {
      const next = [
        cleaned,
        ...current.filter((item) => item.toLowerCase() !== cleaned.toLowerCase()),
      ].slice(0, 6);
      writeStoredJson(COMMAND_SEARCH_STORAGE, next);
      return next;
    });
  }, []);

  const rememberUse = useCallback((action: CommandAction) => {
    setUsage((current) => {
      const next = { ...current, [action.id]: (current[action.id] ?? 0) + 1 };
      writeStoredJson(COMMAND_USAGE_STORAGE, next);
      return next;
    });
  }, []);

  const closePalette = useCallback(() => {
    rememberSearch(query);
    onClose();
  }, [onClose, query, rememberSearch]);

  useEffect(() => {
    if (!open) return;
    setQuery('');
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [open]);

  useEffect(() => {
    setSelected(firstEnabledIndex(filtered));
  }, [filtered]);

  useEffect(() => {
    if (!open) return;

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault();
        closePalette();
      }
    }

    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [closePalette, open]);

  if (!open) return null;

  const activeIndex =
    filtered[selected] && !filtered[selected].disabled ? selected : firstEnabledIndex(filtered);
  const active = filtered[activeIndex];
  // The quick section (only shown with no query) already lists its actions, so the
  // category groups below exclude those ids — every command appears at most once.
  const showQuickActions = !trimmedQuery && quickActions.actions.length > 0;
  const quickActionIds = new Set(
    showQuickActions ? quickActions.actions.map((action) => action.id) : [],
  );
  const grouped = filtered.reduce<Record<string, CommandAction[]>>((acc, action) => {
    if (quickActionIds.has(action.id)) return acc;
    acc[action.group] = acc[action.group] ?? [];
    acc[action.group].push(action);
    return acc;
  }, {});

  function run(action: CommandAction | undefined) {
    if (!action || action.disabled) return;
    rememberSearch(query);
    rememberUse(action);
    action.run();
    onClose();
  }

  return (
    <div
      className="kb-overlay fixed inset-0 z-50 flex items-start justify-center bg-[#000102]/88 px-3 pb-4 pt-[10vh] backdrop-blur-sm"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) closePalette();
      }}
    >
      <div
        className="flex max-h-[calc(100vh-8rem)] w-full max-w-3xl flex-col overflow-hidden border border-sky-400/40 bg-[linear-gradient(180deg,#05080b_0%,#000102_100%)]"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
      >
        <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-2 border-b border-[#121820] p-3">
          <div className="flex min-w-0 items-center gap-3 border border-[#202b36] bg-[#000102] px-3 py-2">
            <Search className="h-4 w-4 shrink-0 text-sky-300" aria-hidden />
            <input
              ref={inputRef}
              value={query}
              aria-label="Search commands"
              onInput={(event) => setQuery(event.currentTarget.value)}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') {
                  event.preventDefault();
                  closePalette();
                } else if (event.key === 'ArrowDown') {
                  event.preventDefault();
                  setSelected((value) => nextEnabledIndex(filtered, value, 1));
                } else if (event.key === 'ArrowUp') {
                  event.preventDefault();
                  setSelected((value) => nextEnabledIndex(filtered, value, -1));
                } else if (event.key === 'Enter') {
                  event.preventDefault();
                  run(active);
                }
              }}
              placeholder="Search controls, pages, firmware, macros..."
              className="min-w-0 flex-1 bg-transparent font-mono text-sm text-slate-100 placeholder:text-slate-600 focus:outline-none"
            />
            <span className="shrink-0 font-mono text-[0.65rem] uppercase text-slate-500">
              {filtered.length} results
            </span>
          </div>
          <button
            type="button"
            onClick={closePalette}
            aria-label="Close command palette"
            className="kb-control kb-control-sm px-2 font-mono text-[0.7rem] uppercase text-slate-400"
          >
            <X className="h-4 w-4" aria-hidden />
            Close
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {filtered.length === 0 ? (
            <p className="p-4 text-sm text-slate-400">No command matches this query.</p>
          ) : (
            <>
              {!trimmedQuery && recentSearches.length > 0 && (
                <div className="border-b border-[#121820] px-2 py-3">
                  <p className="pb-2 font-mono text-[0.65rem] uppercase tracking-[0.2em] text-slate-600">
                    Recent searches
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {recentSearches.map((item) => (
                      <button
                        key={item}
                        type="button"
                        onClick={() => setQuery(item)}
                        className="border border-[#202b36] bg-[#000102] px-2.5 py-1.5 font-mono text-[0.7rem] text-slate-400 hover:border-sky-400/50 hover:text-sky-100"
                      >
                        {item}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {showQuickActions && (
                <CommandGroup
                  title={quickActions.label}
                  actions={quickActions.actions}
                  filtered={filtered}
                  activeIndex={activeIndex}
                  onSelect={setSelected}
                  onRun={run}
                />
              )}

              {Object.entries(grouped).map(([group, groupActions]) => (
                <CommandGroup
                  key={group}
                  title={group}
                  actions={groupActions}
                  filtered={filtered}
                  activeIndex={activeIndex}
                  onSelect={setSelected}
                  onRun={run}
                />
              ))}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function CommandGroup({
  title,
  actions,
  filtered,
  activeIndex,
  onSelect,
  onRun,
}: {
  title: string;
  actions: CommandAction[];
  filtered: CommandAction[];
  activeIndex: number;
  onSelect: (index: number) => void;
  onRun: (action: CommandAction | undefined) => void;
}) {
  return (
    <div className="py-2">
      <p className="px-2 pb-1 font-mono text-[0.65rem] uppercase tracking-[0.2em] text-slate-600">
        {title}
      </p>
      <div className="flex flex-col gap-1">
        {actions.map((action) => {
          const index = filtered.indexOf(action);
          const isActive = index === activeIndex;
          const Icon = action.icon;
          return (
            <button
              key={`${title}-${action.id}`}
              type="button"
              disabled={action.disabled}
              onMouseEnter={() => {
                if (!action.disabled && index >= 0) onSelect(index);
              }}
              onClick={() => onRun(action)}
              className={[
                'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border px-3 py-2 text-left transition-colors',
                isActive
                  ? 'border-sky-400/60 bg-sky-500/10'
                  : 'border-transparent bg-transparent hover:border-[#202b36] hover:bg-[#05080b]',
                action.disabled ? 'cursor-not-allowed opacity-40' : '',
              ].join(' ')}
            >
              <span className="grid h-8 w-8 place-items-center border border-[#202b36] bg-[#000102] text-slate-400">
                <Icon className="h-4 w-4" aria-hidden />
              </span>
              <span className="min-w-0">
                <span className="block truncate text-sm font-medium text-slate-100">
                  {action.label}
                </span>
                <span className="mt-0.5 block truncate text-xs text-slate-400">
                  {action.detail}
                </span>
              </span>
              <span className="self-center border border-[#202b36] px-2 py-1 font-mono text-[0.65rem] uppercase text-slate-500">
                {action.hint}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function compareCommandActions(
  a: CommandAction,
  b: CommandAction,
  usage: Record<string, number>,
  needle: string,
): number {
  if (Boolean(a.disabled) !== Boolean(b.disabled)) return a.disabled ? 1 : -1;

  const scoreDelta =
    commandScore(b, usage[b.id] ?? 0, needle) - commandScore(a, usage[a.id] ?? 0, needle);
  if (scoreDelta !== 0) return scoreDelta;

  return `${a.group} ${a.label}`.localeCompare(`${b.group} ${b.label}`);
}

function commandScore(action: CommandAction, useCount: number, needle: string): number {
  let score = useCount * 24;
  if (!needle) return score;

  const label = action.label.toLowerCase();
  const group = action.group.toLowerCase();
  const detail = action.detail.toLowerCase();
  const hint = action.hint.toLowerCase();
  const keywords = action.keywords?.join(' ').toLowerCase() ?? '';

  if (label === needle) score += 140;
  if (label.startsWith(needle)) score += 90;
  if (group.includes(needle)) score += 40;
  if (hint.includes(needle)) score += 35;
  if (keywords.includes(needle)) score += 30;
  if (detail.includes(needle)) score += 15;
  return score;
}

function commandMatches(action: CommandAction, needle: string): boolean {
  return commandSearchText(action).includes(needle);
}

function commandSearchText(action: CommandAction): string {
  return [action.group, action.label, action.detail, action.hint, ...(action.keywords ?? [])]
    .join(' ')
    .toLowerCase();
}

function readCommandUsage(): Record<string, number> {
  const value = readStoredJson<Record<string, unknown>>(COMMAND_USAGE_STORAGE, {});
  return Object.fromEntries(
    Object.entries(value).filter(
      (entry): entry is [string, number] => typeof entry[1] === 'number',
    ),
  );
}

function readRecentSearches(): string[] {
  return readStoredJson<unknown[]>(COMMAND_SEARCH_STORAGE, [])
    .filter((value): value is string => typeof value === 'string')
    .slice(0, 6);
}

function readStoredJson<T>(key: string, fallback: T): T {
  if (typeof window === 'undefined') return fallback;
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeStoredJson(key: string, value: unknown): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Command history is a convenience cache; storage failures should not block controls.
  }
}

function firstEnabledIndex(actions: CommandAction[]): number {
  const index = actions.findIndex((action) => !action.disabled);
  return index >= 0 ? index : 0;
}

function nextEnabledIndex(actions: CommandAction[], current: number, direction: 1 | -1): number {
  if (actions.length === 0) return 0;

  for (let step = 1; step <= actions.length; step += 1) {
    const index = (current + step * direction + actions.length) % actions.length;
    if (!actions[index]?.disabled) return index;
  }

  return current;
}

function StatusStack({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-3">{children}</div>;
}

interface NoticeProps {
  tone: 'warning' | 'error';
  title: string;
  children: ReactNode;
}

function Notice({ tone, title, children }: NoticeProps) {
  const tones = {
    warning: 'border-amber-500/40 bg-amber-500/10 text-amber-100',
    error: 'border-red-500/40 bg-red-500/10 text-red-100',
  } as const;

  return (
    <div role={tone === 'error' ? 'alert' : 'status'} className={`border p-4 ${tones[tone]}`}>
      <p className="font-mono text-xs font-semibold uppercase tracking-wide">{title}</p>
      <p className="mt-1 text-sm opacity-90">{children}</p>
    </div>
  );
}

interface ToastProps {
  tone: 'ok' | 'warn';
  onDismiss: () => void;
  children: ReactNode;
}

/** A dismissible persist-across-flash notification (restore succeeded / was skipped). */
function Toast({ tone, onDismiss, children }: ToastProps) {
  const tones = {
    ok: 'border-emerald-500/40 bg-emerald-500/10 text-emerald-100',
    warn: 'border-amber-500/40 bg-amber-500/10 text-amber-100',
  } as const;

  return (
    <div role="status" className={`flex items-start justify-between gap-4 border p-4 ${tones[tone]}`}>
      <p className="text-sm opacity-90">{children}</p>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss"
        className="shrink-0 px-2 py-0.5 text-sm opacity-70 transition-opacity hover:opacity-100"
      >
        x
      </button>
    </div>
  );
}

interface DevicePickerProps {
  devices: NativeHidDevice[];
  onSelect: (device: NativeHidDevice) => void;
}

/**
 * Native-build device chooser, shown only when several keeberry devices are
 * attached. The browser uses the WebHID chooser instead, so this never renders
 * there. Picking one connects to it through the Rust transport.
 */
function DevicePicker({ devices, onSelect }: DevicePickerProps) {
  return (
    <main className="border border-[#121820] bg-[linear-gradient(180deg,#05080b_0%,#010203_100%)] p-6">
      <p className="font-mono text-xs uppercase tracking-[0.2em] text-slate-500">
        Select a keeberry keyboard
      </p>
      <ul className="mt-4 flex flex-col gap-2">
        {devices.map((device) => (
          <li key={device.path}>
            <button
              type="button"
              onClick={() => onSelect(device)}
              className="kb-control w-full justify-start whitespace-normal break-words px-4 text-left"
            >
              {device.name}
            </button>
          </li>
        ))}
      </ul>
    </main>
  );
}

interface FlashProgressBannerProps {
  progress: FlashProgress;
  busy: boolean;
  onDismiss: () => void;
}

/**
 * The firmware-flash status banner. It is rendered at the App level so it stays
 * visible across the disconnect/reconnect a flash causes, and is dismissible only
 * once the round-trip reaches a terminal step (done/error) and is no longer busy.
 */
function FlashProgressBanner({ progress, busy, onDismiss }: FlashProgressBannerProps) {
  const tones = {
    error: 'border-red-500/40 bg-red-500/10 text-red-100',
    done: 'border-emerald-500/40 bg-emerald-500/10 text-emerald-100',
    active: 'border-sky-500/40 bg-sky-500/10 text-sky-100',
  } as const;
  const tone =
    progress.phase === 'error'
      ? tones.error
      : progress.phase === 'done'
        ? tones.done
        : tones.active;
  const terminal = progress.phase === 'done' || progress.phase === 'error';

  return (
    <div
      role={progress.phase === 'error' ? 'alert' : 'status'}
      className={`flex items-start justify-between gap-4 border p-4 ${tone}`}
    >
      <div className="flex items-center gap-3">
        {!terminal && <span className="h-2 w-2 animate-pulse bg-current" aria-hidden />}
        <p className="text-sm">{progressMessage(progress)}</p>
      </div>
      {terminal && !busy && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="shrink-0 px-2 py-0.5 text-sm opacity-70 transition-opacity hover:opacity-100"
        >
          x
        </button>
      )}
    </div>
  );
}

function progressMessage(progress: FlashProgress): string {
  // The flasher's own per-event message carries the live detail (and the pre-flash
  // backup-failure note); fall back to a phase label only when the event omits one.
  if (progress.message) return progress.message;
  switch (progress.phase) {
    case 'entering':
      return 'Preparing the keyboard for update…';
    case 'waiting':
      return 'Waiting for the keyboard…';
    case 'flashing':
      return 'Installing update…';
    case 'rebooting':
      return 'Returning to keyboard mode…';
    case 'done':
      return 'Update complete.';
    case 'error':
      return 'The update could not complete. Check the connection and try again.';
  }
}

interface BootloaderPanelProps {
  bundled: BundledFirmware | null;
  busy: boolean;
  onFlash: () => void;
  onReboot: () => void;
}

/**
 * Native-only recovery card, shown while no keyboard is connected. After entering
 * the bootloader the board drops off USB as a DFU device (not a keeberry HID
 * device), so the per-connection panels are gone. This is where a board already
 * in the bootloader gets flashed or rebooted back into its firmware.
 */
function BootloaderPanel({ bundled, busy, onFlash, onReboot }: BootloaderPanelProps) {
  return (
    <section className="border border-amber-500/30 bg-amber-500/10 p-5">
      <div className="flex items-center gap-2">
        <Power className="h-4 w-4 text-amber-200" aria-hidden />
        <h2 className="font-mono text-xs font-semibold uppercase tracking-[0.2em] text-amber-200">
          Recovery bay
        </h2>
      </div>
      <p className="mt-2 text-xs leading-5 text-amber-100/75">
        Recovery tools are available while the board is in recovery mode.
      </p>
      <div className="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          disabled={busy || !bundled}
          onClick={onFlash}
          className="kb-control kb-control-primary"
        >
          <Save className="h-4 w-4" aria-hidden />
          {busy ? 'Working' : 'Install update package'}
        </button>
        <ActionMenu
          label="Recovery"
          align="left"
          actions={[
            {
              id: 'reboot-to-keyboard',
              label: 'Return to keyboard',
              detail: 'Leave recovery mode',
              disabled: busy,
              onSelect: onReboot,
            },
          ]}
        />
      </div>
    </section>
  );
}

interface SwitchedModeWorkbenchProps {
  mode: UsbMode;
  onReconnect: () => void;
  onCancel: () => void;
}

/** Per-personality copy for the waiting panel shown after a USB-mode switch. */
const SWITCHED_COPY: Record<
  UsbMode,
  { glyph: string; title: string; lead: string; steps: string[] }
> = {
  [UsbMode.Normal]: {
    glyph: '⌨️',
    title: 'Returning to keyboard',
    lead: 'The keyboard is re-enumerating as a normal keyboard. This deck reconnects on its own the moment it returns.',
    steps: [],
  },
  [UsbMode.Midi]: {
    glyph: '🎹',
    title: 'Now in MIDI',
    lead: 'The keyboard is acting as a USB-MIDI controller. Its kcp control channel can stay reachable; if it does, this deck reconnects on its own and you can switch back from the USB mode panel.',
    steps: [
      'Switch back from here if it stays connected, or',
      'Unplug and replug the keyboard to return to a keyboard.',
    ],
  },
  [UsbMode.XInput]: {
    glyph: '🎮',
    title: 'Now in XInput',
    lead: 'The keyboard re-enumerated as a separate Xbox 360 controller (USB 045E:028E "Controller"); macOS surfaces it as a gamepad. The host OS claims the whole controller — including the kcp channel the configurator talks over — so this deck cannot switch it back.',
    steps: [
      'Hold Fn + Right Ctrl for ~1.5s — the keyboard glows solid red while both keys are held — to return to a keyboard, or',
      'Unplug and replug the keyboard.',
    ],
  },
};

/**
 * Full-width waiting panel shown while the link is parked in a re-enumerated USB
 * personality (state `'switched'`). The connection is not an error — the board left
 * the kcp interface by our own `SET_USB_MODE` — so this explains the mode, how to get
 * back to a keyboard, and offers a manual reconnect alongside the automatic watcher
 * that reopens the board the moment its kcp interface returns.
 */
function SwitchedModeWorkbench({ mode, onReconnect, onCancel }: SwitchedModeWorkbenchProps) {
  const copy = SWITCHED_COPY[mode];
  // XInput cannot be left from the app, so it reads as a caution; MIDI and the
  // return-to-Normal wait are routine and read as informational.
  const caution = mode === UsbMode.XInput;
  const frame = caution ? 'border-amber-500/30 bg-amber-500/10' : 'border-sky-500/30 bg-sky-500/10';
  const accent = caution ? 'text-amber-200' : 'text-sky-200';

  return (
    <section className={`border ${frame} p-6`}>
      <div className="flex items-center gap-2">
        <Unplug className={`h-4 w-4 ${accent}`} aria-hidden />
        <span className={`font-mono text-[0.65rem] uppercase tracking-[0.2em] ${accent}`}>
          {usbModeLabel(mode)}
        </span>
      </div>
      <h2 className="mt-3 flex items-center gap-2 text-2xl font-semibold tracking-tight text-slate-50">
        <span aria-hidden>{copy.glyph}</span>
        {copy.title}
      </h2>
      <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-300">{copy.lead}</p>

      {copy.steps.length > 0 && (
        <div className="mt-4 max-w-2xl">
          <p className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
            To return to a keyboard
          </p>
          <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-6 text-slate-300">
            {copy.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ul>
        </div>
      )}

      <p className="mt-4 text-xs leading-5 text-slate-400">
        This deck reconnects automatically as soon as the keyboard is back. You can also reconnect
        manually once it returns.
      </p>

      <div className="mt-5 flex flex-wrap gap-2">
        <button type="button" onClick={onReconnect} className="kb-control kb-control-primary">
          <Cable className="h-4 w-4" aria-hidden />
          Reconnect now
        </button>
        <button type="button" onClick={onCancel} className="kb-control">
          <X className="h-4 w-4" aria-hidden />
          Back to start
        </button>
      </div>
    </section>
  );
}

interface MetricTileProps {
  label: string;
  value: string;
  tone: 'ok' | 'blue' | 'idle';
}

function MetricTile({ label, value, tone }: MetricTileProps) {
  const tones = {
    ok: 'text-emerald-200',
    blue: 'text-sky-200',
    idle: 'text-slate-300',
  } as const;
  return (
    <div className="border border-[#202b36] bg-[#000102] px-3 py-2">
      <span className="block font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
        {label}
      </span>
      <span className={`mt-1 block truncate font-mono text-sm ${tones[tone]}`}>{value}</span>
    </div>
  );
}

function StatusPill({ tone, children }: { tone: 'ok' | 'busy' | 'idle'; children: ReactNode }) {
  const Icon = tone === 'ok' ? CheckCircle2 : tone === 'busy' ? Activity : Power;
  const tones = {
    ok: 'border-emerald-400/40 bg-emerald-500/10 text-emerald-200',
    busy: 'border-amber-400/40 bg-amber-500/10 text-amber-200',
    idle: 'border-[#202b36] bg-[#030609] text-slate-400',
  } as const;

  return (
    <span
      className={`inline-flex min-h-8 items-center gap-2 border px-3 py-0.5 font-mono text-xs uppercase ${tones[tone]}`}
    >
      <Icon className="h-3.5 w-3.5" aria-hidden />
      {children}
    </span>
  );
}

function connectionDisplayLabel(state: ConnectionState): string {
  switch (state) {
    case 'unsupported':
      return 'unavailable';
    case 'selecting':
      return 'choosing';
    case 'error':
      return 'attention';
    default:
      return state;
  }
}

function accentText(accent: PageSpec['accent']): string {
  switch (accent) {
    case 'green':
      return 'text-emerald-300';
    case 'yellow':
      return 'text-amber-300';
    case 'violet':
      return 'text-fuchsia-300';
    default:
      return 'text-sky-300';
  }
}
