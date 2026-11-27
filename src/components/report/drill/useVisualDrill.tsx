import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { DataType, Filter, Table, Visual } from '../../../types/bi';
import { useBiStore } from '../../../store/useBiStore';
import { globalDataEngine } from '../../../engine/dataEngine';
import { queryVisualNative } from '../../../bridge/tauriBridge';
import { useTranslation } from '../../../i18n';
import { applyVisualSort, fieldMatches, VisualSortSpec } from '../../../engine/visualSort';
import {
  COLLAPSED_MATRIX,
  DRILLABLE_VISUAL_TYPES,
  DrillFilterSpec,
  DrillLevel,
  MatrixExpansion,
  buildDrillPathFilters,
  buildHoverFilters,
  buildPathFilters,
  canDrillDown,
  canDrillUp,
  drillDownOnPointPatch,
  drillUpPatch,
  expandAllLevelsPatch,
  expandAllMatrix,
  expandOneLevel,
  getDrillState,
  goToNextLevelPatch,
  groupingLevels,
  resolveDrillLevels,
  resolveMatrixRowLevels,
} from '../../../engine/drill';
import {
  baseNativeRequest,
  levelsFromNative,
  matrixFromNative,
  queryLevelsTs,
  queryMatrixTs,
  toNativeDrillFilters,
  toNativeLevels,
} from './engineQuery';
import { useEngineQuery } from './useEngineQuery';
import { ChartDrillControls, DrillContextMenu, DrillMenuItem, MatrixDrillControls } from './DrillControls';
import { HierarchicalMatrix } from './HierarchicalMatrix';

/** Tipo da coluna de um campo do visual ("Tabela.Coluna" ou só "Coluna"). */
export function makeColumnTypeLookup(tables: Table[], baseTableId: string) {
  return (field: string): DataType | undefined => {
    const find = (t: Table | undefined, c: string) => t?.columns.find(col => col.name.toLowerCase() === c.toLowerCase())?.dataType;
    if (field.includes('.')) {
      const [tn, cn] = field.split('.', 2);
      const t = tables.find(x => x.id === tn || x.name === tn);
      if (t) return find(t, cn);
    }
    const base = tables.find(t => t.id === baseTableId || t.name === baseTableId);
    const own = find(base, field);
    if (own) return own;
    for (const t of tables) {
      const dt = find(t, field);
      if (dt) return dt;
    }
    return undefined;
  };
}

export interface VisualDrill {
  /** Resultado do eixo hierárquico (já ordenado); null = visual sem hierarquia (caminho antigo). */
  chartResult: { categories: string[]; series: { name: string; data: (number | null)[] }[] } | null;
  hoverProps: Pick<React.HTMLAttributes<HTMLDivElement>, 'onMouseEnter' | 'onMouseLeave' | 'onFocusCapture'>;
  header: React.ReactNode;
  overlay: React.ReactNode;
  /** Clique num ponto: true quando o drill tratou o clique (o VisualCard não faz o filtro cruzado antigo). */
  handlePointClick: (params: ChartPoint) => boolean;
  handleContextMenu: (params: ChartPoint) => void;
  /**
   * v0.10 Rodada 2 (seams F↔G): filtro temporário completo (ancestrais do drill +
   * nível atual) para a dica de página ao passar o mouse num ponto do gráfico pelo
   * rótulo (`name`/`data.name` do ECharts). Retorna `[]` quando o visual não tem
   * hierarquia de drill — o VisualCard então usa o `xAxisField` cru, como antes.
   */
  resolveHoverDrillFilters: (label: unknown) => DrillFilterSpec[];
  /** Matriz com 2+ níveis: renderiza a matriz hierárquica; null = matriz antiga (1 nível).
   * `onRowHover`/`onRowLeave` (seams F↔G) recebem o filtro do caminho INTEIRO da linha
   * (todos os níveis acima, não só o primeiro) para a dica de página. */
  renderMatrix:
    | ((
        isCurrencyField: (f: string) => boolean,
        onRowHover?: (e: React.MouseEvent, filters: DrillFilterSpec[]) => void,
        onRowLeave?: () => void
      ) => React.ReactNode)
    | null;
}

export interface VisualDrillOptions {
  /** O visual usa o motor nativo (Tauri) — mesmo critério do VisualCard. */
  native: boolean;
  onShowTable: () => void;
  onDrillthrough: (field: string, value: unknown) => void;
  /**
   * v0.10 Rodada 2 (seams F↔G): filtros extras (ex.: a dica de página de OUTRO
   * visual, filtrando este pelo ponto sob o mouse) somados aos ancestrais do
   * próprio drill. Nunca gravado no store — só restringe esta consulta.
   */
  extraDrillFilters?: DrillFilterSpec[];
}

