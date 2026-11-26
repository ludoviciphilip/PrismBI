import React, { useState, useMemo } from 'react';
import {
  BarChart,
  BarChart2,
  LineChart,
  PieChart,
  Grid,
  Hash,
  Table as TableIcon,
  Table2,
  Sliders,
  Filter,
  Eye,
  Palette,
  Layers,
  ChevronDown,
  ChevronRight,
  TrendingUp,
  Gauge,
  LayoutGrid,
  ScatterChart,
} from 'lucide-react';
import { VisualType, Visual } from '../../types/bi';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { useTranslation } from '../../i18n';
import { useUiPrefs } from '../../store/useUiPrefs';
import { CollapsedPaneStrip, PaneCollapseButton } from './PaneCollapseShell';
import { ConditionalFormatList } from '../report/conditionalFormat/ConditionalFormatList';
import { SlicerSyncPanel } from '../report/slicer';

const visualTypes: { type: VisualType; label: string; icon: any }[] = [
  { type: 'column', label: 'Colunas', icon: BarChart },
  { type: 'stacked-column', label: 'Colunas Empilhadas', icon: BarChart2 },
  { type: 'bar', label: 'Barras', icon: BarChart },
  { type: 'stacked-bar', label: 'Barras Empilhadas', icon: BarChart2 },
  { type: 'line', label: 'Linhas', icon: LineChart },
  { type: 'area', label: 'Área', icon: TrendingUp },
  { type: 'combo', label: 'Combinado (Coluna + Linha)', icon: BarChart2 },
  { type: 'pie', label: 'Pizza', icon: PieChart },
  { type: 'donut', label: 'Rosca', icon: PieChart },
  { type: 'treemap', label: 'Treemap', icon: Grid },
  { type: 'waterfall', label: 'Cascata', icon: BarChart2 },
  { type: 'funnel', label: 'Funil', icon: Filter },
  { type: 'scatter', label: 'Dispersão', icon: ScatterChart },
  { type: 'gauge', label: 'Medidor (Gauge)', icon: Gauge },
  { type: 'card', label: 'Cartão KPI', icon: Hash },
  { type: 'multi-card', label: 'Cartão Múltiplo', icon: LayoutGrid },
  { type: 'matrix', label: 'Matriz (Pivot)', icon: Table2 },
  { type: 'table', label: 'Tabela', icon: TableIcon },
  { type: 'slicer', label: 'Segmentador', icon: Sliders },
];

