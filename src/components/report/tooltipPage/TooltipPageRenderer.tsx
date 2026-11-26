import React from 'react';
import { Filter as BiFilter, Page } from '../../../types/bi';
import type { DrillFilterSpec } from '../../../engine/drill';
import { VisualCard } from '../VisualCard';
import { ErrorBoundary } from '../../shared/ErrorBoundary';
import { getEffectiveCanvasSize } from '../../../engine/shared/canvasPresets';

interface TooltipPageRendererProps {
  page: Page;
  extraFilters: BiFilter[];
  /** v0.10 Rodada 2 (seams F↔G): filtro via o mecanismo de drill (ver `TooltipHoverState`). */
  extraDrillFilters?: DrillFilterSpec[];
}

/**
 * Renderiza em miniatura uma página marcada como dica de ferramenta,
 * reaproveitando o `VisualCard` normal (mesmo caminho de consulta nativa /
 * `build_filter_context`), só injetando os filtros temporários do ponto sob
 * o mouse via as props `extraFilters`/`extraDrillFilters` — nada é gravado no store.
 */
export const TooltipPageRenderer: React.FC<TooltipPageRendererProps> = ({ page, extraFilters, extraDrillFilters }) => {
  const { width, height } = getEffectiveCanvasSize(page);

  return (
    <div
      data-testid="tooltip-page-renderer"
      style={{ width, height }}
      className="relative bg-white dark:bg-slate-900 overflow-hidden pointer-events-none select-none"
    >
      {page.visuals.map(visual => (
        <ErrorBoundary key={visual.id} fallbackTitle={visual.title}>
          <VisualCard
            visual={visual}
            isSelected={false}
            onSelect={() => {}}
            extraFilters={extraFilters}
            extraDrillFilters={extraDrillFilters}
            isTooltipPreview
          />
        </ErrorBoundary>
      ))}
      {page.visuals.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center text-[10px] text-slate-400">
          Página de dica vazia
        </div>
      )}
    </div>
  );
};
