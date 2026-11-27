/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Pub/sub minúsculo para pontes entre o hook global de atalhos e UI local
 * que só existe dentro de um componente (ex.: o modal "Novo Projeto" e o
 * <input type="file"> de importação de .insight, ambos com estado local
 * em Ribbon.tsx). Evita duplicar essa UI ou subir estado só por causa do
 * atalho de teclado.
 */

export type ShortcutActionName = 'open-project-file' | 'new-project-modal';

type Listener = (action: ShortcutActionName) => void;

const listeners = new Set<Listener>();

export function emitShortcutAction(action: ShortcutActionName): void {
  listeners.forEach(fn => fn(action));
}

export function subscribeShortcutAction(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
