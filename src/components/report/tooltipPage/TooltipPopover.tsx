import React, { useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Filter as BiFilter, Page } from '../../../types/bi';
import { getEffectiveCanvasSize } from '../../../engine/shared/canvasPresets';
import { computePopoverPosition } from './position';
import { TooltipPageRenderer } from './TooltipPageRenderer';

interface TooltipPopoverProps {
  page: Page;
  filters: BiFilter[];
  anchor: { x: number; y: number };
}

/**
 * Popover flutuante (React portal em document.body) com a página de dica
 * renderizada em miniatura, reposicionado para nunca sair do viewport.
 */
export const TooltipPopover: React.FC<TooltipPopoverProps> = ({ page, filters, anchor }) => {
  const { width, height } = getEffectiveCanvasSize(page);
  const [pos, setPos] = useState(() =>
    computePopoverPosition(anchor, { width, height }, { width: window.innerWidth, height: window.innerHeight })
  );

  useLayoutEffect(() => {
    setPos(computePopoverPosition(anchor, { width, height }, { width: window.innerWidth, height: window.innerHeight }));
  }, [anchor.x, anchor.y, width, height]);

  return createPortal(
    <div
      data-testid="tooltip-page-popover"
      className="fixed z-[999] shadow-2xl rounded-md border border-slate-300 dark:border-slate-700 overflow-hidden"
      style={{ left: pos.x, top: pos.y, width, height }}
    >
      <TooltipPageRenderer page={page} extraFilters={filters} />
    </div>,
    document.body
  );
};
