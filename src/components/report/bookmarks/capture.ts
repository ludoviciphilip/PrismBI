/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { Bookmark, BookmarkVisualState, Filter, Page, Visual } from '../../../types/bi';

export type BookmarkCapture = NonNullable<Bookmark['capture']>;
export const DEFAULT_BOOKMARK_CAPTURE: BookmarkCapture = { data: true, display: true, currentPage: true };

export type BookmarkScope = NonNullable<Bookmark['scope']>;

function scopeMatcher(scope: BookmarkScope | undefined): (visualId: string) => boolean {
  if (!scope || scope === 'all') return () => true;
  const ids = new Set(scope);
  return (visualId: string) => ids.has(visualId);
}

export interface CaptureBookmarkInput {
  activePageId: string;
  /** Todos os filtros ativos no relatório no momento da captura (report/page/visual). */
  filters: Filter[];
  /** Visuais da página ativa (ou de todas as páginas, se a captura precisar olhar fora da página atual). */
  visuals: Visual[];
  capture?: Partial<BookmarkCapture>;
  scope?: BookmarkScope;
}

export type CapturedBookmarkFields = Pick<
  Bookmark,
  'activePageId' | 'filters' | 'visualState' | 'slicerSelections' | 'scope' | 'capture'
>;

/**
 * Captura puramente o estado atual do relatório num formato de Bookmark —
 * dados (filtros + seleção de segmentadores), exibição (visibilidade, nível
 * de drill, ordenação) e página atual — respeitando `capture` e `scope`,
 * como no painel "Indicadores" do Power BI Desktop.
 */
export function captureBookmark(input: CaptureBookmarkInput): CapturedBookmarkFields {
  const capture: BookmarkCapture = { ...DEFAULT_BOOKMARK_CAPTURE, ...input.capture };
  const scope: BookmarkScope = input.scope ?? 'all';
  const inScope = scopeMatcher(scope);

  const filters = capture.data
    ? input.filters.filter(f => f.level !== 'visual' || !f.visualId || inScope(f.visualId))
    : [];

  const visualState: Record<string, BookmarkVisualState> = {};
  const slicerSelections: Record<string, unknown> = {};

  for (const visual of input.visuals) {
    if (!inScope(visual.id)) continue;

    if (capture.display) {
      visualState[visual.id] = {
        hidden: visual.formatConfig?.hidden ?? false,
        drillLevel: visual.dataConfig?.currentDrillLevel,
        sortField: visual.dataConfig?.sortField,
        sortOrder: visual.dataConfig?.sortOrder,
        // v0.10 Rodada 2 (seams E↔G): sem isto, restaurar um indicador num nível > 0
        // mostra "todos os valores" daquele nível (drillMode) em vez do ponto escolhido
        // (drillPath); a matriz perde quais linhas estavam expandidas.
        drillMode: visual.dataConfig?.drillMode,
        drillPath: visual.dataConfig?.drillPath,
        matrixExpansion: visual.dataConfig?.matrixExpansion,
      };
    }

    if (capture.data && visual.dataConfig?.slicerConfig) {
      const { selectedValues, between, relative } = visual.dataConfig.slicerConfig;
      slicerSelections[visual.id] = { selectedValues, between, relative };
    }
  }

  return {
    activePageId: input.activePageId,
    filters,
    visualState: Object.keys(visualState).length > 0 ? visualState : undefined,
    slicerSelections: Object.keys(slicerSelections).length > 0 ? slicerSelections : undefined,
    scope,
    capture,
  };
}

export interface ApplyBookmarkContext {
  pages: Page[];
  filters: Filter[];
  activePageId: string;
}

export interface ApplyBookmarkPatch {
  activePageId?: string;
  filters?: Filter[];
  pages?: Page[];
}

/**
 * Aplica puramente um Bookmark a um estado dado, devolvendo só os campos que
 * precisam mudar (patch), respeitando `capture` (dados/exibição/página
 * atual) e `scope` (todos os visuais ou só os selecionados na captura).
 */
export function applyBookmark(bookmark: Bookmark, ctx: ApplyBookmarkContext): ApplyBookmarkPatch {
  const capture: BookmarkCapture = { ...DEFAULT_BOOKMARK_CAPTURE, ...bookmark.capture };
  const scope: BookmarkScope = bookmark.scope ?? 'all';
  const inScope = scopeMatcher(scope);
  const scopedAll = scope === 'all';

  const patch: ApplyBookmarkPatch = {};

  if (capture.currentPage) {
    patch.activePageId = bookmark.activePageId;
  }

  if (capture.data) {
    if (scopedAll) {
      patch.filters = [...bookmark.filters];
    } else {
      const untouched = ctx.filters.filter(f => !(f.level === 'visual' && f.visualId && inScope(f.visualId)));
      patch.filters = [...untouched, ...bookmark.filters];
    }
  }

  const hasVisualPatches = capture.display || (capture.data && !!bookmark.slicerSelections);
  if (hasVisualPatches) {
    patch.pages = ctx.pages.map(page => ({
      ...page,
      visuals: page.visuals.map(visual => {
        if (!inScope(visual.id)) return visual;
        let next = visual;

        if (capture.display) {
          const vs = bookmark.visualState?.[visual.id];
          if (vs) {
            next = {
              ...next,
              formatConfig: {
                ...next.formatConfig,
                hidden: vs.hidden ?? next.formatConfig?.hidden,
              },
              dataConfig: {
                ...next.dataConfig,
                currentDrillLevel: vs.drillLevel ?? next.dataConfig?.currentDrillLevel,
                sortField: vs.sortField ?? next.dataConfig?.sortField,
                sortOrder: vs.sortOrder ?? next.dataConfig?.sortOrder,
                // v0.10 Rodada 2 (seams E↔G): `drillMode`/`drillPath` são lidos a cada
                // render por `getDrillState` (sem estado React próprio), então só
                // gravar aqui já restaura o ponto certo do drill. `matrixExpansion`
                // sincroniza de volta para o `useState` local em `useVisualDrill`.
                drillMode: vs.drillMode ?? next.dataConfig?.drillMode,
                drillPath: vs.drillPath ?? next.dataConfig?.drillPath,
                matrixExpansion: vs.matrixExpansion ?? next.dataConfig?.matrixExpansion,
              },
            };
          }
        }

        if (capture.data) {
          const sel = bookmark.slicerSelections?.[visual.id] as Record<string, unknown> | undefined;
          if (sel && next.dataConfig?.slicerConfig) {
            next = {
              ...next,
              dataConfig: {
                ...next.dataConfig,
                slicerConfig: { ...next.dataConfig.slicerConfig, ...sel } as typeof next.dataConfig.slicerConfig,
              },
            };
          }
        }

        return next;
      }),
    }));
  }

  return patch;
}
