/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useEffect } from 'react';
import { useBiStore } from './store/useBiStore';
import { Ribbon } from './components/layout/Ribbon';
import { SidebarNav } from './components/layout/SidebarNav';
import { ReportView } from './components/report/ReportView';
import { DataView } from './components/data/DataView';
import { ModelView } from './components/model/ModelView';
import { PowerQueryView } from './components/query/PowerQueryView';
import { MetabaseQuestionView } from './components/metabase/MetabaseQuestionView';
import { DashboardsView } from './components/dashboards/DashboardsView';
import { GovernanceView } from './components/governance/GovernanceView';
import { DaxQueryView } from './components/daxquery/DaxQueryView';
import { DaxComplianceView } from './components/compliance/DaxComplianceView';
import { VisualizationsPane } from './components/panels/VisualizationsPane';
import { FieldsPane } from './components/panels/FieldsPane';
import { FiltersPane } from './components/panels/FiltersPane';
import { AiAssistantModal } from './components/ai/AiAssistantModal';
import { DataSourceModal } from './components/modals/DataSourceModal';
import { QuickMeasureModal } from './components/modals/QuickMeasureModal';
import { CustomMeasureModal } from './components/modals/CustomMeasureModal';
import { LimitationsModal } from './components/modals/LimitationsModal';
import { ThemeManagerModal } from './components/modals/ThemeManagerModal';
import { AiSettingsModal } from './components/modals/AiSettingsModal';
import { CanvasSizeModal } from './components/modals/CanvasSizeModal';
import { UpdateModal } from './components/modals/UpdateModal';
import { WelcomeScreen } from './components/welcome/WelcomeScreen';
import { ErrorBoundary } from './components/shared/ErrorBoundary';
import { isTauriRuntime, checkForUpdatesNative } from './bridge/tauriBridge';
import { APP_VERSION } from './version';

export default function App() {
  const activeView = useBiStore(s => s.activeView);
  const showWelcomeScreen = useBiStore(s => s.showWelcomeScreen);
  const isPresentationMode = useBiStore(s => s.isPresentationMode);
  const setIsPresentationMode = useBiStore(s => s.setIsPresentationMode);
  const setAvailableUpdate = useBiStore(s => s.setAvailableUpdate);

  const [showFiltersPane, setShowFiltersPane] = useState(true);
  const [showVisualsPane, setShowVisualsPane] = useState(true);
  const [showFieldsPane, setShowFieldsPane] = useState(true);

  // Silent background check for app updates on startup (Tauri desktop only)
  useEffect(() => {
    if (!isTauriRuntime()) return;
    checkForUpdatesNative(APP_VERSION)
      .then(res => {
        if (res.updateAvailable) {
          setAvailableUpdate(res);
        }
      })
      .catch(() => {});
  }, [setAvailableUpdate]);

  // Allow exiting presentation mode with Escape
  useEffect(() => {
    if (!isPresentationMode) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsPresentationMode(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isPresentationMode, setIsPresentationMode]);

  if (showWelcomeScreen) {
    return (
      <>
        <WelcomeScreen />
        <UpdateModal />
      </>
    );
  }

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-slate-100 dark:bg-slate-950 font-sans text-slate-800 dark:text-slate-100">
      {/* Exit Presentation Mode Floating Pill */}
      {isPresentationMode && (
        <button
          type="button"
          onClick={() => setIsPresentationMode(false)}
          className="fixed top-3 right-3 z-50 px-3 py-1.5 rounded-full bg-slate-900/90 hover:bg-slate-800 text-white border border-slate-700 text-xs font-bold shadow-lg backdrop-blur-xs cursor-pointer"
        >
          Sair da Apresentação (Esc)
        </button>
      )}
      {/* Top Ribbon Toolbar (Hidden in full presentation mode) */}
      {!isPresentationMode && (
        <Ribbon
          showFiltersPane={showFiltersPane}
          setShowFiltersPane={setShowFiltersPane}
          showVisualsPane={showVisualsPane}
          setShowVisualsPane={setShowVisualsPane}
          showFieldsPane={showFieldsPane}
          setShowFieldsPane={setShowFieldsPane}
        />
      )}

      {/* Main Body Workspace */}
      <div className="flex-1 flex overflow-hidden relative">
        {/* Left Vertical Power BI Navigation Bar */}
        {!isPresentationMode && <SidebarNav />}

        {/* Center Main Stage according to Active View */}
        <div className="flex-1 flex overflow-hidden relative">
          {activeView === 'report' && <ReportView />}
          {activeView === 'data' && <DataView />}
          {activeView === 'model' && <ModelView />}
          {activeView === 'query' && <PowerQueryView />}
          {activeView === 'metabase' && <MetabaseQuestionView />}
          {activeView === 'dashboard' && <DashboardsView />}
          {activeView === 'governance' && <GovernanceView />}
          {activeView === 'daxquery' && <DaxQueryView />}
          {activeView === 'daxcompliance' && <DaxComplianceView />}

          {/* Right Collapsible Panes (only visible in Report View) */}
          {activeView === 'report' && !isPresentationMode && (
            <div className="flex h-full shrink-0">
              {showFiltersPane && <FiltersPane />}
              {showVisualsPane && (
                <ErrorBoundary fallbackTitle="Painel de Visualizações">
                  <VisualizationsPane />
                </ErrorBoundary>
              )}
              {showFieldsPane && <FieldsPane />}
            </div>
          )}
        </div>
      </div>

      {/* Interactive Global Modals */}
      <AiAssistantModal />
      <DataSourceModal />
      <QuickMeasureModal />
      <CustomMeasureModal />
      <LimitationsModal />
      <ThemeManagerModal />
      <AiSettingsModal />
      <CanvasSizeModal />
      <UpdateModal />
    </div>
  );
}
