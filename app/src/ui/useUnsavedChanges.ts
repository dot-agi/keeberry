// SPDX-License-Identifier: GPL-2.0-or-later
import { useSyncExternalStore } from 'react';
import type { KcpClient } from '../kcp';

/**
 * Subscribe to a client's unsaved-changes flag. The client is the single source
 * of truth — it sets the flag as persisted-config setters and `CONFIG.LOAD_DEFAULTS`
 * succeed (defaults are RAM-only until saved) and clears it only on `CONFIG.SAVE`
 * — so `useSyncExternalStore` mirrors it without duplicating state or risking
 * tearing across panels.
 */
export function useUnsavedChanges(client: KcpClient): boolean {
  return useSyncExternalStore(
    (onStoreChange) => client.onUnsavedChange(onStoreChange),
    () => client.hasUnsavedChanges,
  );
}
