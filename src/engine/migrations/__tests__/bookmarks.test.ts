/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import { migrateBookmarks } from '../bookmarks';

describe('migrateBookmarks (projetos antigos continuam abrindo)', () => {
  it('preenche defaults para um indicador legado (só id/name/activePageId/filters)', () => {
    const legacy = [{ id: 'bm_1', name: 'Antigo', activePageId: 'p1', filters: [] }];
    const migrated = migrateBookmarks(legacy);
    expect(migrated).toHaveLength(1);
    expect(migrated[0]).toMatchObject({
      id: 'bm_1',
      name: 'Antigo',
      activePageId: 'p1',
      filters: [],
      order: 0,
      capture: { data: true, display: true, currentPage: true },
      scope: 'all',
    });
    expect(migrated[0].groupId).toBeUndefined();
  });

  it('preserva campos novos já presentes (idempotente)', () => {
    const modern = [
      {
        id: 'bm_2',
        name: 'Novo',
        activePageId: 'p1',
        filters: [],
        order: 5,
        groupId: 'Vendas',
        capture: { data: false, display: true, currentPage: false },
        scope: ['vA'],
      },
    ];
    const migrated = migrateBookmarks(modern);
    expect(migrated[0]).toMatchObject(modern[0]);
  });

  it('lida com entrada inválida (não-array) devolvendo lista vazia', () => {
    expect(migrateBookmarks(undefined)).toEqual([]);
    expect(migrateBookmarks(null)).toEqual([]);
  });
});
