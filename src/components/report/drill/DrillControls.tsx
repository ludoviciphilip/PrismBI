import React, { useEffect, useRef } from 'react';
import { ArrowUp, CircleArrowDown, ChevronsDown, GitFork, ChevronsUpDown, ChevronsDownUp, ListTree } from 'lucide-react';
import type { Translations } from '../../../i18n';

/**
 * Botões de drill no cabeçalho do visual (visíveis no hover), no mesmo padrão
 * de hover/foco/desabilitado+tooltip do Power Query.
 */
export const DRILL_BUTTON_CLASS =
  'w-6 h-6 shrink-0 inline-flex items-center justify-center rounded text-slate-500 dark:text-slate-400 ' +
  'hover:text-amber-600 dark:hover:text-amber-400 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 ' +
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 ' +
  'disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-slate-500';

interface DrillButtonProps {
  label: string;
  title: string;
  disabled?: boolean;
  pressed?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

const DrillButton: React.FC<DrillButtonProps> = ({ label, title, disabled, pressed, onClick, children }) => (
  <button
    type="button"
    aria-label={label}
    title={title}
    disabled={disabled}
    aria-pressed={pressed}
    onClick={e => {
      e.stopPropagation();
      if (!disabled) onClick();
    }}
    className={`${DRILL_BUTTON_CLASS} ${pressed ? 'text-amber-600 bg-amber-100 dark:bg-amber-950/60 ring-1 ring-amber-300 dark:ring-amber-600' : ''}`}
  >
    {children}
  </button>
);

/** Substitui `{level}` por `value` numa string i18n (ver `drill.drillUpHint`/`nextLevelHint`). */
function withLevel(template: string, value: string): string {
  return template.replace('{level}', value);
}

export interface ChartDrillControlsProps {
  levelCaption: string;
  nextCaption?: string;
  canUp: boolean;
  canDown: boolean;
  drillOn: boolean;
  onUp: () => void;
  onToggleDrill: () => void;
  onNextLevel: () => void;
  onExpandAll: () => void;
  t: Translations;
}

export const ChartDrillControls: React.FC<ChartDrillControlsProps> = p => {
  const d = p.t.drill;
  const lastHint = d.atLowestLevel;
  return (
    <div role="group" aria-label={d.controlsGroupLabel} data-testid="drill-controls" className="flex items-center gap-0.5 shrink-0 mr-0.5 pr-1 border-r border-slate-200 dark:border-slate-700">
      <DrillButton
        label={d.drillUp}
        title={p.canUp ? withLevel(d.drillUpHint, p.levelCaption) : d.drillUpAtTop}
        disabled={!p.canUp}
        onClick={p.onUp}
      >
        <ArrowUp className="w-3.5 h-3.5" />
      </DrillButton>
      <DrillButton
        label={d.drillDownMode}
        title={
          !p.canDown
            ? `${d.drillDownMode}: ${lastHint.toLowerCase()}`
            : p.drillOn
              ? d.drillDownModeOnHint
              : d.drillDownModeOffHint
        }
        disabled={!p.canDown}
        pressed={p.drillOn && p.canDown}
        onClick={p.onToggleDrill}
      >
        <CircleArrowDown className="w-3.5 h-3.5" />
      </DrillButton>
      <DrillButton
        label={d.nextLevel}
        title={p.canDown ? withLevel(d.nextLevelHint, p.nextCaption ?? '') : `${d.nextLevel}: ${lastHint.toLowerCase()}`}
        disabled={!p.canDown}
        onClick={p.onNextLevel}
      >
        <ChevronsDown className="w-3.5 h-3.5" />
      </DrillButton>
      <DrillButton
        label={d.expandAllLevels}
        title={p.canDown ? d.expandAllLevelsHint : `${d.expandAllLevels}: ${lastHint.toLowerCase()}`}
        disabled={!p.canDown}
        onClick={p.onExpandAll}
      >
        <GitFork className="w-3.5 h-3.5 rotate-180" />
      </DrillButton>
    </div>
  );
};

export interface MatrixDrillControlsProps {
  canExpand: boolean;
  canCollapse: boolean;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  onExpandOneLevel: () => void;
  t: Translations;
}

export const MatrixDrillControls: React.FC<MatrixDrillControlsProps> = p => {
  const d = p.t.drill;
  return (
    <div role="group" aria-label={d.matrixControlsGroupLabel} data-testid="matrix-drill-controls" className="flex items-center gap-0.5 shrink-0 mr-0.5 pr-1 border-r border-slate-200 dark:border-slate-700">
      <DrillButton
        label={d.expandOneLevel}
        title={p.canExpand ? d.expandOneLevelHint : d.expandOneLevelDoneHint}
        disabled={!p.canExpand}
        onClick={p.onExpandOneLevel}
      >
        <ListTree className="w-3.5 h-3.5" />
      </DrillButton>
      <DrillButton
        label={d.expandAll}
        title={p.canExpand ? d.expandAllHint : d.expandAllDoneHint}
        disabled={!p.canExpand}
        onClick={p.onExpandAll}
      >
        <ChevronsUpDown className="w-3.5 h-3.5" />
      </DrillButton>
      <DrillButton
        label={d.collapseAll}
        title={p.canCollapse ? d.collapseAllHint : d.collapseAllDoneHint}
        disabled={!p.canCollapse}
        onClick={p.onCollapseAll}
      >
        <ChevronsDownUp className="w-3.5 h-3.5" />
      </DrillButton>
    </div>
  );
};

export interface DrillMenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  title?: string;
}

export const DRILL_MENU_WIDTH = 208;

/** Menu de contexto (botão direito num ponto do gráfico). Posição fixa, presa à janela. */
export const DrillContextMenu: React.FC<{ x: number; y: number; items: DrillMenuItem[]; onClose: () => void; menuLabel: string }> = ({
  x,
  y,
  items,
  onClose,
  menuLabel,
}) => {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    ref.current?.querySelector<HTMLButtonElement>('button:not([disabled])')?.focus();
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  const vw = typeof window !== 'undefined' ? window.innerWidth : 1366;
  const vh = typeof window !== 'undefined' ? window.innerHeight : 768;
  const height = items.length * 30 + 8;
  const left = Math.max(4, Math.min(x, vw - DRILL_MENU_WIDTH - 4));
  const top = Math.max(4, Math.min(y, vh - height - 4));

  return (
    <div
      ref={ref}
      role="menu"
      aria-label={menuLabel}
      data-testid="drill-context-menu"
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
      onContextMenu={e => e.preventDefault()}
      style={{ position: 'fixed', left, top, width: DRILL_MENU_WIDTH, zIndex: 10000 }}
      className="w-52 py-1 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-xl text-[12px]"
    >
      {items.map(it => (
        <button
          key={it.label}
          type="button"
          role="menuitem"
          disabled={it.disabled}
          title={it.title}
          onClick={() => {
            if (it.disabled) return;
            it.onSelect();
            onClose();
          }}
          className="w-full flex items-center gap-2 text-left px-3 py-1.5 text-slate-700 dark:text-slate-200 cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800 active:bg-slate-100 dark:active:bg-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-inset disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
        >
          {it.label}
        </button>
      ))}
    </div>
  );
};
