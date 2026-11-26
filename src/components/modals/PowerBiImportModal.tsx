import React, { useState } from 'react';
import { createPortal } from 'react-dom';
import {
  FileCode2,
  Table as TableIcon,
  GitBranch,
  Sigma,
  LayoutGrid,
  AlertTriangle,
  CheckCircle2,
  Upload,
  Layers,
} from 'lucide-react';
import {
  PbiProjectSummary,
  isTauriRuntime,
  openPbipProject,
  openPbixFile,
  pickPowerBiFile,
} from '../../bridge/tauriBridge';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { Filter, Measure, Page, Relationship, Table, Visual, VisualType } from '../../types/bi';
import { CURRENT_ENGINE_VERSION } from '../../engine/migrations';
import { drillConfigFromPbiMeta } from '../report/drill/pbiImport';
import { conditionalFormatsFromPbiMeta } from '../../engine/conditionalFormat';

interface PowerBiImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  initialPath?: string | null;
  onLoaded?: () => void;
}

export const PowerBiImportModal: React.FC<PowerBiImportModalProps> = ({
  isOpen,
  onClose,
  initialPath,
  onLoaded,
}) => {
  const { loadStateFromProject, setActiveView, projectName, addRecentProject, setShowWelcomeScreen } = useBiStore(
    useShallow(s => ({
      loadStateFromProject: s.loadStateFromProject,
      setActiveView: s.setActiveView,
      projectName: s.projectName,
      addRecentProject: s.addRecentProject,
      setShowWelcomeScreen: s.setShowWelcomeScreen,
    }))
  );
  const [summary, setSummary] = useState<PbiProjectSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [manualPath, setManualPath] = useState('');
  const [activeTab, setActiveTab] = useState<'overview' | 'measures' | 'pages' | 'm'>('overview');

  React.useEffect(() => {
    if (isOpen && initialPath && initialPath.trim()) {
      setManualPath(initialPath);
      void inspectPath(initialPath);
    }
  }, [isOpen, initialPath]);

  if (!isOpen) return null;

  const inspectPath = async (path: string) => {
    setLoading(true);
    setError(null);
    try {
      const lower = path.toLowerCase();
      const res = lower.endsWith('.pbix')
        ? await openPbixFile(path)
        : await openPbipProject(path);
      setSummary(res);
      setActiveTab('overview');
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setLoading(false);
    }
  };

  const handlePickNative = async () => {
    if (!isTauriRuntime()) {
      setError('Abertura direta de .pbip/.pbix requer o app desktop PrismBI (Tauri).');
      return;
    }
    const picked = await pickPowerBiFile();
    if (picked) {
      setManualPath(picked);
      await inspectPath(picked);
    }
  };

  const handleApplyToModel = () => {
    if (!summary) return;

    const loadedMap = new Map(
      (summary.loadedTables || []).map(lt => [lt.tableName.toLowerCase(), lt])
    );

    const mappedTables: Table[] = summary.tables.map((t, idx) => {
      const loaded = loadedMap.get(t.name.toLowerCase());
      const rows = loaded?.rows || [];
      const colNamesFromLoaded = new Map(
        (loaded?.columns || []).map(lc => [lc.name.toLowerCase(), lc])
      );

      const cols = t.columns.map((c, cIdx) => ({
        id: `${t.name}_col_${cIdx}`,
        name: c.name,
        originalName: c.name,
        dataType: ((c.dataType as any) || colNamesFromLoaded.get(c.name.toLowerCase())?.dataType || 'text'),
        format: c.formatString || undefined,
        sortByColumn: c.sortByColumn || undefined,
        isHidden: c.isHidden,
        isCalculated: c.isCalculated,
        expression: c.expression || undefined,
      }));

      // Include any extra columns discovered during Excel/CSV load
      for (const lc of loaded?.columns || []) {
        if (!cols.some(c => c.name.toLowerCase() === lc.name.toLowerCase())) {
          cols.push({
            id: `${t.name}_col_${cols.length}`,
            name: lc.name,
            originalName: lc.originalName,
            dataType: (lc.dataType as any) || 'text',
            format: undefined,
            sortByColumn: undefined,
            isHidden: false,
            isCalculated: false,
            expression: undefined,
          });
        }
      }

      const mPartition = t.partitions.find(p => p.sourceType.toLowerCase() === 'm') || t.partitions[0];
      const sourcePathLower = (loaded?.sourcePath || '').toLowerCase();
      const sourceConfig = loaded?.sourcePath
        ? {
            type: (sourcePathLower.endsWith('.csv') || sourcePathLower.endsWith('.tsv')
              ? 'csv'
              : sourcePathLower.endsWith('.parquet')
              ? 'parquet'
              : sourcePathLower.endsWith('.json') || sourcePathLower.endsWith('.ndjson')
              ? 'json'
              : 'xlsx') as 'xlsx' | 'csv' | 'parquet' | 'json',
            filePath: loaded.sourcePath,
            sheetName: loaded.sheetName || undefined,
            mScript: mPartition?.expression || undefined,
            pbipRoot: summary.rootPath,
            lastRefreshedAt: new Date().toISOString(),
          }
        : mPartition?.sourceType
        ? {
            type: 'pbip' as const,
            mScript: mPartition.expression || undefined,
            pbipRoot: summary.rootPath,
            lastRefreshedAt: new Date().toISOString(),
          }
        : undefined;

      return {
        id: t.name,
        name: t.name,
        originalName: t.name,
        sourceConfig,
        isCalculated: t.isCalculated,
        isHidden: t.isHidden,
        x: 80 + (idx % 3) * 280,
        y: 80 + Math.floor(idx / 3) * 240,
        columns: cols,
        rows,
        duckdbTable: loaded?.tableName,
        rowsTruncated: loaded?.truncated,
        totalRows: loaded?.totalRows,
      };
    });

    const mappedMeasures: Measure[] = summary.tables.flatMap(t =>
      t.measures.map((m, mIdx) => {
        const fs = m.formatString || '';
        const format: Measure['format'] = fs.includes('%')
          ? 'percentage'
          : fs.includes('$') || fs.includes('€') || fs.includes('R$')
          ? 'currency'
          : 'number';
        return {
          id: `pbi_m_${t.name}_${mIdx}`,
          name: m.name,
          tableName: t.name,
          tableId: t.name,
          formula: m.expression,
          format,
          formatString: m.formatString || undefined,
          displayFolder: m.displayFolder || undefined,
          description: m.description || undefined,
        };
      })
    );

    const mappedRelationships: Relationship[] = summary.relationships.map((r, idx) => ({
      id: `pbi_rel_${idx}`,
      fromTable: r.fromTable,
      fromColumn: r.fromColumn,
      toTable: r.toTable,
      toColumn: r.toColumn,
      cardinality: (r.cardinality as Relationship['cardinality']) || 'N:1',
      crossFilterDirection: r.crossFilterDirection === 'both' ? 'both' : 'single',
      isActive: r.isActive,
    }));

    const mappedPages: Page[] =
      summary.pages.length > 0
        ? summary.pages.map((p, pIdx) => ({
            id: p.id || `pbi_page_${pIdx + 1}`,
            name: p.name || `Página ${pIdx + 1}`,
            order: pIdx,
            canvasSize: {
              preset: 'custom',
              width: Math.round(p.width || 1280),
              height: Math.round(p.height || 720),
            },
            visuals: p.visuals.map((v, vIdx): Visual => {
              const vType = (v.visualType as VisualType) || 'column';
              const minW = vType === 'action-button' ? 40 : 120;
              const minH = vType === 'action-button' ? 32 : 70;
              return {
                id: v.id || `pbi_vis_${pIdx}_${vIdx}`,
                type: vType,
                title: v.title || `Visual ${vIdx + 1}`,
                x: Math.round(v.x),
                y: Math.round(v.y),
                w: Math.max(minW, Math.round(v.w)),
                h: Math.max(minH, Math.round(v.h)),
                dataConfig: {
                  tableId: v.tableHint || mappedTables[0]?.name || '',
                  xAxisField: v.categoryField || undefined,
                  legendField: v.legendField || undefined,
                  yAxisFields: v.valueFields.length > 0 ? v.valueFields : undefined,
                  ...drillConfigFromPbiMeta(v, vType),
                  aggregation: 'SUM',
                  ...(v.sortField && vType !== 'slicer' ? { sortField: v.sortField } : {}),
                  ...(v.sortOrder ? { sortOrder: v.sortOrder } : {}),
                  ...(vType === 'slicer'
                    ? {
                        slicerConfig: {
                          mode: v.slicerMode === 'tiles' ? 'tiles' : 'dropdown',
                          selectedValues: [],
                        },
                      }
                    : {}),
                },
                formatConfig: {
                  showTitle: vType !== 'action-button' && vType !== 'textbox',
                  title: v.title,
                  showDataLabels: vType === 'pie' || vType === 'donut' || vType === 'funnel',
                  showLegend: true,
                  backgroundColor: '#ffffff',
                  borderColor: '#e2e8f0',
                  borderRadius: 8,
                  cardColumns: v.columnCount || undefined,
                  displayUnits: 'none',
                  colorBySign: true,
                  textContent: v.textContent || undefined,
                  actionType:
                    v.actionType === 'page-navigation' || v.actionType === 'back'
                      ? v.actionType
                      : undefined,
                  actionTargetPageId: v.actionTarget || undefined,
                  conditionalFormats: conditionalFormatsFromPbiMeta(v.conditionalFormats, v.id || `pbi_vis_${pIdx}_${vIdx}`),
                },
              };
            }),
          }))
        : [
            {
              id: 'page_1',
              name: 'Página 1',
              order: 0,
              visuals: [],
            },
          ];

    const importedFilters: Filter[] = summary.pages.flatMap((p, pIdx) =>
      (p.filters || []).map((f, fIdx): Filter => ({
        id: `pbi_flt_${pIdx}_${fIdx}`,
        level: 'page',
        tableId: f.table,
        column: f.column,
        operator: f.values.length > 1 ? 'in' : 'equals',
        value: f.values.length > 1 ? f.values : f.values[0],
      }))
    );

    loadStateFromProject({
      version: String(CURRENT_ENGINE_VERSION),
      projectName: summary.projectName || projectName,
      sourcePbipPath: manualPath.trim() || summary.rootPath || null,
      tables: mappedTables,
      relationships: mappedRelationships,
      measures: mappedMeasures,
      pages: mappedPages,
      activePageId: mappedPages[0]?.id || 'page_1',
      filters: importedFilters,
      bookmarks: [],
      dashboardTiles: [],
      goals: [],
      rlsRoles: [],
      whatIfParameters: [],
      calculationGroups: [],
      userDefinedFunctions: [],
      theme: 'powerbi-light',
      updatedAt: new Date().toISOString(),
    });

    addRecentProject({
      id: `${summary.sourceType}_${summary.rootPath || summary.projectName}`,
      name: summary.projectName || projectName || 'Projeto Power BI',
      type: summary.sourceType === 'pbix' ? 'pbix' : 'pbip',
      path: manualPath.trim() || summary.rootPath || undefined,
      openedAt: new Date().toISOString(),
    });
    setShowWelcomeScreen(false);
    setActiveView('report');
    onLoaded?.();
    onClose();
  };

  const totalMeasures = summary?.tables.reduce((acc, t) => acc + t.measures.length, 0) || 0;
  const totalVisuals = summary?.pages.reduce((acc, p) => acc + p.visuals.length, 0) || 0;

  return createPortal(
    <div className="fixed inset-0 bg-black/60 z-[10000] flex items-center justify-center p-4">
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl max-w-4xl w-full p-6 text-xs flex flex-col max-h-[88vh] overflow-hidden">
        <div className="flex items-center justify-between pb-3 border-b border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-amber-500 text-slate-950 font-bold">
              <FileCode2 className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-bold text-slate-900 dark:text-slate-100 text-sm">
                Abrir Projeto Power BI (.pbip / .pbix)
              </h3>
              <p className="text-[11px] text-slate-500 dark:text-slate-400">
                Parser nativo em Rust para TMDL, PBIR, Report/Layout, DataModelSchema e Power Query (M).
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-sm font-bold p-1 cursor-pointer"
          >
            ✕
          </button>
        </div>

        {/* File Picker Bar */}
        <div className="flex items-center gap-2 py-3 border-b border-slate-100 dark:border-slate-800">
          <button
            type="button"
            onClick={handlePickNative}
            disabled={loading}
            className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold flex items-center gap-1.5 shadow-xs cursor-pointer shrink-0"
          >
            <Upload className="w-3.5 h-3.5" />
            <span>{loading ? 'Lendo...' : 'Selecionar .pbip ou .pbix'}</span>
          </button>
          <input
            type="text"
            value={manualPath}
            onChange={(e) => setManualPath(e.target.value)}
            placeholder="/caminho/para/projeto.pbip ou relatorio.pbix"
            className="flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-xs font-mono text-slate-800 dark:text-slate-100"
          />
          <button
            type="button"
            onClick={() => manualPath.trim() && inspectPath(manualPath.trim())}
            disabled={loading || !manualPath.trim()}
            className="px-3 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-white font-semibold cursor-pointer"
          >
            Inspecionar
          </button>
        </div>

        {error && (
          <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-950/50 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-300 font-medium">
            {error}
          </div>
        )}

        {/* Parsed Summary Content */}
        <div className="flex-1 overflow-y-auto mt-3 space-y-4">
          {!summary ? (
            <div className="py-12 text-center text-slate-400 dark:text-slate-500 space-y-2">
              <Layers className="w-10 h-10 mx-auto text-slate-300 dark:text-slate-700" />
              <p className="font-semibold text-slate-600 dark:text-slate-300">
                Selecione um arquivo .pbip (recomendado) ou .pbix
              </p>
              <p className="text-[11px] max-w-md mx-auto">
                Projetos <strong>.pbip</strong> (TMDL + PBIR) preservam 100% das tabelas, colunas, relacionamentos, medidas DAX, scripts Power Query e layout de visuais.
              </p>
            </div>
          ) : (
            <>
              {/* Stats cards */}
              <div className="grid grid-cols-4 gap-2">
                <div className="p-2.5 rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700">
                  <div className="text-[10px] text-slate-500 uppercase font-semibold">Tabelas</div>
                  <div className="text-lg font-extrabold text-slate-900 dark:text-white flex items-center gap-1.5">
                    <TableIcon className="w-4 h-4 text-amber-500" />
                    {summary.tables.length}
                  </div>
                </div>
                <div className="p-2.5 rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700">
                  <div className="text-[10px] text-slate-500 uppercase font-semibold">Relacionamentos</div>
                  <div className="text-lg font-extrabold text-slate-900 dark:text-white flex items-center gap-1.5">
                    <GitBranch className="w-4 h-4 text-blue-500" />
                    {summary.relationships.length}
                  </div>
                </div>
                <div className="p-2.5 rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700">
                  <div className="text-[10px] text-slate-500 uppercase font-semibold">Medidas DAX</div>
                  <div className="text-lg font-extrabold text-slate-900 dark:text-white flex items-center gap-1.5">
                    <Sigma className="w-4 h-4 text-purple-500" />
                    {totalMeasures}
                  </div>
                </div>
                <div className="p-2.5 rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700">
                  <div className="text-[10px] text-slate-500 uppercase font-semibold">Páginas / Visuais</div>
                  <div className="text-lg font-extrabold text-slate-900 dark:text-white flex items-center gap-1.5">
                    <LayoutGrid className="w-4 h-4 text-emerald-500" />
                    {summary.pages.length} / {totalVisuals}
                  </div>
                </div>
              </div>

              {summary.warnings.length > 0 && (
                <div className="p-3 rounded-xl bg-amber-50 dark:bg-amber-950/40 border border-amber-200 dark:border-amber-800 text-amber-900 dark:text-amber-200 space-y-1">
                  {summary.warnings.map((w, idx) => (
                    <div key={idx} className="flex items-start gap-2 text-[11px]">
                      <AlertTriangle className="w-3.5 h-3.5 text-amber-600 shrink-0 mt-0.5" />
                      <span>{w}</span>
                    </div>
                  ))}
                </div>
              )}

              {/* Sub-tabs */}
              <div className="flex gap-1.5 border-b border-slate-200 dark:border-slate-800 pb-2">
                {[
                  { id: 'overview', label: `Tabelas & Relações (${summary.tables.length})` },
                  { id: 'measures', label: `Medidas DAX (${totalMeasures})` },
                  { id: 'pages', label: `Páginas & Visuais (${summary.pages.length})` },
                  { id: 'm', label: `Power Query M (${summary.powerQueryScripts.length})` },
                ].map(tab => (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveTab(tab.id as any)}
                    className={`px-3 py-1 rounded-lg font-semibold cursor-pointer ${
                      activeTab === tab.id
                        ? 'bg-slate-900 dark:bg-amber-500 text-white dark:text-slate-950'
                        : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300'
                    }`}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>

              {activeTab === 'overview' && (
                <div className="space-y-3">
                  {summary.tables.map(t => (
                    <div
                      key={t.name}
                      className="p-3 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-800/40"
                    >
                      <div className="font-bold text-slate-900 dark:text-slate-100 flex items-center justify-between">
                        <span>{t.name}</span>
                        <span className="text-[10px] text-slate-500">
                          {t.columns.length} colunas • {t.measures.length} medidas
                          {(() => {
                            const lt = summary.loadedTables?.find(x => x.tableName.toLowerCase() === t.name.toLowerCase());
                            return lt ? ` • ${lt.totalRows.toLocaleString('pt-BR')} linhas no modelo` : '';
                          })()}
                        </span>
                      </div>
                      <div className="flex flex-wrap gap-1.5 mt-2">
                        {t.columns.map(c => (
                          <span
                            key={c.name}
                            className="px-2 py-0.5 rounded bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 text-[10px] font-mono"
                          >
                            {c.name} <span className="text-slate-400">({c.dataType})</span>
                          </span>
                        ))}
                      </div>
                    </div>
                  ))}
                  {summary.relationships.length > 0 && (
                    <div className="p-3 rounded-xl border border-slate-200 dark:border-slate-800">
                      <div className="font-bold mb-2">Relacionamentos</div>
                      <div className="space-y-1 font-mono text-[11px]">
                        {summary.relationships.map((r, i) => (
                          <div key={i} className="text-slate-700 dark:text-slate-300">
                            {r.fromTable}[{r.fromColumn}] → {r.toTable}[{r.toColumn}] ({r.cardinality}, {r.crossFilterDirection})
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}

              {activeTab === 'measures' && (
                <div className="space-y-2">
                  {summary.tables.flatMap(t => t.measures).map((m, idx) => (
                    <div
                      key={idx}
                      className="p-3 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/40"
                    >
                      <div className="flex items-center justify-between font-bold text-slate-900 dark:text-slate-100">
                        <span>{m.name}</span>
                        <span className="text-[10px] text-slate-400">{m.tableName}</span>
                      </div>
                      <pre className="mt-1.5 p-2 rounded bg-slate-900 text-amber-300 font-mono text-[11px] overflow-x-auto">
                        {m.expression}
                      </pre>
                    </div>
                  ))}
                </div>
              )}

              {activeTab === 'pages' && (
                <div className="space-y-2">
                  {summary.pages.map(p => (
                    <div
                      key={p.id}
                      className="p-3 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/40"
                    >
                      <div className="font-bold text-slate-900 dark:text-slate-100 mb-1.5">
                        {p.name} ({Math.round(p.width)}x{Math.round(p.height)}) — {p.visuals.length} visuais
                      </div>
                      <div className="grid grid-cols-2 gap-1.5">
                        {p.visuals.map(v => (
                          <div
                            key={v.id}
                            className="p-2 rounded bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 text-[11px]"
                          >
                            <div className="font-semibold truncate">{v.title}</div>
                            <div className="text-[10px] text-slate-500">
                              {v.originalVisualType} → <strong className="text-amber-600">{v.visualType}</strong>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {activeTab === 'm' && (
                <div className="space-y-2">
                  {summary.powerQueryScripts.map((pq, idx) => (
                    <div key={idx} className="p-3 rounded-xl border border-slate-200 dark:border-slate-800">
                      <div className="font-bold mb-1">{pq.name}</div>
                      <pre className="p-2 rounded bg-slate-900 text-emerald-300 font-mono text-[11px] overflow-x-auto max-h-48">
                        {pq.script}
                      </pre>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="pt-3 mt-3 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between">
          <span className="text-[11px] text-slate-500">
            {summary ? `Projeto: ${summary.projectName} (${summary.sourceType.toUpperCase()})` : 'Aguardando seleção de arquivo'}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-semibold cursor-pointer"
            >
              Fechar
            </button>
            <button
              type="button"
              onClick={handleApplyToModel}
              disabled={!summary}
              className="px-4 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-700 disabled:opacity-40 text-white font-bold flex items-center gap-1.5 cursor-pointer"
            >
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>Carregar no PrismBI</span>
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
};
