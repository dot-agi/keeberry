// SPDX-License-Identifier: GPL-2.0-or-later
import { KcpProtocolError, KcpTimeoutError } from '../kcp';

/**
 * Turn a caught exception into the message a panel's `ErrorBanner` shows.
 *
 * Protocol and transport faults carry internal wire detail (command bytes, status
 * codes, sequence numbers) that means nothing to a user, so they collapse to one
 * safe, generic line. Errors raised with an authored, user-facing message — config
 * backup validation, for instance — are surfaced verbatim so their guidance survives.
 */
export function friendlyPanelError(err: unknown): string {
  if (err instanceof KcpProtocolError || err instanceof KcpTimeoutError) {
    return 'The action could not complete. Check the keyboard connection and try again.';
  }
  return err instanceof Error ? err.message : String(err);
}
