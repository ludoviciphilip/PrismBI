/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { useBiStore } from '../../store/useBiStore';
import { exportProjectToFile } from '../../engine/storage';
import { getTranslation } from '../../i18n';
import { showShortcutToast } from './toastBus';
import { emitShortcutAction } from './actionBus';
import { ShortcutCombo, ShortcutDef } from './types';

export * from './types';
export { showShortcutToast, subscribeShortcutToast } from './toastBus';
export { emitShortcutAction, subscribeShortcutAction } from './actionBus';

/** true em macOS (Cmd no lugar de Ctrl) — paridade com apps de escritório nativos. */
export function isMacPlatform(): boolean {
  if (typeof navigator === 'undefined') return false;
  const platform = (navigator as any).userAgentData?.platform || navigator.platform || navigator.userAgent || '';
  return /Mac|iPhone|iPad|iPod/i.test(platform);
}

/** Normaliza uma combinação + plataforma numa chave estável p/ comparação e exibição. */
export function comboKey(combo: ShortcutCombo): string {
  const parts: string[] = [];
  if (combo.mod) parts.push('mod');
  if (combo.shift) parts.push('shift');
  if (combo.alt) parts.push('alt');
  parts.push(combo.key.toLowerCase());
  return parts.join('+');
}

/** Rótulo visível da combinação (ex.: "Ctrl+S" ou "Cmd+S" no Mac). */
export function formatCombo(combo: ShortcutCombo, isMac = isMacPlatform()): string {
  const parts: string[] = [];
  if (combo.mod) parts.push(isMac ? 'Cmd' : 'Ctrl');
  if (combo.shift) parts.push('Shift');
  if (combo.alt) parts.push(isMac ? 'Option' : 'Alt');
  const keyLabels: Record<string, string> = {
    arrowleft: '←',
    arrowright: '→',
    arrowup: '↑',
    arrowdown: '↓',
    pageup: 'PageUp',
    pagedown: 'PageDown',
    delete: 'Delete',
    enter: 'Enter',
    escape: 'Esc',
    '/': '/',
    '?': '?',
  };
  const lowerKey = combo.key.toLowerCase();
  const label = keyLabels[lowerKey] || (combo.key.length === 1 ? combo.key.toUpperCase() : combo.key);
  parts.push(label);
  return parts.join('+');
}

/** Rótulo de uma lista de combos equivalentes, ex.: "Ctrl+P ou Ctrl+E". */
export function formatCombos(combos: ShortcutCombo[], isMac = isMacPlatform()): string {
  return combos.map(c => formatCombo(c, isMac)).join(' ou ');
}

export function matchesCombo(e: KeyboardEvent, combo: ShortcutCombo, isMac = isMacPlatform()): boolean {
  const modPressed = isMac ? e.metaKey : e.ctrlKey;
  const otherModPressed = isMac ? e.ctrlKey : e.metaKey;
  if (otherModPressed) return false; // nunca interpreta Ctrl+Cmd misturado
  if (!!combo.mod !== modPressed) return false;
  if (!!combo.shift !== e.shiftKey) return false;
  if (!!combo.alt !== e.altKey) return false;

  const key = e.key;
  if (combo.key.length === 1) {
    return key.toLowerCase() === combo.key.toLowerCase();
  }
  return key.toLowerCase() === combo.key.toLowerCase() || key === combo.key;
}

function isEditableTarget(el: EventTarget | null): boolean {
  const htmlEl = el as HTMLElement | null;
  if (!htmlEl || typeof htmlEl.tagName !== 'string') return false;
  const tag = htmlEl.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (htmlEl.isContentEditable) return true;
  // Editor de fórmulas DAX (FormulaEditor) e outros campos marcados explicitamente.
  if (htmlEl.closest?.('[data-formula-editor], [contenteditable="true"]')) return true;
  return false;
}

export function isFocusInEditable(): boolean {
  if (typeof document === 'undefined') return false;
  return isEditableTarget(document.activeElement);
}

