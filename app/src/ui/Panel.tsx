// SPDX-License-Identifier: GPL-2.0-or-later
import {
  cloneElement,
  isValidElement,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, Info, Menu } from 'lucide-react';

interface PanelProps {
  /** Section heading, rendered in the shared uppercase panel style. */
  title: string;
  /** Optional help content shown in the panel heading hint. */
  description?: ReactNode;
  /** Optional element pinned to the right of the heading (status, toggle, etc.). */
  headerExtra?: ReactNode;
  children: ReactNode;
}

/**
 * Shared hard-edged panel chrome for every control group.
 * Factoring it out keeps the command pages consistent and free of copy-pasted frame work.
 */
export function Panel({ title, description, headerExtra, children }: PanelProps) {
  return (
    <section className="kb-panel border border-[#121820] p-4 sm:p-5">
      <div className="mb-4 flex min-h-8 items-start justify-between gap-3 border-b border-[#121820] pb-3">
        <div className="flex min-w-0 items-center gap-2">
          <h2 className="kb-panel-heading text-slate-400">{title}</h2>
          {description && <InfoHint label={`${title} details`}>{description}</InfoHint>}
        </div>
        {headerExtra && <div className="shrink-0">{headerExtra}</div>}
      </div>
      {children}
    </section>
  );
}

interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  align?: 'left' | 'center' | 'right';
  fullWidth?: boolean;
}

export function Tooltip({ content, children, align = 'center', fullWidth = false }: TooltipProps) {
  const id = useId();
  const alignClass =
    align === 'left' ? 'left-0' : align === 'right' ? 'right-0' : 'left-1/2 -translate-x-1/2';
  const trigger = isValidElement<{ 'aria-describedby'?: string }>(children) ? (
    cloneElement(children as ReactElement<{ 'aria-describedby'?: string }>, {
      'aria-describedby': [children.props['aria-describedby'], id].filter(Boolean).join(' '),
    })
  ) : (
    <span aria-describedby={id} className="inline-flex">
      {children}
    </span>
  );

  return (
    <span className={['group/tooltip relative inline-flex', fullWidth ? 'w-full' : ''].join(' ')}>
      {trigger}
      <span
        id={id}
        role="tooltip"
        className={[
          'pointer-events-none absolute top-full z-20 mt-2 hidden w-max max-w-[min(18rem,calc(100vw-2rem))] border border-[#202b36] bg-[#000102] px-3 py-2 text-left text-xs leading-5 text-slate-300 shadow-none',
          'group-hover/tooltip:block group-focus-within/tooltip:block',
          alignClass,
        ].join(' ')}
      >
        {content}
      </span>
    </span>
  );
}

interface InfoHintProps {
  label?: string;
  children: ReactNode;
}

export function InfoHint({ label = 'Show details', children }: InfoHintProps) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const rootRef = useRef<HTMLSpanElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const closeHint = useCallback(() => {
    setOpen(false);
  }, []);

  useEffect(() => {
    if (!open) return;

    const onPointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      closeHint();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      closeHint();
      requestAnimationFrame(() => triggerRef.current?.focus());
    };

    window.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [closeHint, open]);

  return (
    <span
      ref={rootRef}
      className="relative inline-flex"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onBlur={(event) => {
        const nextTarget = event.relatedTarget instanceof Node ? event.relatedTarget : null;
        if (!event.currentTarget.contains(nextTarget)) closeHint();
      }}
    >
      <button
        ref={triggerRef}
        type="button"
        aria-label={label}
        aria-describedby={id}
        aria-expanded={open}
        aria-controls={id}
        onFocus={() => setOpen(true)}
        onClick={() => setOpen((value) => !value)}
        className="grid h-5 w-5 place-items-center border border-[#202b36] bg-[#000102] font-mono text-[0.65rem] font-semibold text-slate-500 transition-colors hover:border-sky-400/60 hover:text-sky-100"
      >
        <Info className="h-3 w-3" aria-hidden />
      </button>
      {open && (
        <span
          id={id}
          role="tooltip"
          className="absolute left-0 top-full z-20 mt-2 w-max max-w-[min(18rem,calc(100vw-2rem))] border border-[#202b36] bg-[#000102] px-3 py-2 text-left text-xs leading-5 text-slate-300 shadow-none"
        >
          {children}
        </span>
      )}
    </span>
  );
}

