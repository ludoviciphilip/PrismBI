/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * v0.10 (Indicadores/Bookmarks): normaliza indicadores salvos em projetos
 * antigos — que só tinham `id`, `name`, `activePageId` e `filters` — para o
 * formato novo, preenchendo defaults que preservam o comportamento de
 * antes (captura tudo, escopo = todos os visuais, sem grupo/ordem).
 */

import { Bookmark } from '../../types/bi';

export function migrateBookmarks(raw: unknown): Bookmark[] {
  if (!Array.isArray(raw)) return [];
  return raw.map((b: any, index: number): Bookmark => ({
    id: b?.id ?? `bm_${index}`,
    name: b?.name ?? `Indicador ${index + 1}`,
    activePageId: b?.activePageId ?? '',
    filters: Array.isArray(b?.filters) ? b.filters : [],
    order: typeof b?.order === 'number' ? b.order : index,
    groupId: b?.groupId,
    capture: b?.capture ?? { data: true, display: true, currentPage: true },
    visualState: b?.visualState,
    slicerSelections: b?.slicerSelections,
    scope: b?.scope ?? 'all',
  }));
}
