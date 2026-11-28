/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Pub/sub minúsculo para toasts disparados por atalhos de teclado (ex.:
 * "Projeto salvo" no Ctrl+S). Fica fora do Zustand de propósito: é um
 * detalhe de UI efêmero, não estado de projeto, e não precisa participar
 * do undo/redo nem ser persistido.
 */

export type ShortcutToastKind = 'success' | 'error' | 'info';

export interface ShortcutToastMessage {
  id: number;
  text: string;
  kind: ShortcutToastKind;
}

type Listener = (msg: ShortcutToastMessage) => void;

let nextId = 1;
const listeners = new Set<Listener>();

export function showShortcutToast(text: string, kind: ShortcutToastKind = 'success'): void {
  const msg: ShortcutToastMessage = { id: nextId++, text, kind };
  listeners.forEach(fn => fn(msg));
}

export function subscribeShortcutToast(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
