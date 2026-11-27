/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { SupportedLanguage } from '../../i18n';

/** Views com escopo próprio de atalhos (v0.11: Relatório, Dados, Modelo, Power Query, DAX Query). */
export type ShortcutScope = 'report' | 'data' | 'model' | 'query' | 'daxquery';

export type ShortcutCategory =
  | 'file'
  | 'edit'
  | 'canvas'
  | 'navigation'
  | 'view'
  | 'daxquery'
  | 'help';

/**
 * Uma combinação de teclas. `mod` vira Ctrl no Windows/Linux e Cmd no macOS
 * (ver `isModKey` em registry.ts). `key` é comparado contra `KeyboardEvent.key`
 * (case-insensitive para letras).
 */
export interface ShortcutCombo {
  key: string;
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export interface ShortcutDef {
  /** Id estável, único no registro (usado para detectar conflitos e em testes). */
  id: string;
  category: ShortcutCategory;
  /** Uma ou mais combinações equivalentes (ex.: PDF = Ctrl+P OU Ctrl+E). */
  combos: ShortcutCombo[];
  /** Nome exibido no diálogo de atalhos e nos tooltips, por idioma. */
  label: Record<SupportedLanguage, string>;
  /**
   * Views onde o atalho se aplica. `undefined` = global (qualquer view).
   * As views reconhecidas são as 5 com escopo próprio; fora delas (ex.:
   * Governança, Dashboard) só os atalhos globais funcionam.
   */
  scope?: ShortcutScope[];
  /**
   * Quando true, o atalho dispara mesmo com o foco num input/textarea/
   * contenteditable/editor de fórmulas (ex.: Ctrl+S). Default: false.
   */
  allowInEditable?: boolean;
  /**
   * Quando true, este item existe só para documentação (tooltip/diálogo de
   * atalhos) — o atalho já é tratado por um listener próprio em outro lugar
   * do código (ex.: Ctrl+Z no canvas, Ctrl+Enter na DAX Query View) e o hook
   * global NUNCA deve re-despachá-lo, para não duplicar nem quebrar nada.
   */
  handledElsewhere?: boolean;
  /**
   * Ação disparada pelo hook global quando `handledElsewhere` é falso.
   * Recebe o evento de teclado já validado (escopo e foco já checados).
   */
  action?: (e: KeyboardEvent) => void;
}

export interface ShortcutConflict {
  comboKey: string;
  ids: string[];
}