export interface MenuAction {
  id: string;
  label: string;
  detail?: string;
  hint?: string;
  disabled?: boolean;
  tone?: 'default' | 'ok' | 'warn' | 'danger';
  onSelect: () => void;
}

interface ActionMenuProps {
  label?: string;
  actions: MenuAction[];
  align?: 'left' | 'right';
}

export function ActionMenu({ label = 'Menu', actions, align = 'right' }: ActionMenuProps) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [menuStyle, setMenuStyle] = useState<CSSProperties>({});
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const firstIndex = firstMenuIndex(actions);
  const placeMenu = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;

    const margin = 8;
    const triggerRect = trigger.getBoundingClientRect();
    const width = Math.min(288, Math.max(192, window.innerWidth - margin * 2));
    const preferredLeft = align === 'right' ? triggerRect.right - width : triggerRect.left;
    const left = Math.min(Math.max(margin, preferredLeft), window.innerWidth - width - margin);
    const belowTop = triggerRect.bottom + 4;
    const belowSpace = window.innerHeight - belowTop - margin;
    const aboveSpace = triggerRect.top - margin;
    const opensAbove = belowSpace < 160 && aboveSpace > belowSpace;
    const maxHeight = Math.min(320, Math.max(96, opensAbove ? aboveSpace : belowSpace));
    const top = opensAbove ? Math.max(margin, triggerRect.top - maxHeight - 4) : belowTop;

    setMenuStyle({ left, top, width, maxHeight });
  }, [align]);

  useEffect(() => {
    if (!open) return;
    placeMenu();

    const onPointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      if (menuRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      requestAnimationFrame(() => triggerRef.current?.focus());
    };
    const onReposition = () => placeMenu();

    window.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', onReposition);
    window.addEventListener('scroll', onReposition, true);
    return () => {
      window.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', onReposition);
      window.removeEventListener('scroll', onReposition, true);
    };
  }, [open, placeMenu]);

  useEffect(() => {
    if (!open) return;
    const index = firstIndex;
    setActiveIndex(index);
    requestAnimationFrame(() => itemRefs.current[index]?.focus());
  }, [firstIndex, open]);

  function focusItem(nextIndex: number) {
    setActiveIndex(nextIndex);
    requestAnimationFrame(() => itemRefs.current[nextIndex]?.focus());
  }

  function close(returnFocus: boolean) {
    setOpen(false);
    if (returnFocus) requestAnimationFrame(() => triggerRef.current?.focus());
  }

  function activate(index: number) {
    const action = actions[index];
    if (!action || action.disabled) return;
    action.onSelect();
    close(true);
  }

  const menu = (
    <div
      ref={menuRef}
      role="menu"
      aria-label={label}
      style={menuStyle}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown') {
          event.preventDefault();
          focusItem(nextMenuIndex(actions, activeIndex, 1));
        } else if (event.key === 'ArrowUp') {
          event.preventDefault();
          focusItem(nextMenuIndex(actions, activeIndex, -1));
        } else if (event.key === 'Home') {
          event.preventDefault();
          focusItem(firstMenuIndex(actions));
        } else if (event.key === 'End') {
          event.preventDefault();
          focusItem(lastMenuIndex(actions));
        } else if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          activate(activeIndex);
        } else if (event.key === 'Tab') {
          close(false);
        }
      }}
      className="fixed z-[1000] overflow-y-auto border border-[#202b36] bg-[linear-gradient(180deg,#030609_0%,#000102_100%)] p-1 shadow-[0_18px_32px_rgba(0,0,0,0.72)]"
    >
      {actions.map((action, index) => (
        <button
          key={action.id}
          ref={(element) => {
            itemRefs.current[index] = element;
          }}
          type="button"
          role="menuitem"
          aria-disabled={action.disabled || undefined}
          tabIndex={index === activeIndex ? 0 : -1}
          onFocus={() => setActiveIndex(index)}
          onClick={() => {
            if (action.disabled) return;
            action.onSelect();
            close(true);
          }}
          className={[
            'grid w-full grid-cols-[minmax(0,1fr)_auto] gap-3 border border-transparent px-2.5 py-2 text-left text-xs leading-snug transition-colors',
            menuTone(action.tone),
            action.disabled
              ? 'cursor-not-allowed opacity-40'
              : 'hover:border-[#202b36] hover:bg-[#05080b]',
          ].join(' ')}
        >
          <span className="min-w-0">
            <span className="block break-words font-medium">{action.label}</span>
            {action.detail && (
              <span className="mt-0.5 block break-words text-[0.7rem] text-slate-400">
                {action.detail}
              </span>
            )}
          </span>
          {action.hint && (
            <span className="self-center border border-[#202b36] px-1.5 py-0.5 font-mono text-[0.6rem] uppercase text-slate-500">
              {action.hint}
            </span>
          )}
        </button>
      ))}
    </div>
  );

  return (
    <div ref={rootRef} className="relative inline-flex">
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            setOpen(true);
          }
        }}
        className="kb-control kb-control-sm font-mono uppercase text-slate-300"
      >
        <Menu className="h-3.5 w-3.5" aria-hidden />
        {label}
        <ChevronDown className="h-3 w-3 text-slate-600" aria-hidden />
      </button>

      {open && typeof document !== 'undefined' ? createPortal(menu, document.body) : null}
    </div>
  );
}

