/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { useEffect } from 'react';
import { useBiStore } from '../../store/useBiStore';
import { ActiveView } from '../../types/bi';
import { ShortcutScope } from './types';
import { SHORTCUTS, isMacPlatform, isFocusInEditable, matchesCombo } from './registry';

/** Mapeia a view ativa do app para o escopo de atalhos (as 5 com escopo próprio). */
function activeViewToScope(view: ActiveView): ShortcutScope | null {
  switch (view) {
    case 'report':
    case 'data':
    case 'model':
    case 'query':
    case 'daxquery':
      return view;
    default:
      return null;
  }
}

/**
 * Hook global único de atalhos de teclado (v0.11). Monta UM listener em
 * `window` (capture=false) para todo o app — nenhuma outra tela deve
 * adicionar atalhos novos fora do registro em `registry.ts`.
 *
 * Regras:
 * - Ignora atalhos simples quando o foco está num input/textarea/
 *   contenteditable/editor de fórmulas, EXCETO os marcados com
 *   `allowInEditable` (Ctrl+S, F5, navegação de página, etc.).
 * - Respeita o escopo por view (Relatório, Dados, Modelo, Power Query,
 *   DAX Query); atalhos sem `scope` são globais.
 * - Nunca despacha itens `handledElsewhere` — esses já têm listener
 *   próprio existente no código (Ctrl+Z, Ctrl+F1, Ctrl+Enter, Esc etc.),
 *   evitando duplicar ou quebrar atalhos antigos.
 * - F5 NUNCA recarrega o WebView (preventDefault sempre), mesmo fora do
 *   escopo de qualquer atalho tratado — perder o estado do projeto não é
 *   uma opção.
 */
export function useGlobalShortcuts(): void {
  useEffect(() => {
    const isMac = isMacPlatform();

    const handleKeyDown = (e: KeyboardEvent) => {
      // Garantia dura: F5 nunca recarrega o app, independente de qualquer
      // outra checagem de escopo/foco abaixo.
      if (e.key === 'F5' && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey) {
        e.preventDefault();
      }

      const activeView = useBiStore.getState().activeView;
      const currentScope = activeViewToScope(activeView);
      const editable = isFocusInEditable();

      for (const def of SHORTCUTS) {
        if (def.handledElsewhere || !def.action) continue;
        if (def.scope && (!currentScope || !def.scope.includes(currentScope))) continue;
        if (editable && !def.allowInEditable) continue;

        const matched = def.combos.some(combo => matchesCombo(e, combo, isMac));
        if (!matched) continue;

        e.preventDefault();
        def.action(e);
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
}
