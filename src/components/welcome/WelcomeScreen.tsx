import React, { useEffect, useRef, useState } from 'react';
import {
  FilePlus,
  FileCode2,
  FolderOpen,
  History,
  Sparkles,
  Sun,
  Moon,
  Download,
  Trash2,
  Play,
  ArrowRight,
  Clock,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { loadProjectFromStorage, ProjectState } from '../../engine/storage';
import { APP_VERSION } from '../../version';
import { PowerBiImportModal } from '../modals/PowerBiImportModal';
import daxConformance from '../../data/daxConformance.json';

export const WelcomeScreen: React.FC = () => {
  const {
    newProject,
    loadInitialSampleData,
    loadStateFromProject,
    setShowWelcomeScreen,
    recentProjects,
    addRecentProject,
    removeRecentProject,
    isDarkMode,
    toggleThemeMode,
    setUpdateModalOpen,
    availableUpdate,
  } = useBiStore(
    useShallow(s => ({
      newProject: s.newProject,
      loadInitialSampleData: s.loadInitialSampleData,
      loadStateFromProject: s.loadStateFromProject,
      setShowWelcomeScreen: s.setShowWelcomeScreen,
      recentProjects: s.recentProjects,
      addRecentProject: s.addRecentProject,
      removeRecentProject: s.removeRecentProject,
      isDarkMode: s.isDarkMode,
      toggleThemeMode: s.toggleThemeMode,
      setUpdateModalOpen: s.setUpdateModalOpen,
      availableUpdate: s.availableUpdate,
    }))
  );

  const [pbiModalOpen, setPbiModalOpen] = useState(false);
  const [pbiInitialPath, setPbiInitialPath] = useState<string | null>(null);
  const [lastSavedSession, setLastSavedSession] = useState<ProjectState | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    loadProjectFromStorage()
      .then(saved => {
        if (active && saved && (saved.tables?.length > 0 || saved.pages?.length > 0)) {
          setLastSavedSession(saved);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const handleNewBlank = () => {
    newProject('Novo Projeto');
    setShowWelcomeScreen(false);
  };

  const handleOpenTemplate = () => {
    loadInitialSampleData();
    setShowWelcomeScreen(false);
  };

  const handleContinueLastSession = () => {
    if (!lastSavedSession) return;
    loadStateFromProject(lastSavedSession);
    setShowWelcomeScreen(false);
  };

  const handleInsightFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = ev => {
      try {
        const parsed = JSON.parse(ev.target?.result as string);
        if (parsed && (parsed.tables || parsed.pages)) {
          loadStateFromProject(parsed);
          addRecentProject({
            id: `insight_${Date.now()}`,
            name: parsed.projectName || file.name.replace(/\.(insight|json)$/i, ''),
            type: 'insight',
            openedAt: new Date().toISOString(),
          });
          setShowWelcomeScreen(false);
        } else {
          alert('O arquivo selecionado não contém uma estrutura de projeto .insight válida.');
        }
      } catch {
        alert('Erro ao ler o arquivo .insight. Certifique-se de que é um JSON válido.');
      }
    };
    reader.readAsText(file);
    if (e.target) e.target.value = '';
  };

  const handleOpenRecent = (entry: (typeof recentProjects)[number]) => {
    if ((entry.type === 'pbip' || entry.type === 'pbix') && entry.path) {
      setPbiInitialPath(entry.path);
      setPbiModalOpen(true);
    } else if (entry.type === 'insight') {
      fileInputRef.current?.click();
    }
  };

  return (
    <div className="min-h-screen w-screen flex flex-col bg-slate-100 dark:bg-slate-950 text-slate-800 dark:text-slate-100 overflow-y-auto select-none">
      <input
        ref={fileInputRef}
        type="file"
        accept=".insight,.json"
        onChange={handleInsightFileChange}
        className="hidden"
      />

      {/* Top Bar */}
      <header className="w-full px-8 py-4 flex items-center justify-between border-b border-slate-200 dark:border-slate-800/80 bg-white/80 dark:bg-slate-900/70 backdrop-blur-xs">
        <div className="flex items-center gap-3">
          <img src="/icon.png" alt="PrismBI" className="w-9 h-9 rounded-lg shadow-sm" />
          <div>
            <div className="flex items-center gap-2">
              <span className="font-extrabold text-base tracking-tight text-slate-900 dark:text-white">
                PrismBI
              </span>
              <span className="px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-700 dark:text-amber-300 border border-amber-500/30 text-[10px] font-mono font-bold">
                v{APP_VERSION}
              </span>
            </div>
            <p className="text-[11px] text-slate-500 dark:text-slate-400">
              Business Intelligence nativo para Linux • Rust + DuckDB
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            type="button"
            onClick={() => setUpdateModalOpen(true)}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 border transition-colors cursor-pointer ${
              availableUpdate?.updateAvailable
                ? 'bg-emerald-500 text-slate-950 border-emerald-400 hover:bg-emerald-400 font-bold animate-pulse'
                : 'bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-200 border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'
            }`}
          >
            <Download className="w-3.5 h-3.5" />
            <span>
              {availableUpdate?.updateAvailable
                ? `Atualização v${availableUpdate.latestVersion}`
                : 'Verificar Atualizações'}
            </span>
          </button>

          <button
            type="button"
            onClick={toggleThemeMode}
            title={isDarkMode ? 'Alternar para Modo Claro' : 'Alternar para Modo Escuro'}
            className="p-2 rounded-lg bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-300 hover:text-amber-500 cursor-pointer"
          >
            {isDarkMode ? <Sun className="w-4 h-4 text-amber-400" /> : <Moon className="w-4 h-4" />}
          </button>
        </div>
      </header>

      {/* Center Content */}
      <main className="flex-1 flex items-center justify-center p-6 md:p-10">
        <div className="max-w-4xl w-full space-y-8">
          <div className="text-center space-y-2">
            <h1 className="text-2xl md:text-3xl font-extrabold text-slate-900 dark:text-white tracking-tight">
              Bem-vindo ao PrismBI
            </h1>
            <p className="text-xs md:text-sm text-slate-500 dark:text-slate-400 max-w-xl mx-auto">
              Escolha começar um modelo em branco ou abrir um projeto Power BI (<code className="font-mono">.pbip</code> /{' '}
              <code className="font-mono">.pbix</code>) ou snapshot <code className="font-mono">.insight</code>.
            </p>
          </div>

          {/* Primary Actions Grid */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <button
              type="button"
              onClick={handleNewBlank}
              className="group text-left p-5 rounded-2xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 hover:border-amber-500 dark:hover:border-amber-500 shadow-xs hover:shadow-md transition-all cursor-pointer flex flex-col justify-between gap-4"
            >
              <div className="p-3 rounded-xl bg-amber-500/15 text-amber-600 dark:text-amber-400 w-fit group-hover:scale-105 transition-transform">
                <FilePlus className="w-6 h-6" />
              </div>
              <div>
                <h2 className="font-bold text-sm text-slate-900 dark:text-white flex items-center justify-between">
                  <span>Novo Projeto em Branco</span>
                  <ArrowRight className="w-4 h-4 text-slate-400 group-hover:text-amber-500 transition-colors" />
                </h2>
                <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-1 leading-relaxed">
                  Comece do zero conectando arquivos Excel, CSV, Parquet, JSON ou API REST via DuckDB.
                </p>
              </div>
            </button>

            <button
              type="button"
              onClick={() => {
                setPbiInitialPath(null);
                setPbiModalOpen(true);
              }}
              className="group text-left p-5 rounded-2xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 hover:border-emerald-500 dark:hover:border-emerald-500 shadow-xs hover:shadow-md transition-all cursor-pointer flex flex-col justify-between gap-4"
            >
              <div className="p-3 rounded-xl bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 w-fit group-hover:scale-105 transition-transform">
                <FileCode2 className="w-6 h-6" />
              </div>
              <div>
                <h2 className="font-bold text-sm text-slate-900 dark:text-white flex items-center justify-between">
                  <span>Abrir .pbip ou .pbix</span>
                  <ArrowRight className="w-4 h-4 text-slate-400 group-hover:text-emerald-500 transition-colors" />
                </h2>
                <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-1 leading-relaxed">
                  Importe diretamente projetos Power BI completos com TMDL, PBIR, medidas DAX e Power Query.
                </p>
              </div>
            </button>

            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="group text-left p-5 rounded-2xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 hover:border-blue-500 dark:hover:border-blue-500 shadow-xs hover:shadow-md transition-all cursor-pointer flex flex-col justify-between gap-4"
            >
              <div className="p-3 rounded-xl bg-blue-500/15 text-blue-600 dark:text-blue-400 w-fit group-hover:scale-105 transition-transform">
                <FolderOpen className="w-6 h-6" />
              </div>
              <div>
                <h2 className="font-bold text-sm text-slate-900 dark:text-white flex items-center justify-between">
                  <span>Abrir Snapshot .insight</span>
                  <ArrowRight className="w-4 h-4 text-slate-400 group-hover:text-blue-500 transition-colors" />
                </h2>
                <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-1 leading-relaxed">
                  Restaure rapidamente um arquivo salvo no formato portátil <code className="font-mono">.insight</code>.
                </p>
              </div>
            </button>
          </div>

          {/* Last Saved Session + Recent Projects */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="p-4 rounded-2xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 space-y-3">
              <div className="flex items-center gap-2 text-xs font-bold text-slate-700 dark:text-slate-200">
                <Clock className="w-4 h-4 text-amber-500" />
                <span>Sessão Anterior</span>
              </div>
              {lastSavedSession ? (
                <button
                  type="button"
                  onClick={handleContinueLastSession}
                  className="w-full p-3 rounded-xl bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800/70 hover:border-amber-500 text-left flex items-center justify-between transition-colors cursor-pointer"
                >
                  <div className="overflow-hidden">
                    <div className="font-bold text-xs text-slate-900 dark:text-white truncate">
                      {lastSavedSession.projectName || 'Projeto Salvo'}
                    </div>
                    <div className="text-[10px] text-slate-500 dark:text-slate-400 mt-0.5">
                      {lastSavedSession.tables?.length || 0} tabelas • {lastSavedSession.pages?.length || 0} páginas
                    </div>
                  </div>
                  <span className="px-2.5 py-1 rounded-lg bg-amber-500 text-slate-950 font-bold text-[11px] shrink-0">
                    Continuar
                  </span>
                </button>
              ) : (
                <p className="text-[11px] text-slate-400 dark:text-slate-500 py-3">
                  Nenhuma sessão local salva encontrada ainda.
                </p>
              )}

              <div className="pt-2 border-t border-slate-100 dark:border-slate-800 flex items-center justify-between">
                <span className="text-[11px] text-slate-500 dark:text-slate-400">
                  Quer testar os gráficos e fórmulas antes?
                </span>
                <button
                  type="button"
                  onClick={handleOpenTemplate}
                  className="px-2.5 py-1 rounded-lg bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 text-[11px] font-semibold flex items-center gap-1 cursor-pointer"
                >
                  <Play className="w-3 h-3 text-amber-500" />
                  <span>Explorar Exemplo</span>
                </button>
              </div>
            </div>

            <div className="p-4 rounded-2xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 space-y-3">
              <div className="flex items-center gap-2 text-xs font-bold text-slate-700 dark:text-slate-200">
                <History className="w-4 h-4 text-blue-500" />
                <span>Projetos Recentes</span>
              </div>
              {recentProjects.length === 0 ? (
                <p className="text-[11px] text-slate-400 dark:text-slate-500 py-3">
                  Os projetos <code className="font-mono">.pbip</code>, <code className="font-mono">.pbix</code> e{' '}
                  <code className="font-mono">.insight</code> abertos aparecerão aqui.
                </p>
              ) : (
                <div className="space-y-1.5 max-h-40 overflow-y-auto pr-1">
                  {recentProjects.map(entry => (
                    <div
                      key={entry.id}
                      className="flex items-center justify-between gap-2 p-2 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-200/80 dark:border-slate-800 hover:border-amber-500/60 group"
                    >
                      <button
                        type="button"
                        onClick={() => handleOpenRecent(entry)}
                        className="flex-1 text-left overflow-hidden cursor-pointer"
                      >
                        <div className="flex items-center gap-1.5">
                          <span className="px-1.5 py-0.2 rounded bg-slate-200 dark:bg-slate-700 text-[9px] font-mono font-bold uppercase text-slate-700 dark:text-slate-200">
                            .{entry.type}
                          </span>
                          <span className="font-semibold text-xs text-slate-800 dark:text-slate-100 truncate">
                            {entry.name}
                          </span>
                        </div>
                        {entry.path && (
                          <div className="text-[10px] font-mono text-slate-400 truncate mt-0.5">
                            {entry.path}
                          </div>
                        )}
                      </button>
                      <button
                        type="button"
                        onClick={() => removeRecentProject(entry.id)}
                        title="Remover dos recentes"
                        className="p-1 text-slate-400 hover:text-rose-500 opacity-60 group-hover:opacity-100 cursor-pointer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Shortcut hints */}
          <div className="text-center text-[11px] text-slate-400 dark:text-slate-500 flex flex-wrap items-center justify-center gap-x-4 gap-y-1">
            <span title="Casos do corpus de conformidade com resultado verificado (docs/DAX_CONFORMANCE.md)">
              <Sparkles className="w-3 h-3 inline text-amber-500 mr-1" />
              DAX verificado: {daxConformance.passing}/{daxConformance.total} casos
            </span>
            <span>•</span>
            <span>Scroll do mouse pressionado: mover canvas livremente</span>
            <span>•</span>
            <span>Ctrl + Scroll / Ctrl + = / - : Zoom</span>
          </div>
        </div>
      </main>

      <PowerBiImportModal
        isOpen={pbiModalOpen}
        initialPath={pbiInitialPath}
        onClose={() => {
          setPbiModalOpen(false);
          setPbiInitialPath(null);
        }}
        onLoaded={() => setShowWelcomeScreen(false)}
      />
    </div>
  );
};
