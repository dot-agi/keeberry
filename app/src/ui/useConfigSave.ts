// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useState } from 'react';
import type { KcpClient } from '../kcp';
import { friendlyPanelError } from './panelError';

/** The status banner text shown after a `CONFIG.SAVE` resolves. */
const SAVED_MESSAGE = 'Saved all settings to the keyboard.';

export interface ConfigSave {
  /** True while a `CONFIG.SAVE` request is in flight. */
  saving: boolean;
  /** Success banner text after the last save, or null. */
  status: string | null;
  /** User-facing error banner text after the last save, or null. */
  error: string | null;
  /** Persist the whole live config to flash; resolves true on success, false on failure. */
  save: () => Promise<boolean>;
  /** Clear this hook's status/error banners (before an unrelated action reports its own). */
  clearFeedback: () => void;
}

/**
 * The shared `CONFIG.SAVE` action. kcp edits apply to device RAM the moment they are
 * made; this is the one explicit step that keeps them across a restart (persisting the
 * complete config blob, so it saves everything regardless of which panel triggers it).
 * The Settings and Keymap panels both drive their Save button through this hook, so the
 * persist request, its success message and its error handling live in one place rather
 * than being copy-pasted per panel.
 */
export function useConfigSave(client: KcpClient): ConfigSave {
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const save = useCallback(async () => {
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      await client.configSave();
      setStatus(SAVED_MESSAGE);
      return true;
    } catch (err) {
      setError(friendlyPanelError(err));
      return false;
    } finally {
      setSaving(false);
    }
  }, [client]);

  const clearFeedback = useCallback(() => {
    setStatus(null);
    setError(null);
  }, []);

  return { saving, status, error, save, clearFeedback };
}
