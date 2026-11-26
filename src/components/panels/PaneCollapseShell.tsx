/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { ChevronRight, ChevronLeft } from 'lucide-react';
import { COLLAPSED_PANE_WIDTH } from '../../store/useUiPrefs';

/**
 * Narrow vertical strip shown when a side pane is collapsed, Power BI
 * Desktop style: icon on top, title rotated 90°, click (or the chevron)
 * expands the pane back. Panes live to the LEFT of the canvas edge they
 * attach to (border-l), hence the chevron points left (expand = ChevronLeft)
 * while collapsed, matching the pane's own "collapse" direction (right).
 */
export const CollapsedPaneStrip: React.FC<{
  icon: React.ElementType;
  iconClassName?: string;
  title: string;
  expandLabel: string;
  onExpand: () => void;
  testId?: string;
}> = ({ icon: Icon, iconClassName, title, expandLabel, onExpand, testId }) => {
  return (
    <button
      type="button"
      onClick={onExpand}
      title={expandLabel}
      data-testid={testId}
      style={{ width: COLLAPSED_PANE_WIDTH }}
      className="border-l border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex flex-col items-center h-full shrink-0 py-2.5 gap-2 cursor-pointer hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-300 select-none"
    >
      <Icon className={iconClassName ?? 'w-4 h-4'} />
      <ChevronLeft className="w-3 h-3 text-slate-400" />
      <span
        className="text-[11px] font-semibold tracking-wide whitespace-nowrap"
        style={{ writingMode: 'vertical-rl', transform: 'rotate(180deg)' }}
      >
        {title}
      </span>
    </button>
  );
};

/** Collapse button added to a pane header (chevron pointing into the pane = collapse). */
export const PaneCollapseButton: React.FC<{
  collapseLabel: string;
  onCollapse: () => void;
  testId?: string;
}> = ({ collapseLabel, onCollapse, testId }) => (
  <button
    type="button"
    onClick={onCollapse}
    title={collapseLabel}
    data-testid={testId}
    className="p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-500 dark:text-slate-400 cursor-pointer"
  >
    <ChevronRight className="w-3.5 h-3.5" />
  </button>
);