async function doSaveProject(): Promise<void> {
  const { t, lang } = currentTranslation();
  await useBiStore.getState().saveProject();
  const failedMarker = 'Falha ao salvar';
  const lastSavedAt = useBiStore.getState().lastSavedAt;
  if (lastSavedAt === failedMarker || lastSavedAt === t.common.saveFailed) {
    showShortcutToast(t.shortcuts.toastSaveFailed, 'error');
  } else {
    showShortcutToast(t.shortcuts.toastSaved, 'success');
  }
}

function doSaveAs(): void {
  const state = useBiStore.getState();
  const projectState = state.getProjectState();
  exportProjectToFile(projectState, `${state.projectName.replace(/\s+/g, '_')}.insight`);
  const { t } = currentTranslation();
  showShortcutToast(t.shortcuts.toastExported, 'success');
}

function currentTranslation() {
  const lang = useBiStore.getState().language;
  return { t: getTranslation(lang), lang };
}

function goToPage(delta: 1 | -1): void {
  const state = useBiStore.getState();
  const pages = state.pages;
  if (!pages.length) return;
  const idx = pages.findIndex(p => p.id === state.activePageId);
  if (idx === -1) return;
  const nextIdx = idx + delta;
  if (nextIdx < 0 || nextIdx >= pages.length) return;
  state.setActivePageId(pages[nextIdx].id);
}

/**
 * Registro central de atalhos (v0.11). Fonte única de verdade para:
 * - o hook global (useGlobalShortcuts) que realmente despacha as ações;
 * - os tooltips da faixa (Ribbon);
 * - o diálogo "Atalhos de teclado" (Ctrl+/ ou ?).
 *
 * Itens com `handledElsewhere: true` já têm um listener próprio existente
 * no código (ver referências nos comentários) — ficam aqui só para
 * aparecer no diálogo/tooltip e participar da checagem de conflitos,
 * nunca para serem redisparados pelo hook global.
 */
