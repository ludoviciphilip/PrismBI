import React, { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import {
  FileSpreadsheet,
  Database,
  RefreshCw,
  PlusCircle,
  HelpCircle,
  Share2,
  FileDown,
  Sparkles,
  Sliders,
  Calendar,
  Layers,
  ZoomIn,
  ZoomOut,
  Maximize,
  Smartphone,
  Monitor,
  Sigma,
  Zap,
  ShieldCheck,
  Search,
  Code,
  Info,
  FilePlus,
  Palette,
  FolderOpen,
  Grid,
  Globe,
  Coins,
  Key,
  Sun,
  Moon,
  FileCode2,
  Undo2,
  Redo2,
  Home,
  Download,
  Keyboard,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { exportProjectToFile } from '../../engine/storage';
import { useTranslation } from '../../i18n';
import { getEffectiveCanvasSize } from '../../engine/shared/canvasPresets';
import { subscribeShortcutAction, getShortcutHint } from '../../services/shortcuts/registry';
import { PowerBiImportModal } from '../modals/PowerBiImportModal';
import { etlToM } from '../../services/etl/etlClient';
import { conditionalFormatsToExportPayload } from '../../engine/conditionalFormat';
import {
  isTauriRuntime,
  pickFolderDialog,
  savePbipProject,
} from '../../bridge/tauriBridge';

export const Ribbon: React.FC<{
  showFiltersPane: boolean;
  setShowFiltersPane: (v: boolean) => void;
  showVisualsPane: boolean;
  setShowVisualsPane: (v: boolean) => void;
  showFieldsPane: boolean;
  setShowFieldsPane: (v: boolean) => void;
  showBookmarksPane: boolean;
  setShowBookmarksPane: (v: boolean) => void;
  showSelectionPane: boolean;
  setShowSelectionPane: (v: boolean) => void;
}> = ({
  showFiltersPane,
  setShowFiltersPane,
  showVisualsPane,
  setShowVisualsPane,
  showFieldsPane,
  setShowFieldsPane,
  showBookmarksPane,
  setShowBookmarksPane,
  showSelectionPane,
  setShowSelectionPane,
}) => {
  const [activeRibbonTab, setActiveRibbonTab] = useState<'inicio' | 'inserir' | 'modelagem' | 'exibir' | 'ajuda'>('inicio');
  const [showNewProjectModal, setShowNewProjectModal] = useState(false);
  const [showPbiImportModal, setShowPbiImportModal] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // v0.11 (atalhos de teclado): Ctrl+O e Ctrl+N vivem no hook global, mas
  // "Abrir" e "Novo Projeto" dependem de estado local daqui (o <input
  // type="file"> e o modal de confirmação) — a ponte é esse pub/sub, sem
  // duplicar UI nem subir esse estado para a store só por causa do atalho.
  useEffect(() => {
    return subscribeShortcutAction(action => {
      if (action === 'open-project-file') {
        fileInputRef.current?.click();
      } else if (action === 'new-project-modal') {
        setShowNewProjectModal(true);
      }
    });
  }, []);

  const { t, language, currency, setLanguage, setCurrency } = useTranslation();

  const {
    setActiveView,
    addVisual,
    canvasZoom,
    setCanvasZoom,
    layoutMode,
    setLayoutMode,
    isPresentationMode,
    setIsPresentationMode,
    setDataSourceModalOpen,
    setQuickMeasureModalOpen,
    setCustomMeasureModalOpen,
    setLimitationsModalOpen,
    setAiModalOpen,
    setAiSettingsModalOpen,
    setCanvasSizeModalOpen,
    setShortcutsModalOpen,
    pages,
    activePageId,
    newProject,
    refreshAllData,
    isRefreshingData,
    refreshStatus,
    setRefreshStatus,
    generateCalendar,
    saveProject,
    getProjectState,
    theme,
    setTheme,
    activePalette,
    undo,
    redo,
    canUndo,
    canRedo,
    setThemeModalOpen,
    themeMode,
    setThemeMode,
    isDarkMode,
    gridSize,
    setGridSize,
    showGrid,
    setShowGrid,
    snapToGrid,
    setSnapToGrid,
    loadStateFromProject,
    lastSavedAt,
    projectName,
    setProjectName,
    setShowWelcomeScreen,
    addRecentProject,
    setUpdateModalOpen,
    availableUpdate,
    tables,
  } = useBiStore(useShallow(s => ({
    setActiveView: s.setActiveView,
    addVisual: s.addVisual,
    canvasZoom: s.canvasZoom,
    setCanvasZoom: s.setCanvasZoom,
    layoutMode: s.layoutMode,
    setLayoutMode: s.setLayoutMode,
    isPresentationMode: s.isPresentationMode,
    setIsPresentationMode: s.setIsPresentationMode,
    setDataSourceModalOpen: s.setDataSourceModalOpen,
    setQuickMeasureModalOpen: s.setQuickMeasureModalOpen,
    setCustomMeasureModalOpen: s.setCustomMeasureModalOpen,
    setLimitationsModalOpen: s.setLimitationsModalOpen,
    setAiModalOpen: s.setAiModalOpen,
    setAiSettingsModalOpen: s.setAiSettingsModalOpen,
    setCanvasSizeModalOpen: s.setCanvasSizeModalOpen,
    setShortcutsModalOpen: s.setShortcutsModalOpen,
    pages: s.pages,
    activePageId: s.activePageId,
    newProject: s.newProject,
    refreshAllData: s.refreshAllData,
    isRefreshingData: s.isRefreshingData,
    refreshStatus: s.refreshStatus,
    setRefreshStatus: s.setRefreshStatus,
    generateCalendar: s.generateCalendar,
    saveProject: s.saveProject,
    getProjectState: s.getProjectState,
    theme: s.theme,
    setTheme: s.setTheme,
    activePalette: s.activePalette,
    undo: s.undo,
    redo: s.redo,
    canUndo: s.canUndo,
    canRedo: s.canRedo,
    setThemeModalOpen: s.setThemeModalOpen,
    themeMode: s.themeMode,
    setThemeMode: s.setThemeMode,
    isDarkMode: s.isDarkMode,
    gridSize: s.gridSize,
    setGridSize: s.setGridSize,
    showGrid: s.showGrid,
    setShowGrid: s.setShowGrid,
    snapToGrid: s.snapToGrid,
    setSnapToGrid: s.setSnapToGrid,
    loadStateFromProject: s.loadStateFromProject,
    lastSavedAt: s.lastSavedAt,
    projectName: s.projectName,
    setProjectName: s.setProjectName,
    setShowWelcomeScreen: s.setShowWelcomeScreen,
    addRecentProject: s.addRecentProject,
    setUpdateModalOpen: s.setUpdateModalOpen,
    availableUpdate: s.availableUpdate,
    tables: s.tables,
  })));

  const handleAddVisualToReport = (type: Parameters<typeof addVisual>[0]) => {
    addVisual(type);
    setActiveView('report');
  };

  const handleAutoCalendar = () => {
    let minYear = new Date().getFullYear() - 1;
    let maxYear = new Date().getFullYear() + 1;
    let foundMin = Infinity;
    let foundMax = -Infinity;
    for (const tbl of tables) {
      const dateCols = tbl.columns.filter(c => c.dataType === 'date').map(c => c.name);
      if (dateCols.length === 0) continue;
      for (const row of tbl.rows) {
        for (const col of dateCols) {
          const val = String(row[col] ?? '');
          const m = val.match(/^(\d{4})-\d{2}-\d{2}/);
          if (m) {
            const y = Number(m[1]);
            if (y >= 1990 && y <= 2100) {
              if (y < foundMin) foundMin = y;
              if (y > foundMax) foundMax = y;
            }
          }
        }
      }
    }
    if (Number.isFinite(foundMin) && Number.isFinite(foundMax)) {
      minYear = foundMin;
      maxYear = Math.max(foundMin, foundMax);
    }
    const startDate = `${minYear}-01-01`;
    const endDate = `${maxYear}-12-31`;
    generateCalendar(startDate, endDate);
    setRefreshStatus({
      type: 'success',
      message: `Tabela d_Calendario (${startDate} a ${endDate}) atualizada com sucesso.`,
      timestamp: new Date().toLocaleTimeString('pt-BR'),
    });
  };

  const activeRibbonPage = pages.find(p => p.id === activePageId) || pages[0];
  const { width: ribbonCanvasWidth, height: ribbonCanvasHeight } = getEffectiveCanvasSize(activeRibbonPage);

  useEffect(() => {
    if (!refreshStatus) return;
    const timer = setTimeout(() => setRefreshStatus(null), 7000);
    return () => clearTimeout(timer);
  }, [refreshStatus, setRefreshStatus]);

  const handleExportProject = () => {
    const state = getProjectState();
    exportProjectToFile(state, `${projectName.replace(/\s+/g, '_')}.insight`);
  };

  const handleExportPbip = async () => {
    if (!isTauriRuntime()) {
      alert('Exportação para pasta .pbip requer o app desktop VetroBI (Tauri).');
      return;
    }
    const folder = await pickFolderDialog('Selecione a pasta para salvar o projeto .pbip');
    if (!folder) return;

    const state = getProjectState();
    // Round-trip M: tabelas com consulta ETL exportam o M gerado das etapas.
    // Snapshot inline sem etapas mantém o M original (não vira um #table gigante).
    const mByTable = new Map<string, string>();
    for (const t of state.tables) {
      const q = state.appliedSteps?.[t.id];
      if (!q || (q.source.kind === 'inline' && q.steps.length === 0)) continue;
      try {
        mByTable.set(t.id, await etlToM({ query: q }));
      } catch (e) {
        console.warn(`[pbip] ${t.name}: M não gerado, mantendo o original`, e);
      }
    }
    const resolveTableName = (tableId: string) =>
      state.tables.find(t => t.id === tableId || t.name === tableId)?.name || tableId;
    // "tableId.Coluna" -> "NomeDaTabela.Coluna" (PBIR entities are table names).
    const qualifyExportField = (field: string) => {
      const dot = field.indexOf('.');
      if (dot <= 0) return field;
      const prefix = field.slice(0, dot);
      const known = state.tables.some(t => t.id === prefix || t.name === prefix);
      return known ? `${resolveTableName(prefix)}.${field.slice(dot + 1)}` : field;
    };
    const toExportFilters = (list: typeof state.filters) =>
      list.map(f => ({
        table: resolveTableName(f.tableId),
        column: f.column,
        values: Array.isArray(f.value) ? f.value.map(String) : [String(f.value ?? '')],
      }));

    try {
      const savedPath = await savePbipProject(folder, {
        projectName: state.projectName || projectName || 'VetroBI',
        tables: state.tables.map(t => ({
          name: t.name,
          isHidden: t.isHidden,
          mScript: mByTable.get(t.id) ?? t.sourceConfig?.mScript ?? null,
          columns: t.columns.map(c => ({
            name: c.name,
            dataType: c.dataType,
            formatString: c.format,
            sortByColumn: c.sortByColumn,
            isHidden: c.isHidden,
            isCalculated: c.isCalculated,
            expression: c.expression,
          })),
        })),
        relationships: state.relationships.map(r => ({
          fromTable: r.fromTable,
          fromColumn: r.fromColumn,
          toTable: r.toTable,
          toColumn: r.toColumn,
          crossFilterDirection: r.crossFilterDirection,
          isActive: r.isActive,
        })),
        measures: state.measures.map(m => ({
          name: m.name,
          tableName: m.tableName || state.tables[0]?.name || 'Medidas',
          formula: m.formula,
          formatString: m.formatString,
          displayFolder: m.displayFolder,
          description: m.description,
        })),
        pages: state.pages.map(p => ({
          id: p.id,
          name: p.name,
          width: p.canvasSize?.width || 1280,
          height: p.canvasSize?.height || 720,
          filters: toExportFilters(state.filters.filter(f => f.level === 'page' && f.visualId === p.id)),
          visuals: p.visuals.map(v => ({
            id: v.id,
            visualType: v.type,
            title: v.title,
            x: v.x,
            y: v.y,
            w: v.w,
            h: v.h,
            tableId: resolveTableName(v.dataConfig.tableId),
            xAxisField: v.dataConfig.xAxisField || null,
            legendField: v.dataConfig.legendField || null,
            yAxisFields: v.dataConfig.yAxisFields || [],
            cardColumns: v.formatConfig.cardColumns ?? null,
            slicerMode: v.dataConfig.slicerConfig?.mode ?? null,
            textContent: v.formatConfig.textContent ?? null,
            actionType: v.formatConfig.actionType ?? null,
            actionTarget: v.formatConfig.actionTargetPageId ?? null,
            showTitle: v.formatConfig.showTitle,
            backgroundColor: v.formatConfig.backgroundColor ?? null,
            borderColor: v.formatConfig.borderColor ?? null,
            filters: toExportFilters(state.filters.filter(f => f.level === 'visual' && f.visualId === v.id)),
            // Mapped to PBIR query.sortDefinition by the Rust writer.
            sortField: v.dataConfig.sortField ? qualifyExportField(v.dataConfig.sortField) : null,
            sortOrder: v.dataConfig.sortOrder ?? null,
            // v0.10 Frente H: formatação condicional + segmentador novo, round-trip .pbip.
            condFormats: conditionalFormatsToExportPayload(v.formatConfig.conditionalFormats),
            slicerBetweenStart: v.dataConfig.slicerConfig?.between?.start != null ? String(v.dataConfig.slicerConfig.between.start) : null,
            slicerBetweenEnd: v.dataConfig.slicerConfig?.between?.end != null ? String(v.dataConfig.slicerConfig.between.end) : null,
            slicerRelativeAnchor: v.dataConfig.slicerConfig?.relative?.anchor ?? null,
            slicerRelativeCount: v.dataConfig.slicerConfig?.relative?.count ?? null,
            slicerRelativeUnit: v.dataConfig.slicerConfig?.relative?.unit ?? null,
            slicerRelativeIncludeToday: v.dataConfig.slicerConfig?.relative?.includeToday ?? null,
            slicerShowSearch: v.dataConfig.slicerConfig?.showSearch ?? null,
            // v0.10 Rodada 2 (seams H↔G): hierarquia completa no round-trip .pbip.
            drillFields: v.dataConfig.drillFields ?? [],
            matrixRowFields: v.dataConfig.matrixRowFields ?? [],
            // v0.11 Rodada 2 (Frente I): N níveis nas COLUNAS no round-trip .pbip.
            matrixColumnFields: v.dataConfig.matrixColumnFields ?? [],
          })),
        })),
        themeName: state.activeThemeId || theme || 'VetroTheme',
        themePalette: activePalette,
        // v0.10 Frente H: indicadores (bookmarks) no formato PBIR, round-trip .pbip.
        bookmarks: (state.bookmarks || []).map((bm: any) => ({
          id: bm.id,
          name: bm.name,
          activePageId: bm.activePageId,
          filters: toExportFilters(bm.filters || []),
          order: bm.order ?? null,
          groupId: bm.groupId ?? null,
          captureData: bm.capture?.data ?? null,
          captureDisplay: bm.capture?.display ?? null,
          captureCurrentPage: bm.capture?.currentPage ?? null,
          scope: Array.isArray(bm.scope) ? bm.scope : [],
          visualState: bm.visualState ?? null,
          slicerSelections: bm.slicerSelections ?? null,
        })),
      });
      alert(`Projeto .pbip exportado com sucesso em:\n${savedPath}`);
    } catch (err: any) {
      alert(`Erro ao exportar .pbip: ${err?.message || err}`);
    }
  };

  const handleImportInsight = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const content = event.target?.result as string;
        const parsed = JSON.parse(content);
        if (parsed && (parsed.tables || parsed.pages)) {
          loadStateFromProject(parsed);
          addRecentProject({
            id: `insight_${file.name}`,
            name: parsed.projectName || file.name.replace(/\.(insight|json)$/i, ''),
            type: 'insight',
            openedAt: new Date().toISOString(),
          });
        } else {
          alert('O arquivo selecionado não contém uma estrutura de projeto .insight válida.');
        }
      } catch (err) {
        console.error('Erro ao ler arquivo .insight:', err);
        alert('Erro ao carregar o arquivo .insight. Certifique-se de que é um JSON válido.');
      }
    };
    reader.readAsText(file);
    if (e.target) e.target.value = '';
  };

  return (
    <div className="w-full bg-[#f8fafc] dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 select-none shadow-xs">
      {/* Hidden file input for .insight project import */}
      <input
        type="file"
        ref={fileInputRef}
        accept=".insight,.json"
        onChange={handleImportInsight}
        className="hidden"
      />

      {/* Top Application Bar: Title, Project Name, Status */}
      <div className="flex items-center justify-between px-3 py-1 bg-slate-900 text-white text-xs">
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1.5 font-bold tracking-wide text-amber-400">
            <img src="/icon.png" alt="VetroBI" className="w-4 h-4 rounded-xs" />
            <span>VetroBI</span>
            {isTauriRuntime() && (
              <span className="ml-1 px-1.5 py-0.2 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 text-[9px] font-semibold">
                DuckDB Nativo
              </span>
            )}
          </div>
          <span className="text-slate-500">|</span>
          <input
            type="text"
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
            className="bg-transparent text-slate-200 font-medium hover:bg-slate-800 px-1.5 py-0.5 rounded focus:bg-slate-800 focus:outline-none focus:ring-1 focus:ring-amber-400 max-w-[200px] truncate"
          />
          <button
            onClick={() => setShowWelcomeScreen(true)}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white font-medium transition-colors text-[10px] flex items-center gap-1 cursor-pointer"
            title="Voltar para a Tela Inicial"
          >
            <Home className="w-3 h-3 text-amber-400" />
            <span>Início</span>
          </button>
          <button
            onClick={() => setShowNewProjectModal(true)}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white font-medium transition-colors text-[10px]"
            title={`${t.topBar.newProjectTooltip} (${getShortcutHint('file.new')})`}
          >
            {t.topBar.newProject}
          </button>
          <button
            onClick={() => fileInputRef.current?.click()}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white font-medium transition-colors text-[10px] flex items-center gap-1"
            title={`${t.topBar.openInsightTooltip} (${getShortcutHint('file.open')})`}
          >
            <FolderOpen className="w-3 h-3 text-amber-400" />
            <span>{t.topBar.openInsight}</span>
          </button>
          {availableUpdate?.updateAvailable && (
            <button
              onClick={() => setUpdateModalOpen(true)}
              className="px-2 py-0.5 rounded bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-bold transition-colors text-[10px] flex items-center gap-1 cursor-pointer"
              title="Nova versão disponível para download e atualização"
            >
              <Download className="w-3 h-3" />
              <span>Atualização v{availableUpdate.latestVersion}</span>
            </button>
          )}
        </div>

        <div className="flex items-center gap-2.5 text-[11px] text-slate-300">
          {/* Language Selector */}
          <div className="flex items-center gap-1 bg-slate-800 rounded px-1.5 py-0.5 border border-slate-700/70" title={t.common.language}>
            <Globe className="w-3 h-3 text-amber-400 shrink-0" />
            <select
              value={language}
              onChange={(e) => setLanguage(e.target.value as any)}
              style={{ colorScheme: 'dark' }}
              className="!bg-slate-800 !text-slate-100 text-[10px] font-medium focus:outline-none cursor-pointer pr-5 rounded"
              aria-label={t.common.language}
            >
              <option value="pt-BR" className="bg-slate-900 text-white">🇧🇷 PT</option>
              <option value="en-US" className="bg-slate-900 text-white">🇺🇸 EN</option>
              <option value="es-ES" className="bg-slate-900 text-white">🇪🇸 ES</option>
            </select>
          </div>

          {/* Currency Selector */}
          <div className="flex items-center gap-1 bg-slate-800 rounded px-1.5 py-0.5 border border-slate-700/70" title={t.common.currency}>
            <Coins className="w-3 h-3 text-amber-400 shrink-0" />
            <select
              value={currency}
              onChange={(e) => setCurrency(e.target.value as any)}
              style={{ colorScheme: 'dark' }}
              className="!bg-slate-800 !text-slate-100 text-[10px] font-medium focus:outline-none cursor-pointer pr-5 rounded"
              aria-label={t.common.currency}
            >
              <option value="BRL" className="bg-slate-900 text-white">R$ BRL</option>
              <option value="USD" className="bg-slate-900 text-white">$ USD</option>
              <option value="EUR" className="bg-slate-900 text-white">€ EUR</option>
            </select>
          </div>

          {/* Theme Mode Selector (Light / Dark / System) */}
          <div className="flex items-center gap-1 bg-slate-800 rounded px-1.5 py-0.5 border border-slate-700/70" title="Tema da Interface (Claro, Escuro ou Automático)">
            {themeMode === 'dark' ? (
              <Moon className="w-3 h-3 text-amber-400 shrink-0" />
            ) : themeMode === 'light' ? (
              <Sun className="w-3 h-3 text-amber-400 shrink-0" />
            ) : (
              <Monitor className="w-3 h-3 text-amber-400 shrink-0" />
            )}
            <select
              value={themeMode}
              onChange={(e) => setThemeMode(e.target.value as any)}
              style={{ colorScheme: 'dark' }}
              className="!bg-slate-800 !text-slate-100 text-[10px] font-medium focus:outline-none cursor-pointer pr-5 rounded"
              aria-label="Tema da Interface"
            >
              <option value="light" className="bg-slate-900 text-white">☀️ Claro</option>
              <option value="dark" className="bg-slate-900 text-white">🌙 Escuro</option>
              <option value="system" className="bg-slate-900 text-white">💻 Sistema</option>
            </select>
          </div>

          <span className="text-slate-700">|</span>

          {lastSavedAt && (
            <span className={lastSavedAt === t.common.saveFailed ? 'text-red-400 font-bold text-[10px]' : 'text-slate-400 text-[10px]'}>
              {lastSavedAt.startsWith(t.common.savedAt) || lastSavedAt === t.common.saveFailed ? lastSavedAt : `${t.common.savedAt} ${lastSavedAt}`}
            </span>
          )}
          <button
            onClick={() => handleExportPbip()}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-emerald-300 hover:text-emerald-200 font-medium transition-colors flex items-center gap-1 text-[10px]"
            title="Exportar modelo e páginas no formato Power BI Project (.pbip)"
          >
            <FileCode2 className="w-3 h-3 text-emerald-400" />
            <span>Exportar .pbip</span>
          </button>
          <button
            onClick={() => handleExportProject()}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white font-medium transition-colors flex items-center gap-1 text-[10px]"
            title={`${t.topBar.exportInsightTooltip} (${getShortcutHint('file.saveAs')})`}
          >
            <FileDown className="w-3 h-3 text-amber-400" />
            <span>{t.topBar.exportInsight}</span>
          </button>
          <button
            onClick={() => saveProject()}
            className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 font-medium transition-colors"
            title={`${t.common.save} (${getShortcutHint('file.save')})`}
          >
            {t.common.save}
          </button>
          <button
            onClick={() => setAiSettingsModalOpen(true)}
            className="flex items-center gap-1 bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white px-2 py-0.5 rounded font-medium transition-colors text-[10px]"
            title="Configurar Chave Gemini (BYOK)"
          >
            <Key className="w-3 h-3 text-amber-400" />
            <span>Configurar IA</span>
          </button>
          <button
            onClick={() => setAiModalOpen(true)}
            className="flex items-center gap-1 bg-linear-to-r from-amber-500 to-amber-600 hover:from-amber-600 hover:to-amber-700 text-slate-950 px-2 py-0.5 rounded font-bold transition-all shadow-xs"
          >
            <Sparkles className="w-3.5 h-3.5" />
            <span>{t.topBar.copilotAi}</span>
          </button>
        </div>
      </div>

      {/* Ribbon Navigation Tabs */}
      <div className="flex items-center px-2 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-xs">
        {[
          { id: 'inicio', label: t.ribbonTabs.home },
          { id: 'inserir', label: t.ribbonTabs.insert },
          { id: 'modelagem', label: t.ribbonTabs.modeling },
          { id: 'exibir', label: t.ribbonTabs.view },
          { id: 'ajuda', label: t.ribbonTabs.help },
        ].map(tab => (
          <button
            key={tab.id}
            onClick={() => setActiveRibbonTab(tab.id as any)}
            className={`px-3 py-1 font-medium border-b-2 transition-all ${
              activeRibbonTab === tab.id
                ? 'border-amber-500 text-slate-900 dark:text-amber-400 bg-white dark:bg-slate-800 font-semibold shadow-xs'
                : 'border-transparent text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-slate-200/50 dark:hover:bg-slate-800/60'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Ribbon Action Bar */}
      <div className="flex items-center gap-1 px-3 py-1.5 overflow-x-auto min-h-[64px] bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-800 dark:text-slate-200">
        {/* TAB 1: INÍCIO */}
        {activeRibbonTab === 'inicio' && (
          <>
            {/* Novo Projeto */}
            <div className="flex flex-col items-center px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setShowNewProjectModal(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 group transition-all cursor-pointer"
                title={t.topBar.newProjectTooltip}
              >
                <div className="p-1 rounded bg-slate-100 dark:bg-slate-800 group-hover:bg-slate-200 dark:group-hover:bg-slate-700 text-slate-800 dark:text-slate-100">
                  <FilePlus className="w-5 h-5 text-amber-500" />
                </div>
                <span className="text-[10px] font-semibold mt-0.5">{t.topBar.newProject}</span>
              </button>
            </div>

            {/* Obter Dados & Abrir Power BI */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setDataSourceModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 group transition-all cursor-pointer"
              >
                <div className="p-1 rounded bg-amber-50 dark:bg-amber-950/50 group-hover:bg-amber-100 dark:group-hover:bg-amber-900/60 text-amber-600 dark:text-amber-400">
                  <Database className="w-5 h-5" />
                </div>
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.getData}</span>
              </button>
              <button
                onClick={() => {
                  useBiStore.getState().setSelectedVisualId(null);
                  setShowPbiImportModal(true);
                }}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 group transition-all cursor-pointer"
                title="Abrir projeto Power BI (.pbip / .pbix)"
              >
                <div className="p-1 rounded bg-amber-50 dark:bg-amber-950/50 group-hover:bg-amber-100 dark:group-hover:bg-amber-900/60 text-amber-600 dark:text-amber-400">
                  <FileCode2 className="w-5 h-5" />
                </div>
                <span className="text-[10px] font-semibold mt-0.5">Abrir .pbip/.pbix</span>
              </button>
            </div>

            {/* Transformar Dados */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setActiveView('query')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 group transition-all cursor-pointer"
              >
                <div className="p-1 rounded bg-blue-50 dark:bg-blue-950/50 group-hover:bg-blue-100 dark:group-hover:bg-blue-900/60 text-blue-600 dark:text-blue-400">
                  <Sliders className="w-5 h-5" />
                </div>
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.transformData}</span>
              </button>
              <button
                onClick={() => setActiveView('daxquery')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 group transition-all cursor-pointer"
                title="Abrir editor de consultas DAX (EVALUATE, DEFINE)"
              >
                <div className="p-1 rounded bg-amber-50 dark:bg-amber-950/50 group-hover:bg-amber-100 dark:group-hover:bg-amber-900/60 text-amber-600 dark:text-amber-400">
                  <Code className="w-5 h-5" />
                </div>
                <span className="text-[10px] font-semibold mt-0.5">DAX Query</span>
              </button>
            </div>

            {/* Novo Visual e Medidas */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => handleAddVisualToReport('column')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <PlusCircle className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.newVisual}</span>
              </button>
              <button
                onClick={() => setCustomMeasureModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
                title={t.panes.fields.newMeasure}
              >
                <Sigma className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.newMeasure}</span>
              </button>
              <button
                onClick={() => setQuickMeasureModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Zap className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.quickMeasure}</span>
              </button>
            </div>

            {/* Atualizar Dados das Fontes + Histórico do Canvas */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => undo()}
                disabled={!canUndo}
                title="Desfazer última ação no canvas (Ctrl+Z)"
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-40 disabled:pointer-events-none text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Undo2 className="w-5 h-5 text-slate-600 dark:text-slate-300" />
                <span className="text-[10px] font-semibold mt-0.5">Desfazer</span>
              </button>
              <button
                onClick={() => redo()}
                disabled={!canRedo}
                title="Refazer ação no canvas (Ctrl+Y / Ctrl+Shift+Z)"
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-40 disabled:pointer-events-none text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Redo2 className="w-5 h-5 text-slate-600 dark:text-slate-300" />
                <span className="text-[10px] font-semibold mt-0.5">Refazer</span>
              </button>
              <button
                onClick={() => void refreshAllData()}
                disabled={isRefreshingData}
                title={`Atualizar dados das fontes vinculadas (.xlsx, .csv, .parquet, .json, .pbip, REST) mantendo visuais, medidas e relacionamentos (${getShortcutHint('data.refresh')})`}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-60 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <RefreshCw
                  className={`w-5 h-5 ${
                    isRefreshingData
                      ? 'text-amber-500 animate-spin'
                      : 'text-slate-600 dark:text-slate-300'
                  }`}
                />
                <span className="text-[10px] font-semibold mt-0.5">
                  {isRefreshingData ? 'Atualizando...' : t.ribbonActions.refresh}
                </span>
              </button>
            </div>

            {/* Metabase & AI Mode */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setActiveView('metabase')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Search className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.nav.metabase}</span>
              </button>
              <button
                onClick={() => setAiModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-amber-50 dark:hover:bg-amber-950/50 text-amber-800 dark:text-amber-300 cursor-pointer"
              >
                <Sparkles className="w-5 h-5 text-amber-500" />
                <span className="text-[10px] font-semibold mt-0.5">{t.topBar.copilotAi}</span>
              </button>
            </div>

            {/* Temas do Relatório */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setThemeModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Palette className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.themeManager}</span>
              </button>
            </div>

            {/* Exportar e Compartilhar */}
            <div className="flex items-center gap-1 px-2">
              <button
                onClick={handleExportProject}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
                title={t.topBar.exportInsightTooltip}
              >
                <FileDown className="w-5 h-5 text-slate-600 dark:text-slate-300" />
                <span className="text-[10px] font-semibold mt-0.5">{t.topBar.exportInsight}</span>
              </button>
              <button
                onClick={() => window.print()}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
                title={`PDF / Print (${getShortcutHint('file.exportPdf')})`}
              >
                <Share2 className="w-5 h-5 text-slate-600 dark:text-slate-300" />
                <span className="text-[10px] font-semibold mt-0.5">PDF</span>
              </button>
            </div>
          </>
        )}

        {/* TAB 2: INSERIR */}
        {activeRibbonTab === 'inserir' && (
          <>
            <div className="flex items-center gap-1.5 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => handleAddVisualToReport('column')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-amber-600">📊</span>
                <span className="text-[10px] font-medium">Colunas</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('line')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-blue-600">📈</span>
                <span className="text-[10px] font-medium">Linhas</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('donut')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-emerald-600">🍩</span>
                <span className="text-[10px] font-medium">Rosca</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('waterfall')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-purple-600">📉</span>
                <span className="text-[10px] font-medium">Cascata</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('funnel')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-pink-600">🔻</span>
                <span className="text-[10px] font-medium">Funil</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('card')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-amber-700">🔲</span>
                <span className="text-[10px] font-medium">Cartão KPI</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('multi-card')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-teal-600">🗂️</span>
                <span className="text-[10px] font-medium">Cartão Múltiplo</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('matrix')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-cyan-600">⊞</span>
                <span className="text-[10px] font-medium">Matriz</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('table')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-slate-700 dark:text-slate-300">📋</span>
                <span className="text-[10px] font-medium">Tabela</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('slicer')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-blue-700">🎛️</span>
                <span className="text-[10px] font-medium">Segmentação</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('textbox')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-indigo-500">📝</span>
                <span className="text-[10px] font-medium">Texto</span>
              </button>
              <button
                onClick={() => handleAddVisualToReport('action-button')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <span className="text-base font-bold text-amber-500">🔘</span>
                <span className="text-[10px] font-medium">Botão</span>
              </button>
            </div>
          </>
        )}

        {/* TAB 3: MODELAGEM */}
        {activeRibbonTab === 'modelagem' && (
          <>
            <div className="flex items-center gap-2 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setCustomMeasureModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Sigma className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.newMeasure}</span>
              </button>
              <button
                onClick={() => setQuickMeasureModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Zap className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.quickMeasure}</span>
              </button>
              <button
                onClick={handleAutoCalendar}
                title="Gerar ou atualizar d_Calendario cobrindo automaticamente o período das tabelas carregadas"
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Calendar className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                <span className="text-[10px] font-semibold mt-0.5">Calendário</span>
              </button>
              <button
                onClick={() => setActiveView('model')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Layers className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.nav.model}</span>
              </button>
              <button
                onClick={() => setActiveView('governance')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <ShieldCheck className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.nav.governance}</span>
              </button>
            </div>
          </>
        )}

        {/* TAB 4: EXIBIR */}
        {activeRibbonTab === 'exibir' && (
          <>
            {/* Painéis */}
            <div className="flex items-center gap-1.5 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setShowFiltersPane(!showFiltersPane)}
                className={`flex flex-col items-center p-1.5 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  showFiltersPane ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                <span className="text-[10px]">{t.panes.filters.title}</span>
              </button>
              <button
                onClick={() => setShowVisualsPane(!showVisualsPane)}
                className={`flex flex-col items-center p-1.5 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  showVisualsPane ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                <span className="text-[10px]">{t.panes.visualizations.title}</span>
              </button>
              <button
                onClick={() => setShowFieldsPane(!showFieldsPane)}
                className={`flex flex-col items-center p-1.5 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  showFieldsPane ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                <span className="text-[10px]">{t.panes.fields.title}</span>
              </button>
              <button
                onClick={() => setShowBookmarksPane(!showBookmarksPane)}
                data-testid="ribbon-toggle-bookmarks"
                className={`flex flex-col items-center p-1.5 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  showBookmarksPane ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
                title="Indicadores"
              >
                <span className="text-[10px]">Indicadores</span>
              </button>
              <button
                onClick={() => setShowSelectionPane(!showSelectionPane)}
                data-testid="ribbon-toggle-selection"
                className={`flex flex-col items-center p-1.5 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  showSelectionPane ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
                title="Seleção"
              >
                <span className="text-[10px]">Seleção</span>
              </button>
            </div>

            {/* Layout Celular / Desktop */}
            <div className="flex items-center gap-1 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setLayoutMode(layoutMode === 'desktop' ? 'mobile' : 'desktop')}
                className={`flex flex-col items-center p-1 rounded text-slate-700 dark:text-slate-200 cursor-pointer ${
                  layoutMode === 'mobile' ? 'bg-amber-100 dark:bg-amber-950/60 dark:text-amber-300 font-bold' : 'hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                {layoutMode === 'desktop' ? <Smartphone className="w-5 h-5 text-slate-600 dark:text-slate-300" /> : <Monitor className="w-5 h-5 text-amber-600 dark:text-amber-400" />}
                <span className="text-[10px] font-semibold mt-0.5">
                  {layoutMode === 'desktop' ? t.ribbonActions.mobileLayout : t.ribbonActions.desktopLayout}
                </span>
              </button>
              <button
                onClick={() => {
                  setActiveView('report');
                  setIsPresentationMode(!isPresentationMode);
                }}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
                title="Tela cheia de apresentação (pressione Esc para sair)"
              >
                <Maximize className="w-5 h-5 text-slate-600 dark:text-slate-300" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.presentationMode}</span>
              </button>
            </div>

            {/* Temas da Paleta & Modo da Interface */}
            <div className="flex items-center gap-1.5 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setThemeModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                title={t.ribbonActions.themeManager}
              >
                <Palette className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.ribbonActions.themeManager}</span>
              </button>

              <div className="flex flex-col items-center justify-center pl-1 border-l border-slate-200/80 dark:border-slate-800">
                <span className="text-[9px] font-bold text-slate-400 dark:text-slate-400 uppercase tracking-wider mb-1">
                  Modo de Cor
                </span>
                <div className="flex items-center bg-slate-100 dark:bg-slate-800 p-0.5 rounded-lg border border-slate-300 dark:border-slate-700">
                  <button
                    onClick={() => setThemeMode('light')}
                    className={`flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-semibold transition-all cursor-pointer ${
                      themeMode === 'light'
                        ? 'bg-white text-slate-900 shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                    }`}
                    title="Modo Claro"
                  >
                    <Sun className="w-3 h-3 text-amber-500" />
                    <span>Claro</span>
                  </button>
                  <button
                    onClick={() => setThemeMode('dark')}
                    className={`flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-semibold transition-all cursor-pointer ${
                      themeMode === 'dark'
                        ? 'bg-slate-900 text-white shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                    }`}
                    title="Modo Escuro"
                  >
                    <Moon className="w-3 h-3 text-amber-400" />
                    <span>Escuro</span>
                  </button>
                  <button
                    onClick={() => setThemeMode('system')}
                    className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-semibold transition-all cursor-pointer ${
                      themeMode === 'system'
                        ? 'bg-amber-400 text-slate-950 font-bold shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                    }`}
                    title="Automático (Seguir sistema)"
                  >
                    <Monitor className="w-3 h-3" />
                    <span>Auto</span>
                  </button>
                </div>
              </div>
            </div>

            {/* Tamanho da Grade / Tela */}
            <div className="flex items-center gap-2 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setCanvasSizeModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 transition-colors cursor-pointer"
                title="Ajustar tamanho e proporção da tela/grade do relatório"
              >
                <Grid className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                <span className="text-[10px] font-semibold mt-0.5">
                  {t.ribbonActions.pageSize || 'Tamanho da Grade'}
                </span>
                <span className="text-[9px] font-mono text-slate-500 dark:text-slate-400">
                  {ribbonCanvasWidth} × {ribbonCanvasHeight} px
                </span>
              </button>
            </div>

            {/* Grade do Relatório */}
            <div className="flex items-center gap-2 px-2 border-r border-slate-200 dark:border-slate-800">
              <div className="flex flex-col justify-center">
                <div className="flex items-center gap-1.5 mb-1">
                  <Grid className="w-4 h-4 text-slate-600 dark:text-slate-300" />
                  <span className="text-[10px] font-bold text-slate-700 dark:text-slate-200">Grade:</span>
                  <select
                    value={gridSize}
                    onChange={(e) => setGridSize(Number(e.target.value))}
                    className="border border-slate-300 dark:border-slate-700 rounded px-1.5 py-0.5 text-[10px] bg-slate-50 dark:bg-slate-800 font-medium text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
                  >
                    {[8, 10, 16, 20, 24, 32].map(size => (
                      <option key={size} value={size}>
                        {size}px
                      </option>
                    ))}
                  </select>
                </div>
                <div className="flex items-center gap-2 text-[10px] text-slate-600 dark:text-slate-300">
                  <label className="flex items-center gap-1 cursor-pointer hover:text-slate-900 dark:hover:text-white select-none">
                    <input
                      type="checkbox"
                      checked={showGrid}
                      onChange={(e) => setShowGrid(e.target.checked)}
                      className="rounded border-slate-300 text-amber-500 focus:ring-amber-400 w-3 h-3"
                    />
                    <span>Linhas</span>
                  </label>
                  <label className="flex items-center gap-1 cursor-pointer hover:text-slate-900 dark:hover:text-white select-none">
                    <input
                      type="checkbox"
                      checked={snapToGrid}
                      onChange={(e) => setSnapToGrid(e.target.checked)}
                      className="rounded border-slate-300 text-amber-500 focus:ring-amber-400 w-3 h-3"
                    />
                    <span>{t.ribbonActions.snapToGrid}</span>
                  </label>
                </div>
              </div>
            </div>

            {/* Zoom */}
            <div className="flex items-center gap-1.5 px-2" title="Zoom (Ctrl+= / Ctrl+- / Ctrl+Scroll / Ctrl+0)">
              <button
                onClick={() => setCanvasZoom(canvasZoom - 10)}
                title="Diminuir Zoom (Ctrl + -)"
                className="p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-300 cursor-pointer"
              >
                <ZoomOut className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={() => setCanvasZoom(100)}
                title="Redefinir Zoom para 100% (Ctrl + 0)"
                className="text-[11px] font-semibold text-slate-700 dark:text-slate-200 w-10 text-center hover:text-amber-600 dark:hover:text-amber-400 cursor-pointer"
              >
                {canvasZoom}%
              </button>
              <button
                onClick={() => setCanvasZoom(canvasZoom + 10)}
                title="Aumentar Zoom (Ctrl + =)"
                className="p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-300 cursor-pointer"
              >
                <ZoomIn className="w-4 h-4" />
              </button>
            </div>
          </>
        )}

        {/* TAB 5: AJUDA */}
        {activeRibbonTab === 'ajuda' && (
          <>
            <div className="flex items-center gap-2 px-2 border-r border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setUpdateModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-emerald-700 dark:text-emerald-400 cursor-pointer"
              >
                <Download className="w-5 h-5" />
                <span className="text-[10px] font-semibold mt-0.5">Atualizações</span>
              </button>
              <button
                onClick={() => setActiveView('daxcompliance')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-amber-700 dark:text-amber-400 cursor-pointer"
              >
                <Zap className="w-5 h-5" />
                <span className="text-[10px] font-semibold mt-0.5">Conformidade DAX</span>
              </button>
              <button
                onClick={() => setShowWelcomeScreen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <Home className="w-5 h-5 text-amber-500" />
                <span className="text-[10px] font-semibold mt-0.5">Tela Inicial</span>
              </button>
              <button
                onClick={() => setLimitationsModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-amber-50 dark:hover:bg-slate-800 text-amber-800 dark:text-amber-300 cursor-pointer"
              >
                <Info className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.nav.limitations}</span>
              </button>
              <button
                onClick={() => setShortcutsModalOpen(true)}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
                title={`${t.shortcuts.menuItem} (${getShortcutHint('help.shortcuts')})`}
              >
                <Keyboard className="w-5 h-5 text-amber-500" />
                <span className="text-[10px] font-semibold mt-0.5">{t.shortcuts.menuItem}</span>
              </button>
              <button
                onClick={() => setActiveView('governance')}
                className="flex flex-col items-center p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 cursor-pointer"
              >
                <ShieldCheck className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                <span className="text-[10px] font-semibold mt-0.5">{t.nav.governance}</span>
              </button>
            </div>
          </>
        )}
      </div>

      {/* Modal de Confirmação Novo Projeto */}
      {showNewProjectModal && (
        <div className="fixed inset-0 bg-black/60 z-[10000] flex items-center justify-center p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl max-w-md w-full p-6 space-y-4">
            <div className="flex items-center gap-3">
              <div className="p-2.5 rounded-xl bg-amber-100 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400">
                <FilePlus className="w-6 h-6" />
              </div>
              <div>
                <h3 className="font-bold text-slate-900 dark:text-white text-base">{t.topBar.confirmNewProjectTitle}</h3>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {t.topBar.confirmNewProjectMessage}
                </p>
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-3 border-t border-slate-100 dark:border-slate-800">
              <button
                type="button"
                onClick={() => setShowNewProjectModal(false)}
                className="px-4 py-2 rounded-lg border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 font-medium text-xs transition-colors cursor-pointer"
              >
                {t.common.cancel}
              </button>
              <button
                type="button"
                onClick={() => {
                  newProject();
                  setShowNewProjectModal(false);
                }}
                className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-600 text-slate-950 font-bold text-xs shadow-xs transition-colors cursor-pointer"
              >
                {t.common.confirm}
              </button>
            </div>
          </div>
        </div>
      )}

      <PowerBiImportModal
        isOpen={showPbiImportModal}
        onClose={() => setShowPbiImportModal(false)}
      />

      {refreshStatus &&
        typeof document !== 'undefined' &&
        createPortal(
          <div
            className={`fixed bottom-4 right-4 z-[10001] max-w-md px-3.5 py-2.5 rounded-xl shadow-lg border text-xs flex items-start gap-2.5 ${
              refreshStatus.type === 'error'
                ? 'bg-red-50 dark:bg-red-950/90 border-red-300 dark:border-red-800 text-red-900 dark:text-red-200'
                : refreshStatus.type === 'warning'
                ? 'bg-amber-50 dark:bg-amber-950/90 border-amber-300 dark:border-amber-800 text-amber-950 dark:text-amber-200'
                : 'bg-emerald-50 dark:bg-emerald-950/90 border-emerald-300 dark:border-emerald-800 text-emerald-950 dark:text-emerald-200'
            }`}
          >
            <RefreshCw className="w-4 h-4 shrink-0 mt-0.5" />
            <div className="flex-1 min-w-0">
              <div className="font-bold">Atualização de Dados ({refreshStatus.timestamp})</div>
              <div className="text-[11px] opacity-90 break-words mt-0.5">{refreshStatus.message}</div>
            </div>
            <button
              type="button"
              onClick={() => setRefreshStatus(null)}
              className="opacity-60 hover:opacity-100 font-bold text-xs px-1 cursor-pointer"
              aria-label="Fechar aviso"
            >
              ✕
            </button>
          </div>,
          document.body
        )}
    </div>
  );
};
