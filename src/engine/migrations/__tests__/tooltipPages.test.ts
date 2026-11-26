import { describe, expect, it } from 'vitest';
import { migrateTooltipPages } from '../tooltipPages';
import type { Page } from '../../../types/bi';

describe('migrateTooltipPages', () => {
  it('projeto antigo sem tooltipPage/tooltip -> passa direto sem erro', () => {
    const pages: Page[] = [{ id: 'p1', name: 'P1', order: 1, visuals: [{ id: 'v1', type: 'column', title: 'T', x: 0, y: 0, w: 1, h: 1, dataConfig: { tableId: 't' }, formatConfig: { showTitle: true } }] }];
    const migrated = migrateTooltipPages(pages);
    expect(migrated).toEqual(pages);
  });

  it('tooltipPage.fields ausente ou malformado -> normaliza para array vazio', () => {
    const pages: Page[] = [{ id: 'p1', name: 'P1', order: 1, visuals: [], tooltipPage: { enabled: true } as any }];
    const migrated = migrateTooltipPages(pages);
    expect(migrated[0].tooltipPage?.fields).toEqual([]);
  });

  it('tooltip de visual tipo "page" sem pageId -> ganha pageId "auto"', () => {
    const pages: Page[] = [{
      id: 'p1', name: 'P1', order: 1,
      visuals: [{
        id: 'v1', type: 'column', title: 'T', x: 0, y: 0, w: 1, h: 1,
        dataConfig: { tableId: 't' },
        formatConfig: { showTitle: true, tooltip: { type: 'page' } },
      }],
    }];
    const migrated = migrateTooltipPages(pages);
    expect(migrated[0].visuals[0].formatConfig.tooltip).toEqual({ type: 'page', pageId: 'auto' });
  });

  it('já normalizado -> idempotente (mesma referência de página)', () => {
    const page: Page = { id: 'p1', name: 'P1', order: 1, visuals: [], tooltipPage: { enabled: true, fields: ['Regiao'] } };
    const migrated = migrateTooltipPages([page]);
    expect(migrated[0]).toBe(page);
  });

  it('entrada não-array passa direto', () => {
    expect(migrateTooltipPages(undefined as any)).toBeUndefined();
  });
});
