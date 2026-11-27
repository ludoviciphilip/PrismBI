/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  SHORTCUTS,
  findShortcutConflicts,
  matchesCombo,
  formatCombo,
  formatCombos,
  comboKey,
  getShortcutHint,
} from '../registry';

describe('Keyboard shortcuts registry (v0.11)', () => {
  it('has no id duplicated', () => {
    const ids = SHORTCUTS.map(s => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('has a translation (pt-BR, en-US, es-ES) for every shortcut label', () => {
    for (const def of SHORTCUTS) {
      expect(def.label['pt-BR']).toBeTruthy();
      expect(def.label['en-US']).toBeTruthy();
      expect(def.label['es-ES']).toBeTruthy();
    }
  });

  it('never has two different shortcuts claiming the same combo in overlapping scopes', () => {
    const conflicts = findShortcutConflicts();
    expect(conflicts).toEqual([]);
  });

  it('detects a conflict when two defs share a combo in the same scope (synthetic)', () => {
    const synthetic = [
      {
        id: 'a',
        category: 'file' as const,
        combos: [{ key: 's', mod: true }],
        label: { 'pt-BR': 'A', 'en-US': 'A', 'es-ES': 'A' },
      },
      {
        id: 'b',
        category: 'file' as const,
        combos: [{ key: 's', mod: true }],
        label: { 'pt-BR': 'B', 'en-US': 'B', 'es-ES': 'B' },
      },
    ];
    const conflicts = findShortcutConflicts(synthetic as any);
    expect(conflicts).toHaveLength(1);
    expect(conflicts[0].ids.sort()).toEqual(['a', 'b']);
  });

  it('does NOT flag a conflict when two scoped shortcuts never share a view', () => {
    const synthetic = [
      {
        id: 'a',
        category: 'canvas' as const,
        combos: [{ key: 'c', mod: true }],
        label: { 'pt-BR': 'A', 'en-US': 'A', 'es-ES': 'A' },
        scope: ['report'] as const,
      },
      {
        id: 'b',
        category: 'canvas' as const,
        combos: [{ key: 'c', mod: true }],
        label: { 'pt-BR': 'B', 'en-US': 'B', 'es-ES': 'B' },
        scope: ['model'] as const,
      },
    ];
    const conflicts = findShortcutConflicts(synthetic as any);
    expect(conflicts).toEqual([]);
  });

  it('comboKey is stable for equivalent combos', () => {
    expect(comboKey({ key: 's', mod: true })).toBe(comboKey({ key: 's', mod: true }));
    expect(comboKey({ key: 's', mod: true })).not.toBe(comboKey({ key: 's', mod: true, shift: true }));
  });

  it('formatCombo renders Ctrl on Windows/Linux and Cmd on macOS', () => {
    expect(formatCombo({ key: 's', mod: true }, false)).toBe('Ctrl+S');
    expect(formatCombo({ key: 's', mod: true }, true)).toBe('Cmd+S');
    expect(formatCombo({ key: 's', mod: true, shift: true }, false)).toBe('Ctrl+Shift+S');
  });

  it('formatCombos joins alternative combos (ex.: Ctrl+P ou Ctrl+E)', () => {
    const def = SHORTCUTS.find(s => s.id === 'file.exportPdf')!;
    expect(formatCombos(def.combos, false)).toBe('Ctrl+P ou Ctrl+E');
  });

  it('getShortcutHint returns the primary combo label for a known id', () => {
    expect(getShortcutHint('file.save', false)).toBe('Ctrl+S');
    expect(getShortcutHint('file.saveAs', false)).toBe('Ctrl+Shift+S');
  });

  it('matchesCombo matches Ctrl+S on Windows/Linux and Cmd+S on macOS without cross-firing', () => {
    const combo = { key: 's', mod: true };
    const ctrlS = { key: 's', ctrlKey: true, metaKey: false, shiftKey: false, altKey: false } as KeyboardEvent;
    const cmdS = { key: 's', ctrlKey: false, metaKey: true, shiftKey: false, altKey: false } as KeyboardEvent;

    expect(matchesCombo(ctrlS, combo, false)).toBe(true);
    expect(matchesCombo(cmdS, combo, false)).toBe(false);
    expect(matchesCombo(ctrlS, combo, true)).toBe(false);
    expect(matchesCombo(cmdS, combo, true)).toBe(true);
  });

  it('matchesCombo requires exact shift/alt state (no accidental firing)', () => {
    const combo = { key: 's', mod: true };
    const ctrlShiftS = { key: 's', ctrlKey: true, metaKey: false, shiftKey: true, altKey: false } as KeyboardEvent;
    expect(matchesCombo(ctrlShiftS, combo, false)).toBe(false);
  });

  it('every scoped shortcut only targets the 5 recognized views', () => {
    const valid = new Set(['report', 'data', 'model', 'query', 'daxquery']);
    for (const def of SHORTCUTS) {
      if (!def.scope) continue;
      for (const s of def.scope) {
        expect(valid.has(s)).toBe(true);
      }
    }
  });
});
