/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  nextBookmarkOrder,
  sortBookmarks,
  createBookmarkEntry,
  renameBookmarkEntry,
  deleteBookmarkEntry,
  duplicateBookmarkEntry,
  reorderBookmarkEntry,
  setBookmarkGroup,
  ungroupBookmarkEntries,
  updateBookmarkSettings,
} from '../bookmarks';
import { Bookmark } from '../../types/bi';

function bm(id: string, order: number, over: Partial<Bookmark> = {}): Bookmark {
  return { id, name: id, activePageId: 'p1', filters: [], order, ...over };
}

describe('operações puras da lista de indicadores', () => {
  it('nextBookmarkOrder encadeia a partir do maior order existente', () => {
    expect(nextBookmarkOrder([])).toBe(0);
    expect(nextBookmarkOrder([bm('a', 0), bm('b', 3)])).toBe(4);
  });

  it('createBookmarkEntry cria com id único, nome e próxima ordem', () => {
    const existing = [bm('a', 0)];
    const entry = createBookmarkEntry('Novo', { activePageId: 'p1', filters: [] }, existing);
    expect(entry.name).toBe('Novo');
    expect(entry.order).toBe(1);
    expect(entry.id).toMatch(/^bm_/);
  });

  it('renameBookmarkEntry/deleteBookmarkEntry', () => {
    const list = [bm('a', 0), bm('b', 1)];
    expect(renameBookmarkEntry(list, 'a', 'Novo Nome').find(b => b.id === 'a')?.name).toBe('Novo Nome');
    expect(deleteBookmarkEntry(list, 'a').map(b => b.id)).toEqual(['b']);
  });

  it('duplicateBookmarkEntry insere uma cópia logo após o original e reflui a ordem', () => {
    const list = [bm('a', 0), bm('b', 1)];
    const next = duplicateBookmarkEntry(list, 'a');
    expect(next.map(b => b.id)).toEqual(['a', expect.stringMatching(/^bm_/), 'b']);
    expect(next[1].name).toBe('a - Cópia');
    expect(next.map(b => b.order)).toEqual([0, 1, 2]);
  });

  describe('reorderBookmarkEntry (arrastar para reordenar)', () => {
    it('move um item para depois de outro', () => {
      const list = [bm('a', 0), bm('b', 1), bm('c', 2)];
      const next = sortBookmarks(reorderBookmarkEntry(list, 'a', 'c'));
      expect(next.map(b => b.id)).toEqual(['b', 'c', 'a']);
      expect(next.map(b => b.order)).toEqual([0, 1, 2]);
    });

    it('move um item para antes de outro', () => {
      const list = [bm('a', 0), bm('b', 1), bm('c', 2)];
      const next = sortBookmarks(reorderBookmarkEntry(list, 'c', 'a'));
      expect(next.map(b => b.id)).toEqual(['c', 'a', 'b']);
    });

    it('é no-op quando id === overId', () => {
      const list = [bm('a', 0), bm('b', 1)];
      expect(reorderBookmarkEntry(list, 'a', 'a')).toBe(list);
    });
  });

  describe('grupos', () => {
    it('setBookmarkGroup agrupa os ids informados', () => {
      const list = [bm('a', 0), bm('b', 1), bm('c', 2)];
      const next = setBookmarkGroup(list, ['a', 'b'], 'Vendas');
      expect(next.find(b => b.id === 'a')?.groupId).toBe('Vendas');
      expect(next.find(b => b.id === 'b')?.groupId).toBe('Vendas');
      expect(next.find(b => b.id === 'c')?.groupId).toBeUndefined();
    });

    it('ungroupBookmarkEntries remove groupId dos ids informados', () => {
      const list = [bm('a', 0, { groupId: 'Vendas' }), bm('b', 1, { groupId: 'Vendas' })];
      const next = ungroupBookmarkEntries(list, ['a']);
      expect(next.find(b => b.id === 'a')?.groupId).toBeUndefined();
      expect(next.find(b => b.id === 'b')?.groupId).toBe('Vendas');
    });
  });

  it('updateBookmarkSettings só altera capture/scope, preservando o resto', () => {
    const list = [bm('a', 0, { capture: { data: true, display: true, currentPage: true } })];
    const next = updateBookmarkSettings(list, 'a', { scope: ['vA'] });
    expect(next[0].scope).toEqual(['vA']);
    expect(next[0].capture).toEqual({ data: true, display: true, currentPage: true });
  });
});
