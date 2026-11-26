import { useCallback, useEffect, useRef, useState } from 'react';
import { Filter as BiFilter } from '../../../types/bi';
import type { DrillFilterSpec } from '../../../engine/drill';
import { HoverDebouncer } from './hoverDebouncer';

export interface TooltipHoverState {
  pageId: string;
  filters: BiFilter[];
  /** v0.10 Rodada 2 (seams F↔G): filtro temporário via o mecanismo de drill
   * (ancestrais + nível atual / caminho da linha), para níveis drillados ou de
   * hierarquia de data — nunca um `Filter` de coluna que não existe no modelo. */
  drillFilters?: DrillFilterSpec[];
  anchor: { x: number; y: number };
}

/**
 * Hook fino sobre `HoverDebouncer`: agenda a abertura do popover ~150ms
 * depois do hover (`show`) e cancela tanto o timer pendente quanto o
 * popover já aberto ao sair do ponto (`hide`). Reentradas rápidas em pontos
 * diferentes sempre cancelam o show anterior antes de agendar o novo.
 */
export function useTooltipHover(delayMs = 150) {
  const [popover, setPopover] = useState<TooltipHoverState | null>(null);
  const debouncerRef = useRef<HoverDebouncer<TooltipHoverState> | null>(null);

  if (!debouncerRef.current) {
    debouncerRef.current = new HoverDebouncer<TooltipHoverState>(delayMs, (payload) => setPopover(payload));
  }

  useEffect(() => () => debouncerRef.current?.cancel(), []);

  const show = useCallback((state: TooltipHoverState) => {
    debouncerRef.current?.schedule(state);
  }, []);

  const hide = useCallback(() => {
    debouncerRef.current?.cancel();
    setPopover(null);
  }, []);

  return { popover, show, hide };
}
