import React, { useState } from 'react';
import { Palette, Check, Plus, Trash2, X, Sparkles, Sun, Moon, Monitor, LayoutDashboard } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore, PRESET_THEMES } from '../../store/useBiStore';
import { BiTheme } from '../../types/bi';

export const ThemeManagerModal: React.FC = () => {
  const {
    themeModalOpen,
    setThemeModalOpen,
    activeThemeId,
    setThemeId,
    customThemes,
    addCustomTheme,
    deleteCustomTheme,
    themeMode,
    setThemeMode,
    isDarkMode,
  } = useBiStore(useShallow(s => ({
    themeModalOpen: s.themeModalOpen,
    setThemeModalOpen: s.setThemeModalOpen,
    activeThemeId: s.activeThemeId,
    setThemeId: s.setThemeId,
    customThemes: s.customThemes,
    addCustomTheme: s.addCustomTheme,
    deleteCustomTheme: s.deleteCustomTheme,
    themeMode: s.themeMode,
    setThemeMode: s.setThemeMode,
    isDarkMode: s.isDarkMode,
  })));

  const [activeTab, setActiveTab] = useState<'interface' | 'presets' | 'custom'>('interface');
  const [customName, setCustomName] = useState('Novo Tema');
  const [customColors, setCustomColors] = useState<string[]>([
    '#f59e0b',
    '#3b82f6',
    '#10b981',
    '#8b5cf6',
    '#ec4899',
    '#06b6d4',
  ]);
  const [newColorInput, setNewColorInput] = useState('#64748b');

  if (!themeModalOpen) return null;

  const allThemes = [...PRESET_THEMES, ...customThemes];

  const handleSaveCustom = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = customName.trim();
    if (!trimmed) return;

    const newTheme: BiTheme = {
      id: 'theme_custom_' + Math.random().toString(36).substr(2, 6),
      name: trimmed,
      palette: customColors,
      backgroundColor: '#ffffff',
      cardBackground: '#ffffff',
      borderColor: '#e2e8f0',
      textColor: '#1e293b',
      isCustom: true,
    };

    addCustomTheme(newTheme);
    setCustomName('Novo Tema');
    setActiveTab('presets');
  };

  const handleUpdateColor = (index: number, newColor: string) => {
    setCustomColors(prev => {
      const copy = [...prev];
      copy[index] = newColor;
      return copy;
    });
  };

  const handleRemoveColor = (index: number) => {
    if (customColors.length <= 2) return;
    setCustomColors(prev => prev.filter((_, i) => i !== index));
  };

  const handleAddColor = () => {
    if (customColors.length >= 10) return;
    setCustomColors(prev => [...prev, newColorInput]);
  };

  return (
    <div className="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4">
      <div className="bg-white dark:bg-slate-900 rounded-2xl shadow-2xl max-w-2xl w-full flex flex-col max-h-[85vh] overflow-hidden border border-slate-200 dark:border-slate-800 text-slate-800 dark:text-slate-100 animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-5 py-4 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between bg-slate-50 dark:bg-slate-900/60">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-amber-100 dark:bg-amber-950/60 text-amber-700 dark:text-amber-400">
              <Palette className="w-5 h-5" />
            </div>
            <div>
              <h2 className="font-bold text-slate-900 dark:text-slate-100 text-base">Aparência & Paletas de Cores</h2>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                Alterne entre modo claro/escuro e escolha paletas visuais para gráficos e cartões
              </p>
            </div>
          </div>
          <button
            onClick={() => setThemeModalOpen(false)}
            className="p-1 rounded-lg hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex items-center px-5 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-xs font-semibold">
          <button
            onClick={() => setActiveTab('interface')}
            className={`py-2.5 px-4 border-b-2 flex items-center gap-1.5 transition-all ${
              activeTab === 'interface'
                ? 'border-amber-500 text-amber-900 dark:text-amber-400 bg-amber-50/50 dark:bg-amber-950/20'
                : 'border-transparent text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'
            }`}
          >
            {isDarkMode ? <Moon className="w-3.5 h-3.5 text-amber-400" /> : <Sun className="w-3.5 h-3.5 text-amber-600" />}
            <span>Tema da Aplicação</span>
          </button>
          <button
            onClick={() => setActiveTab('presets')}
            className={`py-2.5 px-4 border-b-2 transition-all ${
              activeTab === 'presets'
                ? 'border-amber-500 text-amber-900 dark:text-amber-400 bg-amber-50/50 dark:bg-amber-950/20'
                : 'border-transparent text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'
            }`}
          >
            Paletas dos Gráficos ({allThemes.length})
          </button>
          <button
            onClick={() => setActiveTab('custom')}
            className={`py-2.5 px-4 border-b-2 flex items-center gap-1.5 transition-all ${
              activeTab === 'custom'
                ? 'border-amber-500 text-amber-900 dark:text-amber-400 bg-amber-50/50 dark:bg-amber-950/20'
                : 'border-transparent text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200'
            }`}
          >
            <Plus className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
            <span>Criar Paleta</span>
          </button>
        </div>

        {/* Content Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {activeTab === 'interface' && (
            <div className="space-y-4">
              <div>
                <h3 className="text-xs font-bold text-slate-800 dark:text-slate-200 uppercase tracking-wider mb-1">
                  Modo da Interface (InsightHub UI)
                </h3>
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  Sua escolha é salva no armazenamento local (<code className="bg-slate-100 dark:bg-slate-800 px-1 py-0.5 rounded text-[11px]">localStorage</code>) e preservada entre sessões.
                </p>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                {/* Light Mode Card */}
                <button
                  type="button"
                  onClick={() => setThemeMode('light')}
                  className={`p-4 rounded-xl border-2 text-left flex flex-col justify-between gap-3 transition-all ${
                    themeMode === 'light'
                      ? 'border-amber-500 bg-amber-50/40 dark:bg-amber-950/20 shadow-md ring-2 ring-amber-400/30'
                      : 'border-slate-200 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700 bg-white dark:bg-slate-800/50'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="p-2 rounded-lg bg-amber-100 text-amber-700">
                      <Sun className="w-5 h-5" />
                    </div>
                    {themeMode === 'light' && (
                      <div className="w-5 h-5 rounded-full bg-amber-500 text-slate-950 flex items-center justify-center">
                        <Check className="w-3 h-3 stroke-[3]" />
                      </div>
                    )}
                  </div>
                  <div>
                    <h4 className="font-bold text-sm text-slate-800 dark:text-slate-100">Modo Claro</h4>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                      Fundo luminoso para ambientes com alta claridade
                    </p>
                  </div>
                  <div className="h-8 rounded bg-slate-100 border border-slate-200 flex items-center px-2 gap-1.5">
                    <div className="w-2.5 h-2.5 rounded-full bg-amber-500" />
                    <div className="w-10 h-1.5 rounded bg-slate-300" />
                    <div className="flex-1" />
                    <div className="w-6 h-3 rounded bg-white border border-slate-200" />
                  </div>
                </button>

                {/* Dark Mode Card */}
                <button
                  type="button"
                  onClick={() => setThemeMode('dark')}
                  className={`p-4 rounded-xl border-2 text-left flex flex-col justify-between gap-3 transition-all ${
                    themeMode === 'dark'
                      ? 'border-amber-500 bg-amber-50/40 dark:bg-amber-950/20 shadow-md ring-2 ring-amber-400/30'
                      : 'border-slate-200 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700 bg-white dark:bg-slate-800/50'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="p-2 rounded-lg bg-indigo-900/60 text-indigo-300">
                      <Moon className="w-5 h-5" />
                    </div>
                    {themeMode === 'dark' && (
                      <div className="w-5 h-5 rounded-full bg-amber-500 text-slate-950 flex items-center justify-center">
                        <Check className="w-3 h-3 stroke-[3]" />
                      </div>
                    )}
                  </div>
                  <div>
                    <h4 className="font-bold text-sm text-slate-800 dark:text-slate-100">Modo Escuro</h4>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                      Paleta escura de alto contraste para foco prolongado
                    </p>
                  </div>
                  <div className="h-8 rounded bg-slate-900 border border-slate-700 flex items-center px-2 gap-1.5">
                    <div className="w-2.5 h-2.5 rounded-full bg-amber-400" />
                    <div className="w-10 h-1.5 rounded bg-slate-700" />
                    <div className="flex-1" />
                    <div className="w-6 h-3 rounded bg-slate-800 border border-slate-700" />
                  </div>
                </button>

                {/* System Mode Card */}
                <button
                  type="button"
                  onClick={() => setThemeMode('system')}
                  className={`p-4 rounded-xl border-2 text-left flex flex-col justify-between gap-3 transition-all ${
                    themeMode === 'system'
                      ? 'border-amber-500 bg-amber-50/40 dark:bg-amber-950/20 shadow-md ring-2 ring-amber-400/30'
                      : 'border-slate-200 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700 bg-white dark:bg-slate-800/50'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="p-2 rounded-lg bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300">
                      <Monitor className="w-5 h-5" />
                    </div>
                    {themeMode === 'system' && (
                      <div className="w-5 h-5 rounded-full bg-amber-500 text-slate-950 flex items-center justify-center">
                        <Check className="w-3 h-3 stroke-[3]" />
                      </div>
                    )}
                  </div>
                  <div>
                    <h4 className="font-bold text-sm text-slate-800 dark:text-slate-100">Automático</h4>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                      Segue automaticamente a preferência do sistema operacional
                    </p>
                  </div>
                  <div className="h-8 rounded bg-gradient-to-r from-slate-200 to-slate-900 border border-slate-300 dark:border-slate-700 flex items-center px-2 justify-between">
                    <span className="text-[10px] font-semibold text-slate-700">Claro</span>
                    <span className="text-[10px] font-semibold text-slate-200">Escuro</span>
                  </div>
                </button>
              </div>

              <div className="p-3.5 rounded-xl bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-700/80 flex items-center justify-between text-xs">
                <div className="flex items-center gap-2">
                  <span className="text-slate-600 dark:text-slate-400 font-medium">Estado Atual em Execução:</span>
                  <span className="px-2 py-0.5 rounded-full bg-amber-100 dark:bg-amber-950/80 text-amber-800 dark:text-amber-400 font-bold text-[11px]">
                    {isDarkMode ? '🌙 Escuro Ativo' : '☀️ Claro Ativo'}
                  </span>
                </div>
                <span className="text-slate-400 dark:text-slate-500 text-[11px]">
                  {themeMode === 'system' ? 'Baseado em prefers-color-scheme' : 'Configurado manualmente'}
                </span>
              </div>
            </div>
          )}

          {activeTab === 'presets' && (
            <div className="space-y-3">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {allThemes.map(t => {
                  const isActive = activeThemeId === t.id;
                  return (
                    <div
                      key={t.id}
                      onClick={() => setThemeId(t.id)}
                      className={`p-3.5 rounded-xl border-2 cursor-pointer transition-all flex flex-col justify-between gap-3 ${
                        isActive
                          ? 'border-amber-500 bg-amber-50/30 dark:bg-amber-950/30 shadow-md ring-2 ring-amber-400/30'
                          : 'border-slate-200 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700 hover:bg-slate-50/60 dark:hover:bg-slate-800/40'
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-1.5">
                          <span className="font-bold text-xs text-slate-800 dark:text-slate-100">{t.name}</span>
                          {t.isCustom && (
                            <span className="text-[9px] bg-purple-100 dark:bg-purple-950/60 text-purple-700 dark:text-purple-300 font-semibold px-1.5 py-0.2 rounded">
                              Personalizado
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-1">
                          {t.isCustom && (
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                deleteCustomTheme(t.id);
                              }}
                              className="p-1 text-slate-400 hover:text-red-600 rounded hover:bg-red-50 dark:hover:bg-red-950/30"
                              title="Excluir tema personalizado"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          )}
                          {isActive && (
                            <div className="w-5 h-5 rounded-full bg-amber-500 text-slate-950 flex items-center justify-center shadow-xs">
                              <Check className="w-3 h-3 stroke-[3]" />
                            </div>
                          )}
                        </div>
                      </div>

                      {/* Color Palette Swatches */}
                      <div className="flex items-center gap-1.5 h-6 rounded-lg overflow-hidden bg-slate-100 dark:bg-slate-800 p-1 border border-slate-200/80 dark:border-slate-700">
                        {t.palette.map((color, idx) => (
                          <div
                            key={idx}
                            style={{ backgroundColor: color }}
                            className="flex-1 h-full rounded-sm shadow-2xs hover:scale-105 transition-transform"
                            title={color}
                          />
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {activeTab === 'custom' && (
            <form onSubmit={handleSaveCustom} className="space-y-4">
              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                  Nome da Paleta / Tema
                </label>
                <input
                  type="text"
                  value={customName}
                  onChange={(e) => setCustomName(e.target.value)}
                  placeholder="Ex: Minha Empresa - Cores Oficiais"
                  className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-xs font-medium text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-amber-500"
                  required
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                  Cores da Paleta (utilizadas sequencialmente em gráficos e séries)
                </label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
                  {customColors.map((col, idx) => (
                    <div
                      key={idx}
                      className="flex items-center gap-2 p-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800"
                    >
                      <input
                        type="color"
                        value={col}
                        onChange={(e) => handleUpdateColor(idx, e.target.value)}
                        className="w-8 h-8 rounded border-none cursor-pointer bg-transparent"
                      />
                      <input
                        type="text"
                        value={col}
                        onChange={(e) => handleUpdateColor(idx, e.target.value)}
                        className="w-20 font-mono text-xs uppercase bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded px-1.5 py-1 text-slate-700 dark:text-slate-200"
                      />
                      {customColors.length > 2 && (
                        <button
                          type="button"
                          onClick={() => handleRemoveColor(idx)}
                          className="text-slate-400 hover:text-red-600 p-1 rounded"
                          title="Remover cor"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  ))}
                </div>

                {customColors.length < 10 && (
                  <div className="flex items-center gap-2 mt-3 pt-3 border-t border-slate-100 dark:border-slate-800">
                    <input
                      type="color"
                      value={newColorInput}
                      onChange={(e) => setNewColorInput(e.target.value)}
                      className="w-7 h-7 rounded border-none cursor-pointer bg-transparent"
                    />
                    <span className="text-xs font-mono text-slate-600 dark:text-slate-400">{newColorInput}</span>
                    <button
                      type="button"
                      onClick={handleAddColor}
                      className="flex items-center gap-1 px-3 py-1 rounded bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 text-xs font-semibold transition-colors"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>Adicionar Mais Uma Cor</span>
                    </button>
                  </div>
                )}
              </div>

              {/* Preview Bar */}
              <div className="p-3 rounded-xl bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-700 space-y-1.5">
                <span className="text-[11px] font-semibold text-slate-600 dark:text-slate-400">Prévia da Sequência Visual:</span>
                <div className="flex h-7 rounded-lg overflow-hidden border border-slate-300 dark:border-slate-700">
                  {customColors.map((c, i) => (
                    <div
                      key={i}
                      style={{ backgroundColor: c }}
                      className="flex-1 flex items-center justify-center text-[10px] text-white/90 font-mono font-bold drop-shadow-xs"
                    >
                      {i + 1}
                    </div>
                  ))}
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setActiveTab('presets')}
                  className="px-4 py-2 rounded-lg border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 font-medium text-xs transition-colors"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-600 text-slate-950 font-bold text-xs shadow-xs transition-colors flex items-center gap-1.5"
                >
                  <Sparkles className="w-3.5 h-3.5" />
                  <span>Salvar e Aplicar Paleta</span>
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Footer */}
        <div className="px-5 py-3 border-t border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
          <span>Configurações salvas automaticamente no navegador.</span>
          <button
            onClick={() => setThemeModalOpen(false)}
            className="px-4 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-900 dark:bg-amber-500 dark:hover:bg-amber-600 dark:text-slate-950 text-white font-semibold text-xs transition-colors"
          >
            Concluir
          </button>
        </div>
      </div>
    </div>
  );
};
