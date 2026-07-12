// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useState } from 'react';
import { NONE, classify, isMatrixHole, type KcpClient } from '../kcp';
import { ErrorBanner, InfoHint, Panel, StatusBanner, UnsavedBadge } from './Panel';
import { friendlyPanelError } from './panelError';
import { useConfigSave } from './useConfigSave';
import { useUnsavedChanges } from './useUnsavedChanges';
import { KeycodePicker } from './KeycodePicker';
import { keyLabel } from './keyDisplay';

interface KeymapEditorProps {
  client: KcpClient;
  rows: number;
  cols: number;
  layerCount: number;
}

interface EditTarget {
  row: number;
  col: number;
}

/**
 * Live keymap editor: a 6×15 matrix grid (the real scanner matrix, with the
 * 75% layout holes rendered as gaps) and a layer selector. The active layer is
 * read position-by-position over `getKeycode` on load; clicking a key opens the
 * categorised {@link KeycodePicker} and writes the choice live with
 * `setKeycode`.
 */
export function KeymapEditor({ client, rows, cols, layerCount }: KeymapEditorProps) {
  const [layer, setLayer] = useState(0);
  const [keys, setKeys] = useState<number[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<EditTarget | null>(null);
  const [timedCaps, setTimedCaps] = useState({ tapDance: 0, macro: 0 });
  const unsaved = useUnsavedChanges(client);
  const {
    saving,
    status: saveStatus,
    error: saveError,
    save,
    clearFeedback,
  } = useConfigSave(client);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    (async () => {
      const next = new Array<number>(rows * cols).fill(NONE);
      try {
        for (let r = 0; r < rows; r += 1) {
          for (let c = 0; c < cols; c += 1) {
            if (isMatrixHole(r, c)) continue;
            const kc = await client.getKeycode(layer, r, c);
            if (cancelled) return;
            next[r * cols + c] = kc;
          }
        }
        if (!cancelled) setKeys(next);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [client, layer, rows, cols]);

  // The tap-dance / macro table capacities bound the `TD(n)` / `MACRO(n)` options
  // the picker offers; they are fixed per firmware build, so read them once. A
  // read failure just leaves those options empty — key binding stays usable and
  // the per-key load effect surfaces any real device error.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const ti = await client.timedInfo();
        if (!cancelled) setTimedCaps({ tapDance: ti.maxTapDance, macro: ti.maxMacro });
      } catch {
        // Intentionally ignored: see the comment above.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function applyKeycode(target: EditTarget, raw: number) {
    setEditing(null);
    setError(null);
    // A fresh edit re-dirties the map, so drop any stale "saved" confirmation.
    clearFeedback();
    try {
      await client.setKeycode(layer, target.row, target.col, raw);
      setKeys((prev) => {
        const next = prev.slice();
        next[target.row * cols + target.col] = raw;
        return next;
      });
    } catch (err) {
      setError(friendlyPanelError(err));
    }
  }

  const editingRaw = editing ? (keys[editing.row * cols + editing.col] ?? NONE) : NONE;

  return (
    <Panel
      title="Keymap"
      description={
        <>
          Click a key to rebind this layer. Edits apply immediately; Save keeps them after a
          restart.
        </>
      }
      headerExtra={
        <div className="flex flex-wrap items-center gap-1.5">
          {Array.from({ length: layerCount }, (_, n) => (
            <button
              key={n}
              type="button"
              aria-pressed={n === layer}
              onClick={() => setLayer(n)}
              className={[
                'kb-control kb-control-sm font-mono uppercase',
                n === layer
                  ? 'border-sky-300 bg-sky-500/25 text-sky-50'
                  : 'border-[#26313c] text-slate-400 hover:border-sky-400/50 hover:bg-[#111820] hover:text-sky-100',
              ].join(' ')}
            >
              L{n}
            </button>
          ))}
        </div>
      }
    >
      {(error ?? saveError) && <ErrorBanner>{error ?? saveError}</ErrorBanner>}
      {saveStatus && <StatusBanner>{saveStatus}</StatusBanner>}

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={saving}
          onClick={() => {
            // The save banner replaces any key-load/bind error while it reports.
            setError(null);
            void save();
          }}
          className={unsaved ? 'kb-control kb-control-primary' : 'kb-control'}
        >
          {saving ? 'Saving…' : 'Save settings'}
        </button>
        {unsaved && <UnsavedBadge />}
        <InfoHint label="Save details">
          Key edits apply immediately. Save keeps them on the keyboard after a restart.
        </InfoHint>
      </div>

      <div className="kb-subpanel p-3">
        <div className="mb-2 flex items-center justify-between gap-3 font-mono text-[0.65rem] uppercase tracking-wide text-slate-600">
          <span>key deck</span>
          <span>{loading ? 'syncing' : 'live'}</span>
        </div>
        <div className="overflow-x-auto">
          <div
            className="grid min-w-[49rem] gap-1"
            style={{ gridTemplateColumns: `repeat(${cols}, minmax(3rem, 1fr))` }}
          >
            {Array.from({ length: rows * cols }, (_, idx) => {
              const row = Math.floor(idx / cols);
              const col = idx % cols;
              if (isMatrixHole(row, col)) {
                return <div key={idx} aria-hidden className="h-12 border border-transparent" />;
              }
              const raw = keys[idx] ?? NONE;
              return (
                <KeyCap
                  key={idx}
                  raw={raw}
                  loading={loading}
                  onClick={() => setEditing({ row, col })}
                />
              );
            })}
          </div>
        </div>
      </div>

      {editing && (
        <KeycodePicker
          context="keymap"
          layerCount={layerCount}
          tapDanceCount={timedCaps.tapDance}
          macroCount={timedCaps.macro}
          currentRaw={editingRaw}
          contextLabel="selected key"
          onPick={(raw) => void applyKeycode(editing, raw)}
          onClose={() => setEditing(null)}
        />
      )}
    </Panel>
  );
}

interface KeyCapProps {
  raw: number;
  loading: boolean;
  onClick: () => void;
}

/** Colour a key by its decoded kind so NO / TRNS read distinctly from bound keys. */
function keyTone(raw: number): string {
  switch (classify(raw).kind) {
    case 'noop':
      return 'border-[#111820] bg-[#050608] text-slate-700';
    case 'transparent':
      return 'border-[#26313c] bg-[#090c10] text-slate-500 italic';
    case 'modifier':
      return 'border-sky-500/50 bg-sky-500/15 text-sky-100';
    case 'momentary':
      return 'border-amber-500/50 bg-amber-500/15 text-amber-100';
    case 'consumer':
      return 'border-fuchsia-500/50 bg-fuchsia-500/15 text-fuchsia-100';
    default:
      return 'border-[#26313c] bg-[#111820] text-slate-100';
  }
}

function KeyCap({ raw, loading, onClick }: KeyCapProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={loading}
      aria-label={loading ? 'Loading key' : `Bind ${keyLabel(raw)}`}
      className={[
        'flex h-12 items-center justify-center border px-1 text-center font-mono text-[0.65rem] font-semibold leading-tight transition-colors hover:border-sky-300 hover:bg-sky-500/15 disabled:cursor-wait disabled:opacity-55',
        keyTone(raw),
      ].join(' ')}
    >
      <span className="max-w-full overflow-hidden text-ellipsis whitespace-nowrap">
        {loading ? '…' : keyLabel(raw)}
      </span>
    </button>
  );
}
