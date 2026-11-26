import { describe, expect, it } from 'vitest';
import { migrateDrillConfigs } from '../drill';
import { migrateProjectState } from '../../migrations';
import { resolveMatrixRowLevels } from '../../drill';
import type { Page, Visual } from '../../../types/bi';

const vis = (over: Partial<Visual>): Visual => ({
  id: 'v',
  type: 'matrix',
  title: 'M',
  x: 0,
  y: 0,
  w: 300,
  h: 200,
  dataConfig: { tableId: 't' },
  formatConfig: { showTitle: true },
  ...over,
});
const page = (visuals: Visual[]): Page => ({ id: 'p', name: 'P', order: 0, visuals });

describe('migrateDrillConfigs', () => {
  it('matriz antiga de 2 níveis (xAxisField + drillFields[0]) vira matrixRowFields com os mesmos 2 níveis', () => {
    const old = vis({ dataConfig: { tableId: 't', xAxisField: 'Categoria', drillFields: ['Produto'], yAxisFields: ['Valor'] } });
    const [p] = migrateDrillConfigs([page([old])]);
    const dc = p.visuals[0].dataConfig;
    expect(dc.matrixRowFields).toEqual(['Categoria', 'Produto']);
    // campos antigos preservados (exportação/outros leitores continuam funcionando)
    expect(dc.xAxisField).toBe('Categoria');
    expect(dc.drillFields).toEqual(['Produto']);
    expect(dc.yAxisFields).toEqual(['Valor']);
    // a matriz antiga só usava drillFields[0]: a migrada também tem exatamente 2 níveis
    const three = vis({ dataConfig: { tableId: 't', xAxisField: 'A', drillFields: ['B', 'C'] } });
    expect(migrateDrillConfigs([page([three])])[0].visuals[0].dataConfig.matrixRowFields).toEqual(['A', 'B']);
  });

  it('idempotente e não toca no que não precisa', () => {
    const flat = vis({ dataConfig: { tableId: 't', xAxisField: 'Categoria' } });
    const already = vis({ dataConfig: { tableId: 't', xAxisField: 'A', matrixRowFields: ['A', 'B', 'C'], drillFields: ['B'] } });
    const chart = vis({ type: 'column', dataConfig: { tableId: 't', xAxisField: 'Data', drillFields: ['X'] } });
    const pages = [page([flat, already, chart])];
    const out = migrateDrillConfigs(pages);
    expect(out[0].visuals[0]).toBe(flat);
    expect(out[0].visuals[1]).toBe(already);
    expect(out[0].visuals[2]).toBe(chart);
    expect(migrateDrillConfigs(migrateDrillConfigs(pages))).toEqual(out);
  });

  it('não liga a hierarquia de data automática em visuais antigos (não grava drillMode)', () => {
    const chart = vis({ type: 'line', dataConfig: { tableId: 't', xAxisField: 'Data' } });
    const out = migrateDrillConfigs([page([chart])]);
    expect(out[0].visuals[0].dataConfig.drillMode).toBeUndefined();
  });

  it('entrada inválida não quebra', () => {
    expect(migrateDrillConfigs(undefined as unknown as Page[])).toBeUndefined();
    expect(migrateDrillConfigs([{ id: 'p', name: 'P', order: 0 } as unknown as Page])).toHaveLength(1);
  });

  it('migrateProjectState aplica a migração: a matriz antiga abre com os mesmos 2 níveis', () => {
    const state = migrateProjectState({
      tables: [{ id: 't', name: 'T', columns: [{ name: 'Categoria', dataType: 'text' }, { name: 'Produto', dataType: 'text' }], rows: [] }],
      pages: [page([vis({ dataConfig: { tableId: 't', xAxisField: 'Categoria', drillFields: ['Produto'] } })])],
    });
    const dc = state.pages[0].visuals[0].dataConfig;
    expect(dc.matrixRowFields).toEqual(['Categoria', 'Produto']);
    expect(resolveMatrixRowLevels(dc, () => 'text').map(l => l.field)).toEqual(['Categoria', 'Produto']);
  });
});
