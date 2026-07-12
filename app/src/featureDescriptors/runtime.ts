// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The data-driven runtime behind `ui/DescriptorPanel.tsx`: reading a control's value from its
 * `get` op, writing it through its `set` op, and evaluating `showIf` visibility. Pure and
 * client-agnostic (it speaks only the `OpRunner` seam), so the whole descriptor IO round-trips
 * against the firmware fixture with no DOM and no real device.
 */
import type { Control, OpRunner } from './types';

/** A control's current value: one byte for the scalar kinds, `[h, s, v]` for `color`. */
export type ControlValue = number | number[];

/** A stable per-control key (its token, else its position) for the panel's value map. */
export function controlKey(control: Control, index: number): string {
  return control.token ?? `c${index}`;
}

/** The value a control shows before its `get` resolves (the low bound for the ranged kinds). */
export function defaultValue(control: Control): ControlValue {
  switch (control.kind) {
    case 'slider':
    case 'number':
      return control.min;
    case 'color':
      return [0, 0, 0];
    default:
      return 0;
  }
}

/**
 * Read a control's current value: run its `get` op and decode the reply at `get.at`
 * (default 0) — one byte for a scalar, three (`h`, `s`, `v`) for a colour.
 */
export async function readControlValue(runner: OpRunner, control: Control): Promise<ControlValue> {
  const payload = await runner.runOp(control.get.cmd, control.get.args);
  const at = control.get.at ?? 0;
  if (control.kind === 'color') {
    return [payload[at] ?? 0, payload[at + 1] ?? 0, payload[at + 2] ?? 0];
  }
  return payload[at] ?? 0;
}

/**
 * Write a control's new value through its `set` op: the value byte(s) are appended after any
 * fixed prefix args (`[...args, value]`).
 */
export async function writeControlValue(
  runner: OpRunner,
  control: Control,
  value: ControlValue,
): Promise<void> {
  const prefix = control.set.args ?? [];
  const bytes = Array.isArray(value) ? value : [value];
  await runner.runOp(control.set.cmd, [...prefix, ...bytes.map((b) => b & 0xff)]);
}

/**
 * Evaluate a `showIf` expression against the current scalar control values (keyed by token),
 * returning whether the control should show. Grammar:
 * `or := and ('||' and)*`, `and := cmp ('&&' cmp)*`, `cmp := primary (op primary)?` with
 * `op ∈ == != < >`, `primary := number | token | '(' or ')'`. Comparisons yield `1`/`0`;
 * `&&`/`||` treat any non-zero operand as true. Throws on a malformed expression or an
 * unresolved token — the caller (the panel) fails open, so a typo never hides a control.
 */
export function evalShowIf(expr: string, values: Record<string, number>): boolean {
  const tokens = tokenizeShowIf(expr);
  let pos = 0;
  const peek = () => tokens[pos];
  const next = () => tokens[pos++];

  function parsePrimary(): number {
    const token = next();
    if (token === undefined) throw new Error('showIf: unexpected end of expression');
    if (token === '(') {
      const value = parseOr();
      if (next() !== ')') throw new Error('showIf: missing )');
      return value;
    }
    if (/^\d+$/.test(token)) return Number(token);
    if (/^[A-Za-z_]\w*$/.test(token)) {
      // Fail open on an unresolved token (a typo, or a token no control in this descriptor
      // defines): throw so the caller shows the control, rather than silently reading it as 0
      // and hiding it. Matches the LSP ignore-unknown intent — a typo never hides a control.
      if (Object.prototype.hasOwnProperty.call(values, token)) return values[token];
      throw new Error(`showIf: unknown token "${token}"`);
    }
    throw new Error(`showIf: unexpected token "${token}"`);
  }

  function parseCmp(): number {
    const left = parsePrimary();
    const op = peek();
    if (op === '==' || op === '!=' || op === '<' || op === '>') {
      next();
      const right = parsePrimary();
      switch (op) {
        case '==':
          return left === right ? 1 : 0;
        case '!=':
          return left !== right ? 1 : 0;
        case '<':
          return left < right ? 1 : 0;
        default:
          return left > right ? 1 : 0;
      }
    }
    return left;
  }

  function parseAnd(): number {
    let left = parseCmp();
    while (peek() === '&&') {
      next();
      const right = parseCmp();
      left = left !== 0 && right !== 0 ? 1 : 0;
    }
    return left;
  }

  function parseOr(): number {
    let left = parseAnd();
    while (peek() === '||') {
      next();
      const right = parseAnd();
      left = left !== 0 || right !== 0 ? 1 : 0;
    }
    return left;
  }

  const result = parseOr();
  if (pos !== tokens.length) throw new Error('showIf: trailing input');
  return result !== 0;
}

/** Split a `showIf` expression into tokens (numbers, identifiers, operators, parens). */
function tokenizeShowIf(expr: string): string[] {
  const pattern = /(==|!=|<|>|&&|\|\||\(|\)|\d+|[A-Za-z_]\w*)/y;
  const tokens: string[] = [];
  let index = 0;
  while (index < expr.length) {
    if (/\s/.test(expr[index])) {
      index += 1;
      continue;
    }
    pattern.lastIndex = index;
    const match = pattern.exec(expr);
    if (!match || match.index !== index) {
      throw new Error(`showIf: bad token at position ${index} in "${expr}"`);
    }
    tokens.push(match[0]);
    index = pattern.lastIndex;
  }
  return tokens;
}
