import React, { useRef, useEffect, useState, useCallback } from 'react';
import {
  Plus,
  Copy,
  Trash2,
  Edit2,
  Smartphone,
  ArrowLeft,
  Target,
  X,
  LayoutGrid,
  Maximize2,
  AlignStartVertical,
  AlignCenterVertical,
  AlignEndVertical,
  AlignStartHorizontal,
  AlignCenterHorizontal,
  AlignEndHorizontal,
  AlignHorizontalDistributeCenter,
  AlignVerticalDistributeCenter,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { VisualCard, SnapGuide } from './VisualCard';
import { ErrorBoundary } from '../shared/ErrorBoundary';
import { useTranslation } from '../../i18n';
import { getEffectiveCanvasSize } from '../../engine/shared/canvasPresets';

export const ReportView: React.FC = () => {
  const { t } = useTranslation();
  const {
    pages,
    activePageId,
    setActivePageId,
    addPage,
    renamePage,
    duplicatePage,
    deletePage,
    selectedVisualId,
    selectedVisualIds,
    setSelectedVisualId,
    toggleVisualSelection,
    moveVisualsDelta,
    deleteSelectedVisuals,
    alignSelectedVisuals,
    undo,
    redo,
    canvasZoom,
    setCanvasZoom,
    layoutMode,
    gridSize,
    showGrid,
    activeDrillthrough,
    clearDrillthrough,
    setCanvasSizeModalOpen,
    isDarkMode,
  } = useBiStore(useShallow(s => ({
    pages: s.pages,
    activePageId: s.activePageId,
    setActivePageId: s.setActivePageId,
    addPage: s.addPage,
    renamePage: s.renamePage,
    duplicatePage: s.duplicatePage,
    deletePage: s.deletePage,
    selectedVisualId: s.selectedVisualId,
    selectedVisualIds: s.selectedVisualIds,
    setSelectedVisualId: s.setSelectedVisualId,
    toggleVisualSelection: s.toggleVisualSelection,
    moveVisualsDelta: s.moveVisualsDelta,
    deleteSelectedVisuals: s.deleteSelectedVisuals,
    alignSelectedVisuals: s.alignSelectedVisuals,
    undo: s.undo,
    redo: s.redo,
    canvasZoom: s.canvasZoom,
    setCanvasZoom: s.setCanvasZoom,
    layoutMode: s.layoutMode,
    gridSize: s.gridSize,
    showGrid: s.showGrid,
    activeDrillthrough: s.activeDrillthrough,
    clearDrillthrough: s.clearDrillthrough,
    setCanvasSizeModalOpen: s.setCanvasSizeModalOpen,
    isDarkMode: s.isDarkMode,
  })));

  const activePage = pages.find(p => p.id === activePageId) || pages[0];
  const { width: canvasWidth, height: canvasHeight, label: canvasSizeLabel } = getEffectiveCanvasSize(activePage);
  const canvasRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [snapGuides, setSnapGuides] = useState<SnapGuide[]>([]);

  const handleSelectVisual = useCallback(
    (id: string, additive?: boolean) => {
      if (additive) {
        toggleVisualSelection(id);
      } else {
        setSelectedVisualId(id);
      }
    },
    [setSelectedVisualId, toggleVisualSelection]
  );

  useEffect(() => {
    const isEditableElement = (el: EventTarget | null) => {
      const htmlEl = el as HTMLElement | null;
      if (!htmlEl) return false;
      const tag = htmlEl.tagName;
      return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || htmlEl.isContentEditable;
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (isEditableElement(e.target) || isEditableElement(document.activeElement)) {
        return;
      }

      if (e.ctrlKey || e.metaKey) {
        if (e.key === '=' || e.key === '+') {
          e.preventDefault();
          setCanvasZoom(useBiStore.getState().canvasZoom + 10);
        } else if (e.key === '-' || e.key === '_') {
          e.preventDefault();
          setCanvasZoom(useBiStore.getState().canvasZoom - 10);
        } else if (e.key === '0') {
          e.preventDefault();
          setCanvasZoom(100);
        } else if (e.key.toLowerCase() === 'z' && !e.shiftKey) {
          e.preventDefault();
          undo();
        } else if (e.key.toLowerCase() === 'y' || (e.key.toLowerCase() === 'z' && e.shiftKey)) {
          e.preventDefault();
          redo();
        }
        return;
      }

      const currentSelected = useBiStore.getState().selectedVisualIds;
      if (e.key === 'Escape' && currentSelected.length > 0) {
        setSelectedVisualId(null);
        return;
      }

      if ((e.key === 'Delete' || e.key === 'Backspace') && currentSelected.length > 0) {
        e.preventDefault();
        deleteSelectedVisuals();
        return;
      }

      if (currentSelected.length > 0 && !e.altKey) {
        const step = e.shiftKey ? 10 : 1;
        if (e.key === 'ArrowLeft') {
          e.preventDefault();
          moveVisualsDelta(currentSelected, -step, 0);
        } else if (e.key === 'ArrowRight') {
          e.preventDefault();
          moveVisualsDelta(currentSelected, step, 0);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          moveVisualsDelta(currentSelected, 0, -step);
        } else if (e.key === 'ArrowDown') {
          e.preventDefault();
          moveVisualsDelta(currentSelected, 0, step);
        }
      }
    };

    const handleWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      e.preventDefault();
      const step = e.deltaY < 0 ? 10 : -10;
      setCanvasZoom(useBiStore.getState().canvasZoom + step);
    };

    const el = scrollContainerRef.current;
    window.addEventListener('keydown', handleKeyDown);
    el?.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      el?.removeEventListener('wheel', handleWheel);
    };
  }, [setCanvasZoom, undo, redo, setSelectedVisualId, deleteSelectedVisuals, moveVisualsDelta]);

  const handleCanvasClick = (e: React.MouseEvent) => {
    if (e.target === canvasRef.current || e.target === scrollContainerRef.current) {
      setSelectedVisualId(null);
    }
  };

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-slate-200/70 dark:bg-slate-950 select-none">
      {/* Active Drill-Through Top Navigation Banner */}
      {activeDrillthrough && (
        <div className="bg-gradient-to-r from-amber-500 via-amber-600 to-amber-700 text-slate-950 px-4 py-2 flex items-center justify-between shadow-md shrink-0 z-30 border-b border-amber-600">
          <div className="flex items-center gap-3">
            <button
              onClick={() => clearDrillthrough(true)}
              className="flex items-center gap-1.5 px-3 py-1 bg-slate-950 text-white rounded-lg text-xs font-bold hover:bg-slate-800 transition-all shadow-xs cursor-pointer hover:scale-102"
              title={`Retornar à página ${activeDrillthrough.sourcePageName}`}
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              <span>Voltar para {activeDrillthrough.sourcePageName}</span>
            </button>

            <div className="flex items-center gap-2 text-xs">
              <span className="bg-amber-300 text-slate-950 px-2.5 py-0.5 rounded-full font-bold flex items-center gap-1 border border-amber-400">
                <Target className="w-3.5 h-3.5 text-slate-900" />
                Detalhamento Ativo (Drill-Through)
              </span>
              <span className="text-white font-medium">
                Entidade filtrada: <strong className="text-amber-100">{activeDrillthrough.field}</strong> ={' '}
                <span className="bg-white text-slate-900 px-2 py-0.5 rounded font-bold shadow-xs">
                  {String(activeDrillthrough.value)}
                </span>
              </span>
              <span className="text-amber-100/80 text-[11px] hidden md:inline">
                • Origem: {activeDrillthrough.sourceVisualTitle}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => clearDrillthrough(false)}
              className="px-2.5 py-1 text-xs bg-amber-400/90 hover:bg-amber-300 text-slate-950 rounded-lg font-semibold transition-colors cursor-pointer"
              title="Manter nesta página mas remover o filtro do detalhamento"
            >
              Limpar Filtro
            </button>
            <button
              onClick={() => clearDrillthrough(true)}
              className="p-1 text-amber-950 hover:bg-amber-600/50 rounded-lg transition-colors cursor-pointer"
              title="Fechar detalhamento e voltar"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Central Canvas Area */}
      <div
        ref={scrollContainerRef}
        className="flex-1 overflow-auto p-6"
        onClick={handleCanvasClick}
      >
        {layoutMode === 'mobile' ? (
          // Mobile View Container
          <div className="w-[380px] min-h-[680px] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl border-8 border-slate-800 dark:border-slate-700 p-4 relative flex flex-col gap-4 overflow-y-auto">
            <div className="w-24 h-4 bg-slate-800 dark:bg-slate-700 rounded-full mx-auto mb-2 shrink-0"></div>
            <div className="text-center font-bold text-slate-800 dark:text-slate-200 text-sm mb-1">
              {activePage?.name || 'Página'} (Layout Celular)
            </div>
            {activePage?.visuals.map(visual => (
              <div key={visual.id} className="relative h-[220px] w-full shrink-0">
                <ErrorBoundary fallbackTitle={visual.title}>
                  <VisualCard
                    visual={visual}
                    isMobile={true}
                    isSelected={selectedVisualId === visual.id}
                    onSelect={setSelectedVisualId}
                  />
                </ErrorBoundary>
              </div>
            ))}
            {(!activePage?.visuals || activePage?.visuals.length === 0) && (
              <div className="text-center text-xs text-slate-400 dark:text-slate-500 py-12">
                Nenhum visual nesta página
              </div>
            )}
          </div>
        ) : (
          // Desktop Free Canvas
          (() => {
            const gSize = gridSize || 20;
            const dotColor = isDarkMode ? '%23334155' : '%23cbd5e1';
            const gridSvgUri = showGrid
              ? `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='${gSize}' height='${gSize}'%3E%3Ccircle cx='1' cy='1' r='1' fill='${dotColor}'/%3E%3C/svg%3E")`
              : 'none';
            const isMulti = selectedVisualIds.length >= 2;
            return (
              <div className="relative flex flex-col items-center">
                {isMulti && (
                  <div
                    onClick={e => e.stopPropagation()}
                    className="sticky top-2 z-40 mb-3 flex items-center gap-1 px-3 py-1.5 rounded-full bg-white/95 dark:bg-slate-800/95 border border-slate-300 dark:border-slate-700 shadow-lg backdrop-blur-xs text-xs text-slate-700 dark:text-slate-200"
                  >
                    <span className="font-semibold text-amber-600 dark:text-amber-400 pr-2 border-r border-slate-200 dark:border-slate-700">
                      {selectedVisualIds.length} selecionados
                    </span>
                    <button
                      onClick={() => alignSelectedVisuals('left')}
                      title="Alinhar à esquerda"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignStartVertical className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => alignSelectedVisuals('center')}
                      title="Centralizar horizontalmente"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignCenterVertical className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => alignSelectedVisuals('right')}
                      title="Alinhar à direita"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignEndVertical className="w-3.5 h-3.5" />
                    </button>
                    <span className="w-px h-4 bg-slate-200 dark:bg-slate-700 mx-0.5" />
                    <button
                      onClick={() => alignSelectedVisuals('top')}
                      title="Alinhar ao topo"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignStartHorizontal className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => alignSelectedVisuals('middle')}
                      title="Centralizar verticalmente"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignCenterHorizontal className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => alignSelectedVisuals('bottom')}
                      title="Alinhar à base"
                      className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                    >
                      <AlignEndHorizontal className="w-3.5 h-3.5" />
                    </button>
                    {selectedVisualIds.length >= 3 && (
                      <>
                        <span className="w-px h-4 bg-slate-200 dark:bg-slate-700 mx-0.5" />
                        <button
                          onClick={() => alignSelectedVisuals('distribute-h')}
                          title="Distribuir horizontalmente"
                          className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                        >
                          <AlignHorizontalDistributeCenter className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => alignSelectedVisuals('distribute-v')}
                          title="Distribuir verticalmente"
                          className="p-1.5 rounded hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                        >
                          <AlignVerticalDistributeCenter className="w-3.5 h-3.5" />
                        </button>
                      </>
                    )}
                    <span className="w-px h-4 bg-slate-200 dark:bg-slate-700 mx-0.5" />
                    <button
                      onClick={() => deleteSelectedVisuals()}
                      title="Excluir selecionados (Delete)"
                      className="p-1.5 rounded text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/50 cursor-pointer"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}

                <div
                  ref={canvasRef}
                  onClick={handleCanvasClick}
                  style={{
                    transform: canvasZoom !== 100 ? `scale(${canvasZoom / 100})` : undefined,
                    transformOrigin: canvasZoom !== 100 ? 'top center' : undefined,
                    width: canvasWidth,
                    height: canvasHeight,
                    backgroundImage: gridSvgUri,
                    backgroundSize: `${gSize}px ${gSize}px`,
                  }}
                  className="mx-auto bg-white dark:bg-slate-900 rounded shadow-sm border border-slate-300 dark:border-slate-800 relative shrink-0"
                >
                  {activePage?.visuals.map(visual => {
                    const visSelected = selectedVisualIds.includes(visual.id) || selectedVisualId === visual.id;
                    return (
                      <ErrorBoundary key={visual.id} fallbackTitle={visual.title}>
                        <VisualCard
                          visual={visual}
                          isSelected={visSelected}
                          isMultiSelected={isMulti && selectedVisualIds.includes(visual.id)}
                          onSelect={handleSelectVisual}
                          onSnapGuidesChange={setSnapGuides}
                        />
                      </ErrorBoundary>
                    );
                  })}

                  {/* Smart Snap Guides (60fps canvas alignment lines) */}
                  {snapGuides.map((g, idx) =>
                    g.orientation === 'v' ? (
                      <div
                        key={`snap-v-${idx}`}
                        className="absolute top-0 bottom-0 w-px bg-rose-500/90 pointer-events-none z-50 shadow-[0_0_4px_rgba(244,63,94,0.65)]"
                        style={{ left: g.position }}
                      />
                    ) : (
                      <div
                        key={`snap-h-${idx}`}
                        className="absolute left-0 right-0 h-px bg-rose-500/90 pointer-events-none z-50 shadow-[0_0_4px_rgba(244,63,94,0.65)]"
                        style={{ top: g.position }}
                      />
                    )
                  )}

                  {(!activePage?.visuals || activePage?.visuals.length === 0) && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center text-slate-400 dark:text-slate-500 pointer-events-none">
                      <p className="text-sm font-semibold">{t.report.emptyPageTitle}</p>
                      <p className="text-xs mt-1">{t.report.emptyPageSubtitle}</p>
                    </div>
                  )}
                </div>
              </div>
            );
          })()
        )}
      </div>

      {/* Bottom Page Navigation Bar */}
      <div className="h-9 bg-white dark:bg-slate-900 border-t border-slate-300 dark:border-slate-800 flex items-center justify-between px-3 text-xs shrink-0 select-none">
        <div className="flex items-center gap-1 overflow-x-auto">
          {pages.map(page => {
            const isActive = page.id === activePageId;
            return (
              <div
                key={page.id}
                onClick={() => setActivePageId(page.id)}
                className={`group flex items-center gap-2 px-3 py-1 rounded-t border-t-2 cursor-pointer transition-all ${
                  isActive
                    ? 'border-amber-500 bg-amber-50/50 dark:bg-amber-950/40 text-slate-900 dark:text-amber-400 font-bold shadow-xs'
                    : 'border-transparent text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-slate-200'
                }`}
              >
                <span className="truncate max-w-[140px] text-[11px]">{page.name}</span>

                {/* Page Quick Actions (rename, duplicate, delete) */}
                {isActive && (
                  <div className="flex items-center gap-1 opacity-60 group-hover:opacity-100">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        const newName = prompt('Novo nome para a página:', page.name);
                        if (newName) renamePage(page.id, newName);
                      }}
                      className="p-0.5 hover:text-amber-700 dark:hover:text-amber-400"
                      title={t.report.renamePage}
                    >
                      <Edit2 className="w-2.5 h-2.5" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        duplicatePage(page.id);
                      }}
                      className="p-0.5 hover:text-amber-700 dark:hover:text-amber-400"
                      title={t.report.duplicatePage}
                    >
                      <Copy className="w-2.5 h-2.5" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setCanvasSizeModalOpen(true);
                      }}
                      className="p-0.5 hover:text-amber-700 dark:hover:text-amber-400"
                      title={t.report.canvasSize || 'Ajustar Tamanho da Grade'}
                    >
                      <Maximize2 className="w-2.5 h-2.5" />
                    </button>
                    {pages.length > 1 && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          if (confirm(`Excluir página "${page.name}"?`)) {
                            deletePage(page.id);
                          }
                        }}
                        className="p-0.5 hover:text-red-600 dark:hover:text-red-400"
                        title={t.report.deletePage}
                      >
                        <Trash2 className="w-2.5 h-2.5" />
                      </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}

          <button
            onClick={() => addPage()}
            className="flex items-center gap-1 text-[11px] font-medium text-slate-600 dark:text-slate-400 hover:text-amber-600 dark:hover:text-amber-400 px-2 py-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors ml-1"
            title={t.report.addPage}
          >
            <Plus className="w-3.5 h-3.5" />
            <span>{t.report.addPage}</span>
          </button>
        </div>

        <button
          onClick={() => setCanvasSizeModalOpen(true)}
          className="flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] text-slate-600 dark:text-slate-400 hover:text-slate-950 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors font-medium cursor-pointer border border-transparent hover:border-slate-300 dark:hover:border-slate-700"
          title="Clique para alterar o tamanho da grade / resolução desta página"
        >
          <LayoutGrid className="w-3 h-3 text-amber-600 shrink-0" />
          <span>
            Grade: <strong className="font-mono text-slate-800 dark:text-slate-200">{canvasWidth} × {canvasHeight} px</strong> • {canvasSizeLabel}
          </span>
        </button>
      </div>
    </div>
  );
};
