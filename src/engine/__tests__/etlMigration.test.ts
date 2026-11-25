import { describe, it, expect, vi } from 'vitest';
import { migratePowerQueryStep, migrateAppliedSteps } from '../migrations';
import { PowerQueryStep, Table } from '../../types/bi';
import { EtlQuery } from '../../types/etl';

function step(partial: Partial<PowerQueryStep>): PowerQueryStep {
  return { id: partial.id || 's1', name: 'x', type: partial.type as any, params: partial.params || {}, description: partial.description };
}

describe('migratePowerQueryStep', () => {
  it('maps rename to renameColumns', () => {
    const s = step({ type: 'rename', params: { oldName: 'A', newName: 'B' } });
    expect(migratePowerQueryStep(s)).toEqual({ id: 's1', name: 'rename', op: 'renameColumns', renames: [['A', 'B']] });
  });

  it('maps changeType to changeType with mapped EtlType', () => {
    const s = step({ type: 'changeType', params: { columnName: 'Valor', newType: 'currency' } });
    expect(migratePowerQueryStep(s)).toEqual({
      id: 's1',
      name: 'changeType',
      op: 'changeType',
      columns: [{ column: 'Valor', type: 'currency' }],
    });
  });

  it('maps filter equals to filterRows condition eq', () => {
    const s = step({ type: 'filter', params: { column: 'Regiao', operator: 'equals', value: 'SP' } });
    expect(migratePowerQueryStep(s)).toEqual({
      id: 's1',
      name: 'filter',
      op: 'filterRows',
      condition: { column: 'Regiao', op: 'eq', value: 'SP' },
    });
  });

  it('maps removeColumns', () => {
    const s = step({ type: 'removeColumns', params: { columnNames: ['A', 'B'] } });
    expect(migratePowerQueryStep(s)).toMatchObject({ op: 'removeColumns', columns: ['A', 'B'] });
  });

  it('maps removeOtherColumns to selectColumns', () => {
    const s = step({ type: 'removeOtherColumns', params: { keepColumns: ['A'] } });
    expect(migratePowerQueryStep(s)).toMatchObject({ op: 'selectColumns', columns: ['A'] });
  });

  it('maps useFirstRowAsHeaders to promoteHeaders', () => {
    const s = step({ type: 'useFirstRowAsHeaders', params: {} });
    expect(migratePowerQueryStep(s)).toMatchObject({ op: 'promoteHeaders' });
  });

  it('maps sort, deduplicate, fillDown, fillUp', () => {
    expect(migratePowerQueryStep(step({ type: 'sort', params: { column: 'A', order: 'desc' } }))).toMatchObject({
      op: 'sort',
      by: [{ column: 'A', descending: true }],
    });
    expect(migratePowerQueryStep(step({ type: 'deduplicate', params: {} }))).toMatchObject({ op: 'removeDuplicates' });
    expect(migratePowerQueryStep(step({ type: 'fillDown', params: { column: 'A' } }))).toMatchObject({ op: 'fillDown', columns: ['A'] });
    expect(migratePowerQueryStep(step({ type: 'fillUp', params: { column: 'A' } }))).toMatchObject({ op: 'fillUp', columns: ['A'] });
  });

  it('maps addIndexColumn', () => {
    expect(migratePowerQueryStep(step({ type: 'addIndexColumn', params: { newColumnName: 'Idx', startIndex: 1 } }))).toMatchObject({
      op: 'addIndexColumn',
      newColumn: 'Idx',
      start: 1,
      step: 1,
    });
  });

  it('maps textTransform uppercase to textTransform upper', () => {
    expect(migratePowerQueryStep(step({ type: 'textTransform', params: { column: 'A', mode: 'uppercase' } }))).toMatchObject({
      op: 'textTransform',
      columns: ['A'],
      fn: 'upper',
    });
  });

  it('maps addCustomColumn wrapping expression with each', () => {
    expect(
      migratePowerQueryStep(step({ type: 'addCustomColumn', params: { newColumnName: 'C', expression: '[A] + [B]' } }))
    ).toMatchObject({ op: 'addCustomColumn', newColumn: 'C', expression: 'each [A] + [B]' });
  });

  it('drops unmappable steps with console.warn', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const s = step({ type: 'groupBy', params: {} });
    expect(migratePowerQueryStep(s)).toBeNull();
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it('source step has no equivalent and returns null silently', () => {
    expect(migratePowerQueryStep(step({ type: 'source', params: {} }))).toBeNull();
  });
});

describe('migrateAppliedSteps', () => {
  const tables: Table[] = [
    {
      id: 't1',
      name: 'Vendas',
      originalName: 'Vendas',
      columns: [],
      rows: [],
      sourceConfig: { type: 'csv', filePath: '/dados/vendas.csv' },
    },
    {
      id: 't2',
      name: 'Metas',
      originalName: 'Metas',
      columns: [{ id: 'c1', name: 'B', dataType: 'number' } as any],
      rows: [{ B: 10 }, { B: null }],
    },
  ];

  it('converts legacy PowerQueryStep[] map into EtlQuery map (file source replays the steps)', () => {
    const legacy = {
      t1: [step({ type: 'rename', params: { oldName: 'A', newName: 'B' } }), step({ id: 's2', type: 'groupBy', params: {} })],
    };
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const result = migrateAppliedSteps(legacy, tables);
    warnSpy.mockRestore();
    expect(result.t1.name).toBe('Vendas');
    expect(result.t1.source).toEqual({ kind: 'csv', path: '/dados/vendas.csv' });
    expect(result.t1.steps).toHaveLength(1);
    expect(result.t1.steps[0]).toMatchObject({ op: 'renameColumns' });
  });

  it('without a reloadable source, snapshots current rows and drops already-applied legacy steps', () => {
    const legacy = { t2: [step({ type: 'rename', params: { oldName: 'A', newName: 'B' } })] };
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const result = migrateAppliedSteps(legacy, tables);
    warnSpy.mockRestore();
    expect(result.t2.source).toEqual({ kind: 'inline', columns: ['B'], rows: [[10], [null]] });
    expect(result.t2.steps).toEqual([]);
  });

  it('passes through projects already in EtlQuery format', () => {
    const already: Record<string, EtlQuery> = {
      t1: { name: 'Vendas', source: { kind: 'table' }, steps: [{ id: 'a', name: 'x', op: 'promoteHeaders' }] },
    };
    const result = migrateAppliedSteps(already, tables);
    expect(result).toEqual(already);
  });

  it('returns empty object for undefined/invalid input', () => {
    expect(migrateAppliedSteps(undefined, tables)).toEqual({});
    expect(migrateAppliedSteps(null as any, tables)).toEqual({});
  });
});