export const SHORTCUTS: ShortcutDef[] = [
  // --- Arquivo (global) ---
  {
    id: 'file.save',
    category: 'file',
    combos: [{ key: 's', mod: true }],
    label: { 'pt-BR': 'Salvar', 'en-US': 'Save', 'es-ES': 'Guardar' },
    allowInEditable: true,
    action: () => {
      void doSaveProject();
    },
  },
  {
    id: 'file.saveAs',
    category: 'file',
    combos: [{ key: 's', mod: true, shift: true }],
    label: { 'pt-BR': 'Salvar como', 'en-US': 'Save As', 'es-ES': 'Guardar como' },
    allowInEditable: true,
    action: () => doSaveAs(),
  },
  {
    id: 'file.open',
    category: 'file',
    combos: [{ key: 'o', mod: true }],
    label: { 'pt-BR': 'Abrir projeto', 'en-US': 'Open project', 'es-ES': 'Abrir proyecto' },
    allowInEditable: true,
    action: () => emitShortcutAction('open-project-file'),
  },
  {
    id: 'file.new',
    category: 'file',
    combos: [{ key: 'n', mod: true }],
    label: { 'pt-BR': 'Novo projeto', 'en-US': 'New project', 'es-ES': 'Nuevo proyecto' },
    allowInEditable: true,
    action: () => emitShortcutAction('new-project-modal'),
  },
  {
    id: 'file.exportPdf',
    category: 'file',
    combos: [{ key: 'p', mod: true }, { key: 'e', mod: true }],
    label: { 'pt-BR': 'Exportar PDF', 'en-US': 'Export to PDF', 'es-ES': 'Exportar a PDF' },
    allowInEditable: true,
    action: () => window.print(),
  },
  {
    id: 'data.refresh',
    category: 'file',
    combos: [{ key: 'F5' }],
    label: { 'pt-BR': 'Atualizar dados', 'en-US': 'Refresh data', 'es-ES': 'Actualizar datos' },
    allowInEditable: true,
    action: () => {
      void useBiStore.getState().refreshAllData();
    },
  },

  // --- Canvas do Relatório ---
  {
    id: 'canvas.copy',
    category: 'canvas',
    combos: [{ key: 'c', mod: true }],
    label: { 'pt-BR': 'Copiar visual', 'en-US': 'Copy visual', 'es-ES': 'Copiar visual' },
    scope: ['report'],
    action: () => useBiStore.getState().copySelectedVisuals(),
  },
  {
    id: 'canvas.paste',
    category: 'canvas',
    combos: [{ key: 'v', mod: true }],
    label: { 'pt-BR': 'Colar visual', 'en-US': 'Paste visual', 'es-ES': 'Pegar visual' },
    scope: ['report'],
    action: () => useBiStore.getState().pasteVisuals(),
  },
  {
    id: 'canvas.duplicate',
    category: 'canvas',
    combos: [{ key: 'd', mod: true }],
    label: { 'pt-BR': 'Duplicar visual', 'en-US': 'Duplicate visual', 'es-ES': 'Duplicar visual' },
    scope: ['report'],
    action: () => useBiStore.getState().duplicateSelectedVisuals(),
  },
  {
    id: 'canvas.delete',
    category: 'canvas',
    combos: [{ key: 'Delete' }],
    label: { 'pt-BR': 'Excluir visual', 'en-US': 'Delete visual', 'es-ES': 'Eliminar visual' },
    scope: ['report'],
    // v0.6.0: já tratado em ReportView.tsx (Delete/Backspace) — mantém aqui
    // só para aparecer no diálogo/tooltip sem duplicar o listener.
    handledElsewhere: true,
  },
  {
    id: 'canvas.selectAll',
    category: 'canvas',
    combos: [{ key: 'a', mod: true }],
    label: { 'pt-BR': 'Selecionar todos os visuais', 'en-US': 'Select all visuals', 'es-ES': 'Seleccionar todos los visuales' },
    scope: ['report'],
    action: () => useBiStore.getState().selectAllVisualsOnPage(),
  },

  // --- Navegação de páginas (Relatório) ---
  {
    id: 'navigation.prevPage',
    category: 'navigation',
    combos: [{ key: 'PageUp', mod: true }],
    label: { 'pt-BR': 'Página anterior', 'en-US': 'Previous page', 'es-ES': 'Página anterior' },
    scope: ['report'],
    allowInEditable: true,
    action: () => goToPage(-1),
  },
  {
    id: 'navigation.nextPage',
    category: 'navigation',
    combos: [{ key: 'PageDown', mod: true }],
    label: { 'pt-BR': 'Próxima página', 'en-US': 'Next page', 'es-ES': 'Página siguiente' },
    scope: ['report'],
    allowInEditable: true,
    action: () => goToPage(1),
  },

  // --- Visualização ---
  {
    id: 'view.presentationMode',
    category: 'view',
    combos: [{ key: 'F11' }, { key: 'f', mod: true, shift: true }],
    label: { 'pt-BR': 'Modo Apresentação', 'en-US': 'Presentation mode', 'es-ES': 'Modo presentación' },
    scope: ['report'],
    allowInEditable: true,
    action: () => useBiStore.getState().setIsPresentationMode(true),
  },
  {
    id: 'view.exitPresentation',
    category: 'view',
    combos: [{ key: 'Escape' }],
    label: { 'pt-BR': 'Sair do Modo Apresentação', 'en-US': 'Exit presentation mode', 'es-ES': 'Salir del modo presentación' },
    // App.tsx: já trata Escape para sair do Modo Apresentação, globalmente.
    handledElsewhere: true,
  },
  {
    id: 'canvas.undo',
    category: 'edit',
    combos: [{ key: 'z', mod: true }],
    label: { 'pt-BR': 'Desfazer', 'en-US': 'Undo', 'es-ES': 'Deshacer' },
    scope: ['report'],
    // ReportView.tsx: já trata Ctrl+Z no canvas.
    handledElsewhere: true,
  },
  {
    id: 'canvas.redo',
    category: 'edit',
    combos: [{ key: 'y', mod: true }, { key: 'z', mod: true, shift: true }],
    label: { 'pt-BR': 'Refazer', 'en-US': 'Redo', 'es-ES': 'Rehacer' },
    scope: ['report'],
    // ReportView.tsx: já trata Ctrl+Y / Ctrl+Shift+Z no canvas.
    handledElsewhere: true,
  },
  {
    id: 'view.collapsePanes',
    category: 'view',
    combos: [{ key: 'F1', mod: true }],
    label: { 'pt-BR': 'Recolher/expandir painéis', 'en-US': 'Collapse/expand panes', 'es-ES': 'Contraer/expandir paneles' },
    // App.tsx: já trata Ctrl+F1 globalmente (v0.10.0).
    handledElsewhere: true,
  },
  {
    id: 'daxquery.run',
    category: 'daxquery',
    combos: [{ key: 'Enter', mod: true }],
    label: { 'pt-BR': 'Executar consulta', 'en-US': 'Run query', 'es-ES': 'Ejecutar consulta' },
    scope: ['daxquery'],
    allowInEditable: true,
    // DaxQueryView.tsx: já trata Ctrl+Enter para executar a consulta.
    handledElsewhere: true,
  },

  // --- Ajuda ---
  {
    id: 'help.shortcuts',
    category: 'help',
    combos: [{ key: '/', mod: true }, { key: '?' }],
    label: { 'pt-BR': 'Atalhos de teclado', 'en-US': 'Keyboard shortcuts', 'es-ES': 'Atajos de teclado' },
    action: () => useBiStore.getState().setShortcutsModalOpen(true),
  },
];