function menuTone(tone: MenuAction['tone'] = 'default'): string {
  switch (tone) {
    case 'ok':
      return 'text-emerald-200';
    case 'warn':
      return 'text-amber-200';
    case 'danger':
      return 'text-red-200';
    default:
      return 'text-slate-200';
  }
}

function firstMenuIndex(actions: MenuAction[]): number {
  const index = actions.findIndex((action) => !action.disabled);
  return index >= 0 ? index : 0;
}

function lastMenuIndex(actions: MenuAction[]): number {
  for (let index = actions.length - 1; index >= 0; index -= 1) {
    if (!actions[index]?.disabled) return index;
  }
  return 0;
}

function nextMenuIndex(actions: MenuAction[], current: number, direction: 1 | -1): number {
  if (actions.length === 0) return 0;

  for (let step = 1; step <= actions.length; step += 1) {
    const index = (current + step * direction + actions.length) % actions.length;
    if (!actions[index]?.disabled) return index;
  }

  return current;
}

interface FieldProps {
  label: string;
  value: string;
  /** Value colour: `default` slate, `ok` emerald, `muted` dim. */
  tone?: 'default' | 'ok' | 'muted';
}

/** A labelled read-only value cell (`<dt>`/`<dd>`), for the panels' `<dl>` grids. */
export function Field({ label, value, tone = 'default' }: FieldProps) {
  const valueClass =
    tone === 'ok' ? 'text-emerald-300' : tone === 'muted' ? 'text-slate-400' : 'text-slate-100';
  return (
    <div className="kb-field-cell">
      <dt className="font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">{label}</dt>
      <dd className={`text-fit mt-2 font-mono text-sm ${valueClass}`}>{value}</dd>
    </div>
  );
}

/**
 * The shared error banner shown inside a panel when a request fails. It renders the
 * passed message verbatim, so callers must pass a user-facing string: authored
 * guidance directly, or a caught exception run through `friendlyPanelError` (see
 * panelError.ts) to sanitize raw wire faults.
 */
export function ErrorBanner({ children }: { children: ReactNode }) {
  return (
    <p
      role="alert"
      className="mb-4 border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-200"
    >
      {children}
    </p>
  );
}

/**
 * The shared success/status banner shown inside a panel after an action resolves
 * (saved, exported, switching…). A polite live region so assistive tech announces
 * the outcome without interrupting; the visual twin of {@link ErrorBanner}.
 */
export function StatusBanner({ children }: { children: ReactNode }) {
  return (
    <p
      role="status"
      className="mb-4 border border-emerald-500/40 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-200"
    >
      {children}
    </p>
  );
}

/**
 * The amber "unsaved edits" chip a persistence panel shows while live changes have
 * not yet been kept with `CONFIG.SAVE`. Shared so every Save surface (Settings, Keymap)
 * flags the same state identically.
 */
export function UnsavedBadge() {
  return (
    <span className="inline-flex items-center gap-2 border border-amber-500/40 bg-amber-500/10 px-3 py-1 font-mono text-xs font-semibold uppercase text-amber-200">
      <span className="h-1.5 w-1.5 bg-amber-400" />
      Unsaved
    </span>
  );
}