interface ChartPoint {
  name?: string;
  data?: { name?: string };
  event?: { event?: MouseEvent };
}

export function useVisualDrill(visual: Visual, filters: Filter[], opts: VisualDrillOptions): VisualDrill {
  const tables = useBiStore(s => s.tables);
  const measuresCount = useBiStore(s => s.measures.length);
  const dataVersion = useBiStore(s => s.dataVersion);
  const updateVisual = useBiStore(s => s.updateVisual);
  const setCrossFilter = useBiStore(s => s.setCrossFilter);
  const { t } = useTranslation();
  const dc = visual.dataConfig;
  const typeOf = useMemo(() => makeColumnTypeLookup(tables, dc.tableId), [tables, dc.tableId]);

  const optsRef = useRef(opts);
  optsRef.current = opts;
  const [hovered, setHovered] = useState(false);
  const [drillOn, setDrillOn] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; keys: unknown[] } | null>(null);
  const [expansion, setExpansion] = useState<MatrixExpansion>(() => dc.matrixExpansion ?? COLLAPSED_MATRIX);
  // v0.10 Rodada 2 (seams E↔G): a expansão da matriz é estado local (não persistido
  // no projeto), mas um indicador (Bookmark) PRECISA restaurá-la. `dc.matrixExpansion`
  // só muda quando algo de fora (a aplicação do indicador) grava um valor novo — a
  // edição normal (+/−, "expandir tudo") nunca escreve em `dc`, só em `expansion` — então
  // sincronizar aqui nunca sobrescreve um toggle manual do usuário.
  const lastSyncedExpansion = useRef(dc.matrixExpansion);
  useEffect(() => {
    if (dc.matrixExpansion !== lastSyncedExpansion.current) {
      lastSyncedExpansion.current = dc.matrixExpansion;
      setExpansion(dc.matrixExpansion ?? COLLAPSED_MATRIX);
    }
  }, [dc.matrixExpansion]);

  const applyPatch = useCallback(
    (patch: Partial<Visual['dataConfig']> | null) => {
      if (patch) updateVisual(visual.id, { dataConfig: { ...visual.dataConfig, ...patch } });
    },
    [updateVisual, visual.id, visual.dataConfig]
  );

  // ----- Gráficos (eixo com hierarquia) -----
  const extraDrillFilters = opts.extraDrillFilters ?? [];
  const hasExtraDrill = extraDrillFilters.length > 0;
  const { chartLevels, state, groupLv, drillFilters } = useMemo(() => {
    const lv: DrillLevel[] = DRILLABLE_VISUAL_TYPES.has(visual.type) ? resolveDrillLevels(dc, typeOf) : [];
    const st = getDrillState(dc, lv);
    // v0.10 Rodada 2 (seams F↔G): a dica de página de OUTRO visual pode precisar
    // restringir esta consulta mesmo quando este visual não tem hierarquia própria
    // (ex.: um gráfico de mês simples, filtrado pelo ano/trimestre do ponto sob o mouse).
    const active = lv.length > 0 && (lv.length > 1 || hasExtraDrill);
    return {
      chartLevels: lv,
      state: st,
      groupLv: active ? groupingLevels(st, lv) : [],
      drillFilters: active ? [...buildDrillPathFilters(st, lv), ...extraDrillFilters] : [],
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visual.type, dc, typeOf, hasExtraDrill, JSON.stringify(extraDrillFilters)]);
  // `chartActive` controla a UI de drill (cabeçalho, menu, canUp/canDown): só quando o
  // visual TEM hierarquia própria. `queryActive` controla a consulta: também quando só
  // há `extraDrillFilters` (dica de página de outro visual), sem mostrar controles de drill.
  const chartActive = chartLevels.length > 1;
  const queryActive = groupLv.length > 0;
  const chartKey = queryActive
    ? JSON.stringify([dataVersion, tables.length, measuresCount, visual.id, visual.type, dc.tableId, dc.yAxisFields, dc.legendField, dc.aggregation, groupLv, drillFilters, filters])
    : '';
  const chartQuery = useEngineQuery(
    chartKey,
    queryActive,
    opts.native,
    () => queryLevelsTs(globalDataEngine, { visual, filters, levels: groupLv, drillFilters }),
    () =>
      queryVisualNative({
        ...baseNativeRequest(visual, filters),
        categoryLevels: toNativeLevels(groupLv),
        drillFilters: toNativeDrillFilters(drillFilters),
      }).then(r => levelsFromNative(r, groupLv))
  );

  const sortedChart = useMemo(() => {
    const raw = chartQuery.data;
    if (!queryActive || !raw) return null;
    const base = globalDataEngine.getVisualSortSpec(visual);
    const single = groupLv.length === 1 ? groupLv[0] : null;
    const keepBaseKeys = Boolean(single && !single.datePart && fieldMatches(single.field, dc.xAxisField || ''));
    const spec: VisualSortSpec = {
      sortField: dc.sortField,
      sortOrder: dc.sortOrder,
      limit: dc.limit,
      categoryFields: [...(base?.categoryFields ?? []), ...groupLv.map(l => l.field)],
      valueFields: [...(base?.valueFields ?? dc.yAxisFields ?? []), ...raw.series.map(s => s.name)],
      // Partes de data e tuplas seguem a ordem do motor (cronológica), não a alfabética do rótulo.
      categorySortKeys: keepBaseKeys ? base?.categorySortKeys : new Map<string, unknown>(raw.categories.map((c, i) => [c, i])),
    };
    const sorted = applyVisualSort({ categories: raw.categories, series: raw.series }, spec);
    const keyByLabel = new Map(raw.categories.map((c, i) => [c, raw.categoryKeys[i]]));
    return { ...sorted, keyByLabel };
  }, [queryActive, chartQuery.data, visual, dc.sortField, dc.sortOrder, dc.limit, dc.xAxisField, dc.yAxisFields, groupLv]);

  const canUp = canDrillUp(state);
  const canDown = chartActive && canDrillDown(state, chartLevels);
  const plainLevel = groupLv.length === 1 && !groupLv[0].datePart ? groupLv[0] : null;

  const handlePointClick = useCallback(
    (params: ChartPoint): boolean => {
      if (!chartActive || !sortedChart) return false;
      const label = params?.name ?? params?.data?.name;
      const keys = label !== undefined ? sortedChart.keyByLabel.get(label) : undefined;
      if (!keys) return false;
      if (drillOn) {
        const patch = drillDownOnPointPatch(dc, chartLevels, keys);
        if (patch) {
          applyPatch(patch);
          return true;
        }
      }
      // Filtro cruzado pelo campo do nível atual (não pelo campo original do eixo).
      if (plainLevel) {
        setCrossFilter(dc.tableId, plainLevel.field, keys[0]);
        optsRef.current.onDrillthrough(plainLevel.field, keys[0]);
      }
      return true;
    },
    [chartActive, sortedChart, drillOn, dc, chartLevels, applyPatch, plainLevel, setCrossFilter]
  );

  const handleContextMenu = useCallback(
    (params: ChartPoint) => {
      if (!chartActive || !sortedChart) return;
      const ev = params?.event?.event;
      ev?.preventDefault?.();
      const label = params?.name ?? params?.data?.name;
      const keys = label !== undefined ? sortedChart.keyByLabel.get(label) : undefined;
      if (!keys) return;
      setMenu({ x: ev?.clientX ?? 0, y: ev?.clientY ?? 0, keys });
    },
    [chartActive, sortedChart]
  );

  const menuItems: DrillMenuItem[] = menu
    ? [
        {
          label: t.drill.drillDownAction,
          disabled: !canDown,
          title: canDown ? t.drill.drillDownActionHint.replace('{level}', chartLevels[state.level + 1]?.caption ?? '') : t.drill.atLowestLevel,
          onSelect: () => applyPatch(drillDownOnPointPatch(dc, chartLevels, menu.keys)),
        },
        {
          label: t.drill.drillUp,
          disabled: !canUp,
          title: canUp ? t.drill.drillUpActionHint : t.drill.drillUpAtTop,
          onSelect: () => applyPatch(drillUpPatch(dc, chartLevels)),
        },
        { label: t.drill.showAsTable, title: t.drill.showAsTableHint, onSelect: () => optsRef.current.onShowTable() },
        {
          label: t.drill.drillthroughAction,
          disabled: !plainLevel,
          title: plainLevel ? t.drill.drillthroughActionHint : t.drill.drillthroughUnavailableHint,
          onSelect: () => plainLevel && optsRef.current.onDrillthrough(plainLevel.field, menu.keys[0]),
        },
      ]
    : [];

  // ----- Matriz (N níveis) -----
  const matrixLevels: DrillLevel[] = useMemo(
    () => (visual.type === 'matrix' ? resolveMatrixRowLevels(dc, typeOf) : []),
    [visual.type, dc, typeOf]
  );
  // `matrixActive` (UI: header de drill) só com hierarquia própria; a consulta roda
  // também só com `extraDrillFilters` (dica de página, seams F↔G), igual ao gráfico.
  const matrixActive = matrixLevels.length > 1;
  const matrixQueryActive = matrixLevels.length > 0 && (matrixActive || hasExtraDrill);
  const matrixKey = matrixQueryActive
    ? JSON.stringify([dataVersion, tables.length, measuresCount, visual.id, dc.tableId, dc.yAxisFields, dc.legendField, dc.aggregation, matrixLevels, expansion, filters, extraDrillFilters])
    : '';
  const matrixQuery = useEngineQuery(
    matrixKey,
    matrixQueryActive,
    opts.native,
    () => queryMatrixTs(globalDataEngine, { visual, filters, levels: matrixLevels, expansion, drillFilters: extraDrillFilters }),
    () =>
      queryVisualNative({
        ...baseNativeRequest(visual, filters),
        xAxisField: matrixLevels[0]?.field,
        matrixRows: toNativeLevels(matrixLevels),
        matrixExpandDepth: expansion.depth,
        matrixExpanded: expansion.expanded.map(id => JSON.parse(id) as unknown[]),
        matrixCollapsed: expansion.collapsed.map(id => JSON.parse(id) as unknown[]),
        drillFilters: toNativeDrillFilters(extraDrillFilters),
      }).then(r => matrixFromNative(r, matrixLevels))
  );

  const canExpandMatrix = expansion.depth < matrixLevels.length - 1 || expansion.collapsed.length > 0;
  const canCollapseMatrix = expansion.depth > 0 || expansion.expanded.length > 0;

  let header: React.ReactNode = null;
  if (hovered && chartActive) {
    header = (
      <ChartDrillControls
        levelCaption={chartLevels[state.level]?.caption ?? ''}
        nextCaption={chartLevels[state.level + 1]?.caption}
        canUp={canUp}
        canDown={canDown}
        drillOn={drillOn}
        onUp={() => applyPatch(drillUpPatch(dc, chartLevels))}
        onToggleDrill={() => setDrillOn(v => !v)}
        onNextLevel={() => applyPatch(goToNextLevelPatch(dc, chartLevels))}
        onExpandAll={() => applyPatch(expandAllLevelsPatch(dc, chartLevels))}
        t={t}
      />
    );
  } else if (hovered && matrixActive) {
    header = (
      <MatrixDrillControls
        canExpand={canExpandMatrix}
        canCollapse={canCollapseMatrix}
        onExpandOneLevel={() => setExpansion(e => expandOneLevel(e, matrixLevels.length))}
        onExpandAll={() => setExpansion(expandAllMatrix(matrixLevels.length))}
        onCollapseAll={() => setExpansion(COLLAPSED_MATRIX)}
        t={t}
      />
    );
  }

  const chartResult = useMemo(
    () => (sortedChart ? { categories: sortedChart.categories, series: sortedChart.series } : null),
    [sortedChart]
  );

  const resolveHoverDrillFilters = useCallback(
    (label: unknown): DrillFilterSpec[] => {
      if (!sortedChart) return [];
      const keys = sortedChart.keyByLabel.get(label as string);
      if (!keys) return [];
      return buildHoverFilters(state, chartLevels.length ? chartLevels : groupLv, keys);
    },
    [sortedChart, state, chartLevels, groupLv]
  );

  return {
    chartResult,
    hoverProps: {
      onMouseEnter: () => setHovered(true),
      onMouseLeave: () => setHovered(false),
      onFocusCapture: () => setHovered(true),
    },
    header,
    overlay: menu ? <DrillContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} menuLabel={t.drill.pointMenuLabel} /> : null,
    handlePointClick,
    handleContextMenu,
    resolveHoverDrillFilters,
    renderMatrix: matrixQueryActive
      ? (isCurrencyField, onRowHover, onRowLeave) => (
          <HierarchicalMatrix
            visual={visual}
            levels={matrixLevels}
            data={matrixQuery.data}
            error={matrixQuery.error}
            expansion={expansion}
            onExpansionChange={setExpansion}
            isCurrencyField={isCurrencyField}
            onRowHover={
              onRowHover ? (e, path) => onRowHover(e, buildPathFilters(matrixLevels, path)) : undefined
            }
            onRowLeave={onRowLeave}
          />
        )
      : null,
  };
}