export const SHORTCUT_CATEGORY_LABEL: Record<string, Record<string, string>> = {
  file: { 'pt-BR': 'Arquivo', 'en-US': 'File', 'es-ES': 'Archivo' },
  edit: { 'pt-BR': 'Editar', 'en-US': 'Edit', 'es-ES': 'Editar' },
  canvas: { 'pt-BR': 'Canvas do relatório', 'en-US': 'Report canvas', 'es-ES': 'Lienzo del informe' },
  navigation: { 'pt-BR': 'Navegação', 'en-US': 'Navigation', 'es-ES': 'Navegación' },
  view: { 'pt-BR': 'Visualização', 'en-US': 'View', 'es-ES': 'Visualización' },
  daxquery: { 'pt-BR': 'DAX Query View', 'en-US': 'DAX Query View', 'es-ES': 'DAX Query View' },
  help: { 'pt-BR': 'Ajuda', 'en-US': 'Help', 'es-ES': 'Ayuda' },
};

export function getShortcutById(id: string): ShortcutDef | undefined {
  return SHORTCUTS.find(s => s.id === id);
}

/** Rótulo formatado (ex.: "Ctrl+S") do primeiro combo de um atalho — usado nos tooltips da faixa. */
export function getShortcutHint(id: string, isMac = isMacPlatform()): string {
  const def = getShortcutById(id);
  if (!def || !def.combos.length) return '';
  return formatCombo(def.combos[0], isMac);
}

/**
 * Detecta conflitos: duas definições diferentes reivindicando a MESMA
 * combinação de teclas em escopos que se sobrepõem (global sempre se
 * sobrepõe a qualquer escopo; dois escopos só colidem se compartilharem
 * uma view). Usado pelo teste do registro — nunca deve haver conflitos.
 */
export function findShortcutConflicts(defs: ShortcutDef[] = SHORTCUTS): { comboKey: string; ids: string[] }[] {
  const byCombo = new Map<string, { id: string; scope?: string[] }[]>();
  for (const def of defs) {
    for (const combo of def.combos) {
      const key = comboKey(combo);
      const list = byCombo.get(key) || [];
      list.push({ id: def.id, scope: def.scope });
      byCombo.set(key, list);
    }
  }

  const conflicts: { comboKey: string; ids: string[] }[] = [];
  for (const [key, entries] of byCombo.entries()) {
    if (entries.length < 2) continue;
    const clashingIds: string[] = [];
    for (let i = 0; i < entries.length; i++) {
      for (let j = i + 1; j < entries.length; j++) {
        const a = entries[i];
        const b = entries[j];
        const aGlobal = !a.scope;
        const bGlobal = !b.scope;
        const overlap =
          aGlobal || bGlobal || (a.scope || []).some(s => (b.scope || []).includes(s));
        if (overlap) {
          if (!clashingIds.includes(a.id)) clashingIds.push(a.id);
          if (!clashingIds.includes(b.id)) clashingIds.push(b.id);
        }
      }
    }
    if (clashingIds.length > 1) {
      conflicts.push({ comboKey: key, ids: clashingIds });
    }
  }
  return conflicts;
}
