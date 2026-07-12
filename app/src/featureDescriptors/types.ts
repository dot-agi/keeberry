// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The self-describing `FeatureDescriptor` schema (planning note `.planning/sdk-llm-friendly.md`
 * §6, "Design 3"). A feature's whole configurator surface is expressed as *data* — a list of
 * typed controls bound to kcp ops — and one generic renderer (`ui/DescriptorPanel.tsx`) draws
 * it. There is no per-feature React: authoring a config GUI becomes writing this object, the
 * MCP `inputSchema` idea applied to the configurator.
 *
 * This file is the spec an LLM (or human) fills in for a new feature, so the doc comments are
 * the contract: keep every field documented with its units, range and wire semantics.
 */

/**
 * A kcp operation a control reads from or writes to, named by its raw command byte (a value
 * of `kcp/protocol.ts`'s `Cmd`). The high nibble is the feature's group, the low nibble the
 * op within it.
 *
 * The renderer's value convention (the v1.5 stand-in for §7's eventual field-path codegen):
 *  - **read** (a control's `get`): the op is sent with `args` (default none) and the control's
 *    current value is decoded from the reply payload at byte `at` (default `0`) — a single
 *    byte for the scalar kinds, three bytes (`h`, `s`, `v`) for `color`. The clean getters put
 *    the value at byte 0 (autocorrect, unicode, KRO), so `at` is omitted; a struct reply (e.g.
 *    RGB `GET_STATE`) names the field's offset.
 *  - **write** (a control's `set`): the op is sent as `[...args, value]` — the new value
 *    byte(s) appended after any fixed prefix `args`. A clean single-value setter (autocorrect
 *    `SET`, RGB `SET_BRIGHTNESS`) needs no `args`.
 */
export interface Op {
  /** The raw kcp command byte — a `kcp/protocol.ts` `Cmd` value. */
  cmd: number;
  /**
   * Fixed request bytes that prefix the payload. A `set` appends the control's value after
   * them (`[...args, value]`); a `get` sends them as-is (e.g. an index selecting a slot).
   * Omit for the common case of a value-only request.
   */
  args?: number[];
  /**
   * Read-only: the byte offset of this control's value within the `get` reply payload
   * (default `0`). It names the field's position in a struct reply — e.g. RGB brightness is
   * byte 4 of `GET_STATE` — and is ignored by a `set`.
   */
  at?: number;
}

/** Fields shared by every control kind. */
interface ControlBase {
  /** Human label shown beside the widget. */
  label: string;
  /**
   * Optional symbolic id, referenced only by other controls' `showIf` expressions — it is
   * **not** sent on the wire. Give a control a token when another control's visibility
   * depends on its value.
   */
  token?: string;
  /**
   * Optional visibility expression, evaluated against the live values of token'd controls;
   * the control renders only when the expression is truthy (a missing `showIf` always shows).
   * Grammar (VIA-style): integer literals, control tokens, the comparisons `==`, `!=`, `<`,
   * `>`, the boolean connectives `&&`, `||`, and `(`…`)` grouping; a bare token is truthy when
   * non-zero. Example: `"enabled == 1 && mode != 2"`. A malformed expression or an unknown
   * token fails open (the control shows), per the LSP ignore-unknown rule.
   */
  showIf?: string;
}

/** A two-state on/off switch (value `0` or `1`). Rendered as an Off/On segmented control. */
export interface ToggleControl extends ControlBase {
  kind: 'toggle';
  get: Op;
  set: Op;
}

/** A continuous integer in `[min, max]` (optionally stepped). Rendered as a range slider. */
export interface SliderControl extends ControlBase {
  kind: 'slider';
  min: number;
  max: number;
  /** Slider increment (default `1`). */
  step?: number;
  get: Op;
  set: Op;
}

/** A typed integer in `[min, max]`, committed on blur/Enter. Rendered as a number input. */
export interface NumberControl extends ControlBase {
  kind: 'number';
  min: number;
  max: number;
  get: Op;
  set: Op;
}

/** A choice from a fixed value set. Rendered as a select. */
export interface EnumControl extends ControlBase {
  kind: 'enum';
  /** The selectable options; each `value` is the byte written by `set`. */
  options: { label: string; value: number }[];
  get: Op;
  set: Op;
}

/** An HSV colour — the value is the three bytes `[h, s, v]`. Rendered as an HSV picker. */
export interface ColorControl extends ControlBase {
  kind: 'color';
  get: Op;
  set: Op;
}

/** The discriminated union of every renderable control, keyed by `kind`. */
export type Control = ToggleControl | SliderControl | NumberControl | EnumControl | ColorControl;

/** A feature's whole config surface: a title and its ordered controls, keyed by `fid`. */
export interface FeatureDescriptor {
  /**
   * The feature this describes. For a firmware feature it is the `FeatureId` discriminant (the
   * same id the FEATURES enumeration reports); the app-side registry is keyed by it, and the
   * render-priority matches it against the enumerated features.
   */
  fid: number;
  /** Panel heading. */
  title: string;
  /** The controls, rendered top-to-bottom. */
  controls: Control[];
}

/**
 * The minimal capability `ui/DescriptorPanel.tsx` needs from a kcp client: run an op by its
 * raw command byte and resolve the decoded reply payload. `kcp/client.ts`'s `KcpClient`
 * satisfies this through its `runOp` method, so the panel stays decoupled from the concrete
 * client (and trivially testable against the firmware fixture).
 */
export interface OpRunner {
  runOp(cmd: number, args?: number[]): Promise<Uint8Array>;
}