export const VisualizationsPane: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const paneState = useUiPrefs(s => s.panes.visualizations);
  const togglePaneCollapsed = useUiPrefs(s => s.togglePaneCollapsed);
  const { pages, activePageId, selectedVisualId, updateVisual, tables, measures, relationships } = useBiStore(useShallow(s => ({
    pages: s.pages,
    activePageId: s.activePageId,
    selectedVisualId: s.selectedVisualId,
    updateVisual: s.updateVisual,
    tables: s.tables,
    measures: s.measures,
    relationships: s.relationships,
  })));
  const [activeTab, setActiveTab] = useState<'build' | 'format'>('build');

  const activePage = pages.find(p => p.id === activePageId);
  const selectedVisual = activePage?.visuals.find(v => v.id === selectedVisualId);

  const selectedTable = useMemo(() => {
    if (!selectedVisual) return undefined;
    return tables.find(t => t.id === selectedVisual.dataConfig.tableId);
  }, [tables, selectedVisual]);

  const relatedTables = useMemo(() => {
    if (!selectedTable || !relationships) return [];
    const relTableIds = new Set<string>();
    relationships.forEach(r => {
      if (!r.isActive) return;
      if (r.fromTable === selectedTable.id || r.fromTable === selectedTable.name) {
        relTableIds.add(r.toTable);
      } else if (r.toTable === selectedTable.id || r.toTable === selectedTable.name) {
        relTableIds.add(r.fromTable);
      }
    });
    return tables.filter(t => t.id !== selectedTable.id && (relTableIds.has(t.id) || relTableIds.has(t.name)));
  }, [selectedTable, relationships, tables]);

  const conditionalFormatFields = useMemo(() => {
    if (!selectedVisual) return [];
    const fields = new Set<string>();
    (selectedVisual.dataConfig.yAxisFields || []).forEach(f => fields.add(f));
    if (selectedVisual.dataConfig.xAxisField) fields.add(selectedVisual.dataConfig.xAxisField);
    if (selectedVisual.dataConfig.legendField) fields.add(selectedVisual.dataConfig.legendField);
    if ((selectedVisual.type === 'table' || selectedVisual.type === 'matrix') && selectedTable) {
      selectedTable.columns.forEach(c => fields.add(c.name));
    }
    return [...fields];
  }, [selectedVisual, selectedTable]);

  const fieldLabels = useMemo(() => {
    if (!selectedVisual) {
      return {
        categoryLabel: 'Eixo X / Categorias',
        categorySubtitle: undefined,
        categoryPlaceholder: '(Selecione a coluna)',
        showCategory: true,
        valueLabel: 'Eixo Y / Valores',
        valueSubtitle: undefined,
        valuePlaceholder: '(Selecione métrica)',
        showSeriesBreakdown: true,
      };
    }
    switch (selectedVisual.type) {
      case 'pie':
      case 'donut':
        return {
          categoryLabel: 'Legenda (Fatias)',
          categorySubtitle: 'Campo para categorizar as fatias da pizza/rosca',
          categoryPlaceholder: '(Selecione a coluna da Legenda)',
          showCategory: true,
          valueLabel: 'Valores',
          valueSubtitle: 'Métrica ou medida para dimensionar as fatias',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: false,
        };

      case 'treemap':
      case 'funnel':
        return {
          categoryLabel: 'Categoria',
          categorySubtitle: 'Campo para agrupar as seções',
          categoryPlaceholder: '(Selecione a categoria)',
          showCategory: true,
          valueLabel: 'Valores',
          valueSubtitle: 'Métrica ou medida para dimensionar as seções',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: false,
        };

      case 'card':
      case 'kpi':
      case 'gauge':
        return {
          categoryLabel: '',
          categoryPlaceholder: '',
          showCategory: false,
          valueLabel: 'Campo de Valor',
          valueSubtitle: 'Indicador numérico ou medida DAX principal',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: false,
        };

      case 'bar':
      case 'stacked-bar':
        return {
          categoryLabel: 'Eixo Y / Categorias',
          categorySubtitle: 'Campo categórico para as barras horizontais',
          categoryPlaceholder: '(Selecione a coluna de categorias)',
          showCategory: true,
          valueLabel: 'Eixo X / Valores',
          valueSubtitle: 'Métrica ou medida para o comprimento das barras',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: true,
        };

      case 'multi-card':
        return {
          categoryLabel: '',
          categoryPlaceholder: '',
          showCategory: false,
          valueLabel: 'Campos / Medidas',
          valueSubtitle: 'Múltiplas métricas exibidas em cartões',
          valuePlaceholder: '(Selecione métricas ou medidas)',
          showSeriesBreakdown: false,
        };

      case 'slicer':
        return {
          categoryLabel: 'Campo de Segmentação',
          categorySubtitle: 'Coluna usada para filtrar os visuais',
          categoryPlaceholder: '(Selecione o campo)',
          showCategory: true,
          valueLabel: 'Valores',
          valuePlaceholder: '(Opcional)',
          showSeriesBreakdown: false,
        };

      case 'matrix':
        return {
          categoryLabel: 'Linhas',
          categorySubtitle: 'Campo para agrupar as linhas da matriz',
          categoryPlaceholder: '(Selecione a coluna de linhas)',
          showCategory: true,
          valueLabel: 'Valores',
          valueSubtitle: 'Métrica ou medida DAX agregada nas células',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: true,
        };

      case 'scatter':
        return {
          categoryLabel: 'Eixo X (Numérico)',
          categorySubtitle: 'Campo numérico para dispersão horizontal',
          categoryPlaceholder: '(Selecione o campo do eixo X)',
          showCategory: true,
          valueLabel: 'Eixo Y (Numérico)',
          valueSubtitle: 'Campo numérico para dispersão vertical',
          valuePlaceholder: '(Selecione o campo do eixo Y)',
          showSeriesBreakdown: false,
        };

      default: // column, stacked-column, line, area, waterfall
        return {
          categoryLabel: 'Eixo X / Categorias',
          categorySubtitle: 'Campo para o eixo horizontal',
          categoryPlaceholder: '(Selecione a coluna do eixo X)',
          showCategory: true,
          valueLabel: 'Eixo Y / Valores',
          valueSubtitle: 'Métrica ou medida para a altura/valores',
          valuePlaceholder: '(Selecione métrica ou medida)',
          showSeriesBreakdown: true,
        };
    }
  }, [selectedVisual?.type]);

  if (paneState === 'collapsed') {
    return (
      <CollapsedPaneStrip
        icon={Layers}
        iconClassName="w-4 h-4 text-slate-500 dark:text-slate-400"
        title={t.panes.visualizations.title}
        expandLabel={t.panes.expand}
        onExpand={() => togglePaneCollapsed('visualizations')}
        testId="visualizations-pane-collapsed"
      />
    );
  }

  if (!selectedVisual) {
    return (
      <div className="w-64 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col h-full text-xs text-slate-700 dark:text-slate-300">
        <div className="p-3 border-b border-slate-200 dark:border-slate-800 font-semibold text-slate-700 dark:text-slate-200 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between">
          <span>{t.panes.visualizations.title}</span>
          <PaneCollapseButton
            collapseLabel={t.panes.collapse}
            onCollapse={() => togglePaneCollapsed('visualizations')}
            testId="visualizations-pane-collapse"
          />
        </div>
        <div className="p-3 grid grid-cols-4 gap-1.5 border-b border-slate-200 dark:border-slate-800">
          {visualTypes.map(v => {
            const Icon = v.icon;
            return (
              <div
                key={v.type}
                title={v.label}
                className="flex flex-col items-center justify-center p-2 rounded border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-800/40 text-slate-400 dark:text-slate-500 opacity-60"
              >
                <Icon className="w-4 h-4" />
              </div>
            );
          })}
        </div>
        <div className="p-4 text-center text-slate-400 dark:text-slate-500 flex flex-col items-center justify-center flex-1">
          <Layers className="w-8 h-8 mb-2 text-slate-300 dark:text-slate-600" />
          <p className="font-medium text-slate-600 dark:text-slate-300">Nenhum visual selecionado</p>
          <p className="text-[11px] text-slate-400 dark:text-slate-500 mt-1">Clique em um elemento do relatório para configurar eixos e formatação.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-68 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col h-full text-xs select-none shadow-xs text-slate-800 dark:text-slate-200">
      {/* Pane Header */}
      <div className="p-2 border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between font-semibold text-slate-700 dark:text-slate-200">
        <span>{t.panes.visualizations.title}</span>
        <div className="flex bg-slate-200/70 dark:bg-slate-800 p-0.5 rounded text-[11px]">
          <button
            onClick={() => setActiveTab('build')}
            className={`px-2 py-0.5 rounded font-medium transition-colors ${
              activeTab === 'build' ? 'bg-white dark:bg-slate-700 text-slate-800 dark:text-slate-100 shadow-xs' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
          >
            {t.panes.visualizations.buildVisual}
          </button>
          <button
            onClick={() => setActiveTab('format')}
            className={`px-2 py-0.5 rounded font-medium transition-colors ${
              activeTab === 'format' ? 'bg-white dark:bg-slate-700 text-slate-800 dark:text-slate-100 shadow-xs' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
          >
            {t.panes.visualizations.formatVisual}
          </button>
        </div>
        <PaneCollapseButton
          collapseLabel={t.panes.collapse}
          onCollapse={() => togglePaneCollapsed('visualizations')}
          testId="visualizations-pane-collapse"
        />
      </div>

      {/* Visual Type Picker Grid */}
      <div className="p-2.5 grid grid-cols-4 gap-1 border-b border-slate-200 dark:border-slate-800 bg-slate-50/40 dark:bg-slate-900/40">
        {visualTypes.map(v => {
          const Icon = v.icon;
          const isCurrent = selectedVisual.type === v.type;
          return (
            <button
              key={v.type}
              onClick={() => updateVisual(selectedVisual.id, { type: v.type })}
              title={v.label}
              className={`flex flex-col items-center justify-center p-1.5 rounded border transition-all ${
                isCurrent
                  ? 'border-amber-500 bg-amber-50 dark:bg-amber-950/40 text-amber-700 dark:text-amber-400 ring-1 ring-amber-400'
                  : 'border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:border-amber-300 dark:hover:border-amber-500/50 hover:bg-amber-50/30 dark:hover:bg-amber-950/20'
              }`}
            >
              <Icon className="w-4 h-4" />
            </button>
          );
        })}
      </div>

      {/* Tab 1: Field Wells (Eixo X, Valores, Legenda, Agregação) */}
      {activeTab === 'build' && (
        <div className="flex-1 overflow-y-auto p-3 space-y-4">
          {/* Tabela de Origem */}
          <div>
            <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 uppercase tracking-wider block mb-1">
              Tabela de Origem
            </label>
            <select
              value={selectedVisual.dataConfig.tableId}
              onChange={(e) =>
                updateVisual(selectedVisual.id, {
                  dataConfig: { ...selectedVisual.dataConfig, tableId: e.target.value },
                })
              }
              className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
            >
              {tables.map(t => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </div>

          {/* Eixo X / Legenda / Categoria */}
          {fieldLabels.showCategory && (
            <div>
              <div className="flex items-center justify-between mb-0.5">
                <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 uppercase tracking-wider block">
                  {fieldLabels.categoryLabel}
                </label>
                {(selectedVisual.type === 'pie' || selectedVisual.type === 'donut') && (
                  <span className="text-[9px] text-amber-700 dark:text-amber-300 bg-amber-100 dark:bg-amber-950/60 px-1.5 py-0.2 rounded font-semibold">
                    Fatias
                  </span>
                )}
              </div>
              {fieldLabels.categorySubtitle && (
                <p className="text-[10px] text-slate-400 dark:text-slate-500 mb-1 leading-tight">
                  {fieldLabels.categorySubtitle}
                </p>
              )}
              <select
                value={selectedVisual.dataConfig.xAxisField || ''}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    dataConfig: { ...selectedVisual.dataConfig, xAxisField: e.target.value },
                  })
                }
                className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
              >
                <option value="">{fieldLabels.categoryPlaceholder}</option>
                <optgroup label={`Tabela Principal (${selectedTable?.name})`}>
                  {selectedTable?.columns.map(c => (
                    <option key={c.name} value={c.name}>
                      {c.name} ({c.dataType})
                    </option>
                  ))}
                </optgroup>
                {relatedTables.map(relT => (
                  <optgroup key={relT.id} label={`Tabela Relacionada (${relT.name})`}>
                    {relT.columns.map(c => (
                      <option key={`${relT.name}.${c.name}`} value={`${relT.name}.${c.name}`}>
                        {relT.name}.{c.name} ({c.dataType})
                      </option>
                    ))}
                  </optgroup>
                ))}
              </select>
            </div>
          )}

          {/* Valores / Métrica */}
          <div>
            <div className="flex items-center justify-between mb-0.5">
              <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 uppercase tracking-wider block">
                {fieldLabels.valueLabel}
              </label>
              {measures.some(m => m.name === selectedVisual.dataConfig.yAxisFields?.[0]) && (
                <span className="text-[9px] text-purple-700 dark:text-purple-300 bg-purple-100 dark:bg-purple-950/60 px-1.5 py-0.2 rounded font-semibold">
                  Medida DAX
                </span>
              )}
            </div>
            {fieldLabels.valueSubtitle && (
              <p className="text-[10px] text-slate-400 dark:text-slate-500 mb-1 leading-tight">
                {fieldLabels.valueSubtitle}
              </p>
            )}
            <select
              value={selectedVisual.dataConfig.yAxisFields?.[0] || ''}
              onChange={(e) => {
                const val = e.target.value;
                const isMeasure = measures.some(m => m.name === val || `Σ ${m.name}` === val);
                updateVisual(selectedVisual.id, {
                  dataConfig: {
                    ...selectedVisual.dataConfig,
                    yAxisFields: [val],
                    aggregation: isMeasure ? 'NONE' : (selectedVisual.dataConfig.aggregation === 'NONE' ? 'SUM' : selectedVisual.dataConfig.aggregation || 'SUM'),
                  },
                });
              }}
              className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
            >
              <option value="">{fieldLabels.valuePlaceholder}</option>
              <optgroup label="Medidas Calculadas (DAX)">
                {measures.map(m => (
                  <option key={m.name} value={m.name}>
                    Σ {m.name}
                  </option>
                ))}
              </optgroup>
              <optgroup label={`Colunas Numéricas (${selectedTable?.name})`}>
                {selectedTable?.columns
                  .filter(c => c.dataType === 'number' || c.dataType === 'currency' || c.dataType === 'percentage')
                  .map(c => (
                    <option key={c.name} value={c.name}>
                      {c.name}
                    </option>
                  ))}
              </optgroup>
              {relatedTables.map(relT => (
                <optgroup key={relT.id} label={`Colunas Numéricas (${relT.name})`}>
                  {relT.columns
                    .filter(c => c.dataType === 'number' || c.dataType === 'currency' || c.dataType === 'percentage')
                    .map(c => (
                      <option key={`${relT.name}.${c.name}`} value={`${relT.name}.${c.name}`}>
                        {relT.name}.{c.name}
                      </option>
                    ))}
                </optgroup>
              ))}
            </select>
          </div>

          {/* Agregação */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 uppercase tracking-wider">
                Função de Agregação
              </label>
              {measures.some(m => m.name === selectedVisual.dataConfig.yAxisFields?.[0]) && (
                <span className="text-[9px] text-purple-700 dark:text-purple-300 bg-purple-100 dark:bg-purple-950/60 px-1.5 py-0.2 rounded font-semibold">
                  Medida DAX (Já agregada)
                </span>
              )}
            </div>
            <div className="grid grid-cols-3 gap-1">
              {(['NONE', 'SUM', 'AVG', 'COUNT', 'MIN', 'MAX', 'DISTINCTCOUNT'] as const).map(agg => {
                const isSelected =
                  (selectedVisual.dataConfig.aggregation ||
                    (measures.some(m => m.name === selectedVisual.dataConfig.yAxisFields?.[0]) ? 'NONE' : 'SUM')) === agg;
                return (
                  <button
                    key={agg}
                    onClick={() =>
                      updateVisual(selectedVisual.id, {
                        dataConfig: { ...selectedVisual.dataConfig, aggregation: agg },
                      })
                    }
                    className={`py-1 px-1 rounded border text-[10px] font-medium text-center truncate cursor-pointer ${
                      isSelected
                        ? 'bg-amber-100 dark:bg-amber-950/60 border-amber-500 text-amber-900 dark:text-amber-300 font-bold shadow-2xs'
                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-700'
                    }`}
                    title={
                      agg === 'NONE'
                        ? 'Nenhuma: não realiza agregações adicionais (ideal para medidas DAX)'
                        : agg === 'DISTINCTCOUNT'
                        ? 'Contagem Distinta'
                        : undefined
                    }
                  >
                    {agg === 'NONE'
                      ? 'Nenhuma'
                      : agg === 'SUM'
                      ? 'Soma'
                      : agg === 'AVG'
                      ? 'Média'
                      : agg === 'COUNT'
                      ? 'Contagem'
                      : agg === 'DISTINCTCOUNT'
                      ? 'Distintos'
                      : agg}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Legenda / Série (apenas para gráficos com quebra de série dimensional) */}
          {fieldLabels.showSeriesBreakdown && selectedVisual.type !== 'slicer' && (
            <div>
              <label className="text-[11px] font-semibold text-slate-600 uppercase tracking-wider block mb-1">
                Legenda de Série (Opcional)
              </label>
              <select
                value={selectedVisual.dataConfig.legendField || ''}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    dataConfig: { ...selectedVisual.dataConfig, legendField: e.target.value || undefined },
                  })
                }
                className="w-full bg-slate-50 border border-slate-300 rounded px-2 py-1 text-slate-800 focus:outline-none focus:ring-1 focus:ring-amber-500"
              >
                <option value="">(Sem quebra de série)</option>
                {selectedTable?.columns.map(c => (
                  <option key={c.name} value={c.name}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>
          )}

          {selectedVisual.type === 'matrix' && (
            <div>
              <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 uppercase tracking-wider block mb-1">
                Sub-linha (Hierarquia Expansível)
              </label>
              <select
                value={selectedVisual.dataConfig.drillFields?.[0] || ''}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    dataConfig: {
                      ...selectedVisual.dataConfig,
                      drillFields: e.target.value ? [e.target.value] : [],
                    },
                  })
                }
                className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
              >
                <option value="">(Nenhuma sub-linha)</option>
                {selectedTable?.columns.map(c => (
                  <option key={c.name} value={c.name}>
                    {c.name}
                  </option>
                ))}
                {relatedTables.map(relT =>
                  relT.columns.map(c => (
                    <option key={`${relT.name}.${c.name}`} value={`${relT.name}.${c.name}`}>
                      {relT.name}.{c.name}
                    </option>
                  ))
                )}
              </select>
            </div>
          )}
        </div>
      )}

      {/* Tab 2: Format Pane (Título, Cores, Rótulos, Fundo) */}
      {activeTab === 'format' && (
        <div className="flex-1 overflow-y-auto p-3 space-y-4">
          {/* Título */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-[11px] font-semibold text-slate-700">Título do Visual</label>
              <input
                type="checkbox"
                checked={selectedVisual.formatConfig.showTitle}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    formatConfig: { ...selectedVisual.formatConfig, showTitle: e.target.checked },
                  })
                }
                className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
              />
            </div>
            {selectedVisual.formatConfig.showTitle && (
              <input
                type="text"
                value={selectedVisual.title}
                onChange={(e) => updateVisual(selectedVisual.id, { title: e.target.value })}
                className="w-full bg-slate-50 border border-slate-300 rounded px-2 py-1 text-slate-800 focus:outline-none focus:ring-1 focus:ring-amber-500"
              />
            )}
          </div>

          {/* Rótulos de dados */}
          <div className="flex items-center justify-between py-1 border-t border-slate-100">
            <span className="text-[11px] font-medium text-slate-700">Rótulos de Dados</span>
            <input
              type="checkbox"
              checked={selectedVisual.formatConfig.showDataLabels ?? false}
              onChange={(e) =>
                updateVisual(selectedVisual.id, {
                  formatConfig: { ...selectedVisual.formatConfig, showDataLabels: e.target.checked },
                })
              }
              className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
            />
          </div>

          {/* Legenda */}
          <div className="flex items-center justify-between py-1 border-t border-slate-100">
            <span className="text-[11px] font-medium text-slate-700">Exibir Legenda</span>
            <input
              type="checkbox"
              checked={selectedVisual.formatConfig.showLegend ?? true}
              onChange={(e) =>
                updateVisual(selectedVisual.id, {
                  formatConfig: { ...selectedVisual.formatConfig, showLegend: e.target.checked },
                })
              }
              className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
            />
          </div>

          {/* Mostrar BLANK como 0 */}
          <div className="flex items-center justify-between py-1 border-t border-slate-100">
            <div>
              <span className="text-[11px] font-medium text-slate-700 block">Mostrar BLANK como 0</span>
              <span className="text-[9px] text-slate-400 block">Exibe 0 em vez de vazio para valores nulos</span>
            </div>
            <input
              type="checkbox"
              checked={selectedVisual.formatConfig.showBlankAsZero ?? false}
              onChange={(e) =>
                updateVisual(selectedVisual.id, {
                  formatConfig: { ...selectedVisual.formatConfig, showBlankAsZero: e.target.checked },
                })
              }
              className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
            />
          </div>

          {/* Unidades Compactas & Sinal */}
          <div className="space-y-2 border-t border-slate-100 dark:border-slate-800 pt-2">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-[11px] font-medium text-slate-700 dark:text-slate-200 block">Unidades de Exibição</span>
                <span className="text-[9px] text-slate-400 block">Exibir valores exatos ou abreviados (Mil/Mi)</span>
              </div>
              <select
                value={selectedVisual.formatConfig.displayUnits || 'none'}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    formatConfig: {
                      ...selectedVisual.formatConfig,
                      displayUnits: e.target.value as 'none' | 'auto',
                    },
                  })
                }
                className="bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-0.5 text-[11px] text-slate-700 dark:text-slate-200"
              >
                <option value="none">Exato</option>
                <option value="auto">Compacto (Mil / Mi)</option>
              </select>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-[11px] font-medium text-slate-700 dark:text-slate-200 block">Colorir Positivo/Negativo</span>
                <span className="text-[9px] text-slate-400 block">Destaca verde/vermelho em cartões KPI</span>
              </div>
              <input
                type="checkbox"
                checked={selectedVisual.formatConfig.colorBySign ?? false}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    formatConfig: { ...selectedVisual.formatConfig, colorBySign: e.target.checked },
                  })
                }
                className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
              />
            </div>

            {selectedVisual.type === 'slicer' && (
              <div className="flex items-center justify-between">
                <span className="text-[11px] font-medium text-slate-700 dark:text-slate-200">Estilo do Segmentador</span>
                <select
                  value={selectedVisual.dataConfig.slicerConfig?.mode || 'dropdown'}
                  onChange={(e) =>
                    updateVisual(selectedVisual.id, {
                      dataConfig: {
                        ...selectedVisual.dataConfig,
                        slicerConfig: {
                          ...selectedVisual.dataConfig.slicerConfig,
                          mode: e.target.value as NonNullable<Visual['dataConfig']['slicerConfig']>['mode'],
                        },
                      },
                    })
                  }
                  className="bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-0.5 text-[11px] text-slate-700 dark:text-slate-200"
                >
                  <option value="dropdown">Suspenso (Dropdown)</option>
                  <option value="list">Lista</option>
                  <option value="tiles">Blocos / Botões (Tiles)</option>
                  <option value="between">{t.slicer.between}</option>
                  <option value="relative-date">{t.slicer.relativeDate}</option>
                </select>
              </div>
            )}

            {selectedVisual.type === 'slicer' &&
              ['list', 'dropdown', 'tiles'].includes(selectedVisual.dataConfig.slicerConfig?.mode || 'dropdown') && (
                <div className="flex items-center justify-between">
                  <span className="text-[11px] font-medium text-slate-700 dark:text-slate-200">{t.slicer.showSearchBox}</span>
                  <input
                    type="checkbox"
                    checked={selectedVisual.dataConfig.slicerConfig?.showSearch ?? false}
                    onChange={(e) =>
                      updateVisual(selectedVisual.id, {
                        dataConfig: {
                          ...selectedVisual.dataConfig,
                          slicerConfig: { ...selectedVisual.dataConfig.slicerConfig, mode: selectedVisual.dataConfig.slicerConfig?.mode || 'dropdown', showSearch: e.target.checked },
                        },
                      })
                    }
                    className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
                  />
                </div>
              )}

            {selectedVisual.type === 'slicer' && (
              <SlicerSyncPanel
                visual={selectedVisual}
                pages={pages}
                onChange={(patch) =>
                  updateVisual(selectedVisual.id, {
                    dataConfig: {
                      ...selectedVisual.dataConfig,
                      slicerConfig: {
                        ...selectedVisual.dataConfig.slicerConfig,
                        mode: selectedVisual.dataConfig.slicerConfig?.mode || 'dropdown',
                        ...patch,
                      },
                    },
                  })
                }
              />
            )}
          </div>

          {/* Paleta de Cores */}
          <div className="space-y-1.5 border-t border-slate-100 pt-2">
            <label className="text-[11px] font-semibold text-slate-700 block">Paleta de Cores</label>
            <div className="grid grid-cols-2 gap-1.5">
              {[
                { name: 'Padrão Power BI', colors: ['#f59e0b', '#3b82f6', '#10b981', '#8b5cf6'] },
                { name: 'Azul Corporativo', colors: ['#0284c7', '#0369a1', '#38bdf8', '#7dd3fc'] },
                { name: 'Esmeralda & Ouro', colors: ['#059669', '#d97706', '#10b981', '#f59e0b'] },
                { name: 'Vibrante Moderno', colors: ['#ec4899', '#8b5cf6', '#06b6d4', '#10b981'] },
              ].map(palette => (
                <button
                  key={palette.name}
                  onClick={() =>
                    updateVisual(selectedVisual.id, {
                      formatConfig: { ...selectedVisual.formatConfig, colorPalette: palette.colors },
                    })
                  }
                  className="flex flex-col p-1.5 border border-slate-200 rounded hover:border-amber-400 bg-white"
                >
                  <span className="text-[10px] text-slate-600 mb-1 truncate">{palette.name}</span>
                  <div className="flex gap-0.5 w-full h-2 rounded overflow-hidden">
                    {palette.colors.map((c, i) => (
                      <div key={i} style={{ backgroundColor: c }} className="flex-1 h-full"></div>
                    ))}
                  </div>
                </button>
              ))}
            </div>
          </div>

          {/* Fundo e Bordas */}
          <div className="space-y-2 border-t border-slate-100 pt-2">
            <label className="text-[11px] font-semibold text-slate-700 block">Bordas e Cantos</label>
            <div className="flex items-center justify-between">
              <span className="text-slate-600 text-[11px]">Raio do Canto (px)</span>
              <input
                type="number"
                min="0"
                max="24"
                value={selectedVisual.formatConfig.borderRadius ?? 8}
                onChange={(e) =>
                  updateVisual(selectedVisual.id, {
                    formatConfig: { ...selectedVisual.formatConfig, borderRadius: Number(e.target.value) },
                  })
                }
                className="w-16 bg-slate-50 border border-slate-300 rounded px-1.5 py-0.5 text-center text-xs"
              />
            </div>
          </div>

          {/* Formatação condicional */}
          <div className="space-y-2 border-t border-slate-100 dark:border-slate-800 pt-2">
            <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 block">Formatação condicional</label>
            <ConditionalFormatList
              visual={selectedVisual}
              availableFields={conditionalFormatFields}
              measureFields={measures.filter(m => m.tableName === selectedTable?.name || m.tableId === selectedTable?.id).map(m => m.name)}
              onChange={(formats) =>
                updateVisual(selectedVisual.id, {
                  formatConfig: { ...selectedVisual.formatConfig, conditionalFormats: formats },
                })
              }
            />
          </div>
        </div>
      )}
    </div>
  );
});
