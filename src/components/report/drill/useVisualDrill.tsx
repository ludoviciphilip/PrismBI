import React, { useCallback, useMemo, useRef, useState } from 'react';
import { DataType, Filter, Table, Visual } from '../../../types/bi';
import { useBiStore } from '../../../store/useBiStore';
import { globalDataEngine } from '../../../engine/dataEngine';
import { queryVisualNative } from '../../../bridge/tauriBridge';
import { applyVisualSort, fieldMatches, VisualSortSpec } from '../../../engine/visualSort';
import {
  COLLAPSED_MATRIX,
  DRILLABLE_VISUAL_TYPES,
  DrillLevel,
  MatrixExpansion,
  buildDrillPathFilters,
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

export interface VisualDrillOptions {
  /** O visual usa o motor nativo (Tauri) — mesmo critério do VisualCard. */
  native: boolean;
  onShowTable: () => void;
  onDrillthrough: (field: string, value: unknown) => void;
}

interface ChartPoint {
  name?: string;
  data?: { name?: string };
  event?: { event?: MouseEvent };
}

export interface VisualDrill {
  /** Resultado do eixo hierárquico (já ordenado); null = visual sem hierarquia (caminho antigo). */
  chartResult: { categories: string[]; series: { name: string; data: number[] }[] } | null;
  hoverProps: Pick<React.HTMLAttributes<HTMLDivElement>, 'onMouseEnter' | 'onMouseLeave' | 'onFocusCapture'>;
  header: React.ReactNode;
  overlay: React.ReactNode;
  /** Clique num ponto: true quando o drill tratou o clique (o VisualCard não faz o filtro cruzado antigo). */
  handlePointClick: (params: ChartPoint) => boolean;
  handleContextMenu: (params: ChartPoint) => void;
  /** Matriz com 2+ níveis: renderiza a matriz hierárquica; null = matriz antiga (1 nível). */
  renderMatrix: ((isCurrencyField: (f: string) => boolean) => React.ReactNode) | null;
}

export function useVisualDrill(visual: Visual, filters: Filter[], opts: VisualDrillOptions): VisualDrill {
  const tables = useBiStore(s => s.tables);
  const measuresCount = useBiStore(s => s.measures.length);
  const dataVersion = useBiStore(s => s.dataVersion);
  const updateVisual = useBiStore(s => s.updateVisual);
  const setCrossFilter = useBiStore(s => s.setCrossFilter);
  const dc = visual.dataConfig;
  const typeOf = useMemo(() => makeColumnTypeLookup(tables, dc.tableId), [tables, dc.tableId]);

  const optsRef = useRef(opts);
  optsRef.current = opts;
  const [hovered, setHovered] = useState(false);
  const [drillOn, setDrillOn] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; keys: unknown[] } | null>(null);
  const [expansion, setExpansion] = useState<MatrixExpansion>(COLLAPSED_MATRIX);

  const applyPatch = useCallback(
    (patch: Partial<Visual['dataConfig']> | null) => {
      if (patch) updateVisual(visual.id, { dataConfig: { ...visual.dataConfig, ...patch } });
    },
    [updateVisual, visual.id, visual.dataConfig]
  );

  // ----- Gráficos (eixo com hierarquia) -----
  const { chartLevels, state, groupLv, drillFilters } = useMemo(() => {
    const lv: DrillLevel[] = DRILLABLE_VISUAL_TYPES.has(visual.type) ? resolveDrillLevels(dc, typeOf) : [];
    const st = getDrillState(dc, lv);
    const active = lv.length > 1;
    return {
      chartLevels: lv,
      state: st,
      groupLv: active ? groupingLevels(st, lv) : [],
      drillFilters: active ? buildDrillPathFilters(st, lv) : [],
    };
  }, [visual.type, dc, typeOf]);
  const chartActive = chartLevels.length > 1;
  const chartKey = chartActive
    ? JSON.stringify([dataVersion, tables.length, measuresCount, visual.id, visual.type, dc.tableId, dc.yAxisFields, dc.legendField, dc.aggregation, groupLv, drillFilters, filters])
    : '';
  const chartQuery = useEngineQuery(
    chartKey,
    chartActive,
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
    if (!chartActive || !raw) return null;
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
  }, [chartActive, chartQuery.data, visual, dc.sortField, dc.sortOrder, dc.limit, dc.xAxisField, dc.yAxisFields, groupLv]);

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
          label: 'Fazer drill down',
          disabled: !canDown,
          title: canDown ? `Descer para ${chartLevels[state.level + 1]?.caption ?? ''} filtrando por este ponto` : 'Já está no nível mais baixo da hierarquia',
          onSelect: () => applyPatch(drillDownOnPointPatch(dc, chartLevels, menu.keys)),
        },
        {
          label: 'Fazer drill up',
          disabled: !canUp,
          title: canUp ? 'Subir um nível na hierarquia' : 'Já está no nível mais alto',
          onSelect: () => applyPatch(drillUpPatch(dc, chartLevels)),
        },
        { label: 'Mostrar como tabela', title: 'Mostrar os dados do visual como tabela', onSelect: () => optsRef.current.onShowTable() },
        {
          label: 'Drill-through',
          disabled: !plainLevel,
          title: plainLevel ? 'Detalhar este ponto em outra página' : 'Drill-through indisponível em níveis de data ou rótulos concatenados',
          onSelect: () => plainLevel && optsRef.current.onDrillthrough(plainLevel.field, menu.keys[0]),
        },
      ]
    : [];

  // ----- Matriz (N níveis) -----
  const matrixLevels: DrillLevel[] = useMemo(
    () => (visual.type === 'matrix' ? resolveMatrixRowLevels(dc, typeOf) : []),
    [visual.type, dc, typeOf]
  );
  const matrixActive = matrixLevels.length > 1;
  const matrixKey = matrixActive
    ? JSON.stringify([dataVersion, tables.length, measuresCount, visual.id, dc.tableId, dc.yAxisFields, dc.legendField, dc.aggregation, matrixLevels, expansion, filters])
    : '';
  const matrixQuery = useEngineQuery(
    matrixKey,
    matrixActive,
    opts.native,
    () => queryMatrixTs(globalDataEngine, { visual, filters, levels: matrixLevels, expansion }),
    () =>
      queryVisualNative({
        ...baseNativeRequest(visual, filters),
        xAxisField: matrixLevels[0]?.field,
        matrixRows: toNativeLevels(matrixLevels),
        matrixExpandDepth: expansion.depth,
        matrixExpanded: expansion.expanded.map(id => JSON.parse(id) as unknown[]),
        matrixCollapsed: expansion.collapsed.map(id => JSON.parse(id) as unknown[]),
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
      />
    );
  }

  const chartResult = useMemo(
    () => (sortedChart ? { categories: sortedChart.categories, series: sortedChart.series } : null),
    [sortedChart]
  );

  return {
    chartResult,
    hoverProps: {
      onMouseEnter: () => setHovered(true),
      onMouseLeave: () => setHovered(false),
      onFocusCapture: () => setHovered(true),
    },
    header,
    overlay: menu ? <DrillContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} /> : null,
    handlePointClick,
    handleContextMenu,
    renderMatrix: matrixActive
      ? isCurrencyField => (
          <HierarchicalMatrix
            visual={visual}
            levels={matrixLevels}
            data={matrixQuery.data}
            error={matrixQuery.error}
            expansion={expansion}
            onExpansionChange={setExpansion}
            isCurrencyField={isCurrencyField}
          />
        )
      : null,
  };
}
