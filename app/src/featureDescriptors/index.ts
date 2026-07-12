// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The app-side `FeatureDescriptor` registry (planning §6's "hybrid storage", default branch):
 * descriptors live in the app keyed by `fid` — zero firmware flash, the VIA model. The
 * render-priority in `App.tsx` consults this map: a feature with a hand-built panel keeps it;
 * otherwise a registered descriptor renders through the one generic `DescriptorPanel`;
 * otherwise the feature falls back to its generic FEATURES toggle.
 *
 * The registry ships **empty**: every feature shipped today has a richer hand-built panel, so
 * none registers a data-driven descriptor. `just new-feature --kind config` stamps a descriptor
 * file and registers it at the `@scaffold:` anchors below — that is how the generic
 * `DescriptorPanel` / `runtime.ts` / `types.ts` extension point gets populated. An empty registry
 * is the clean degenerate case: `App.tsx`'s descriptor branch yields nothing and every feature
 * falls through to its generic toggle.
 */
import { GROUP_DEFS, type GroupName } from '../kcp/info';
import type { FeatureDescriptor } from './types';

// @scaffold:descriptor-imports — `just new-feature <Name> --kind config` inserts each new
// descriptor's `import { <camel>Descriptor } from './<name>';` above this line.

/** The registered descriptors, in registration order (empty until a config feature is scaffolded). */
const REGISTERED_DESCRIPTORS: readonly FeatureDescriptor[] = [
  // @scaffold:descriptor-registry — `just new-feature <Name> --kind config` inserts each new
  // `<camel>Descriptor,` entry above this line.
];

/** Every registered descriptor, keyed by its `fid`. */
export const featureDescriptors: ReadonlyMap<number, FeatureDescriptor> = new Map(
  REGISTERED_DESCRIPTORS.map((descriptor) => [descriptor.fid, descriptor] as const),
);

/**
 * The kcp group a descriptor talks to, derived from its first control's op nibble (the high
 * nibble of the command byte equals the capability bit). The app gates a descriptor on the
 * device actually advertising that group; `null` for an empty or unknown-group descriptor.
 */
export function descriptorGroup(descriptor: FeatureDescriptor): GroupName | null {
  const cmd = descriptor.controls[0]?.get.cmd;
  if (cmd === undefined) return null;
  const nibble = cmd >> 4;
  return GROUP_DEFS.find((group) => group.bit === nibble)?.name ?? null;
}
