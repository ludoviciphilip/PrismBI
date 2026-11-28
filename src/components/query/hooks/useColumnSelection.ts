// Seleção múltipla de colunas na grade de preview: clique seleciona,
// Ctrl/Cmd+clique alterna, Shift+clique seleciona um intervalo (como no Power Query).

import { useCallback, useState } from 'react';

export interface ColumnClickModifiers {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}

/** Lógica pura (sem React) — testada diretamente, sem precisar montar componentes. */
export function computeNextSelection(
  columnOrder: string[],
  prevSelected: string[],
  anchor: string | null,
  column: string,
  mods: ColumnClickModifiers,
): { selected: string[]; anchor: string } {
  if (mods.shiftKey && anchor && columnOrder.includes(anchor)) {
    const ai = columnOrder.indexOf(anchor);
    const ci = columnOrder.indexOf(column);
    if (ai >= 0 && ci >= 0) {
      // A ordem segue o gesto (âncora → coluna clicada), como no Power Query:
      // clicar em "Nome do Mês" e Shift+clicar em "Ano" (à esquerda) dá
      // [Nome do Mês, Ano], e o Mesclar Colunas sai "Abr-2026".
      const range = columnOrder.slice(Math.min(ai, ci), Math.max(ai, ci) + 1);
      return { selected: ai <= ci ? range : range.reverse(), anchor };
    }
  }
  if (mods.ctrlKey || mods.metaKey) {
    const next = prevSelected.includes(column) ? prevSelected.filter(c => c !== column) : [...prevSelected, column];
    return { selected: next, anchor: column };
  }
  return { selected: [column], anchor: column };
}

export function useColumnSelection(columnOrder: string[]) {
  const [selected, setSelected] = useState<string[]>([]);
  const [anchor, setAnchor] = useState<string | null>(null);

  const handleColumnClick = useCallback(
    (column: string, mods: ColumnClickModifiers = {}) => {
      setSelected(prev => {
        const next = computeNextSelection(columnOrder, prev, anchor, column, mods);
        setAnchor(next.anchor);
        return next.selected;
      });
    },
    [columnOrder, anchor],
  );

  const clearSelection = useCallback(() => {
    setSelected([]);
    setAnchor(null);
  }, []);

  const isSelected = useCallback((column: string) => selected.includes(column), [selected]);

  return { selected, isSelected, handleColumnClick, clearSelection, setSelected };
}
