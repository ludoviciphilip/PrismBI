import { describe, expect, it } from 'vitest';
import { migrateConditionalFormats } from '../condfmt';
import type { VisualFormatConfig } from '../../../types/bi';

const base: VisualFormatConfig = {
  showTitle: true,
  showDataLabels: false,
  showLegend: true,
};

describe('migrateConditionalFormats', () => {
  it('sem campo legado nem novo -> retorna o objeto como está', () => {
    expect(migrateConditionalFormats(base)).toBe(base);
  });

  it('já no formato novo -> não toca (idempotente)', () => {
    const cfg: VisualFormatConfig = {
      ...base,
      conditionalFormats: [{ id: '1', target: 'background', applyTo: 'Vendas', mode: 'rules', rules: [] }],
    };
    expect(migrateConditionalFormats(cfg)).toBe(cfg);
  });

  it('migra o campo legado para um ConditionalFormat de background/rules', () => {
    const cfg: VisualFormatConfig = {
      ...base,
      conditionalFormatting: {
        field: 'Vendas',
        rules: [
          { operator: '>', value: 1000, color: '#16a34a' },
          { operator: '<', value: 0, color: '#dc2626' },
        ],
      },
    };
    const migrated = migrateConditionalFormats(cfg);
    expect(migrated!.conditionalFormats).toHaveLength(1);
    const cf = migrated!.conditionalFormats![0];
    expect(cf.target).toBe('background');
    expect(cf.mode).toBe('rules');
    expect(cf.applyTo).toBe('Vendas');
    expect(cf.rules).toEqual([
      { operator: '>', value: 1000, color: '#16a34a', valueKind: 'number' },
      { operator: '<', value: 0, color: '#dc2626', valueKind: 'number' },
    ]);
    // legado preservado (compat com leituras antigas)
    expect(migrated!.conditionalFormatting).toEqual(cfg.conditionalFormatting);
  });

  it('legado sem regras ou sem campo -> não migra', () => {
    const noRules: VisualFormatConfig = { ...base, conditionalFormatting: { field: 'Vendas', rules: [] } };
    expect(migrateConditionalFormats(noRules)!.conditionalFormats).toBeUndefined();

    const noField: VisualFormatConfig = { ...base, conditionalFormatting: { field: '', rules: [{ operator: '>', value: 0, color: '#000' }] } };
    expect(migrateConditionalFormats(noField)!.conditionalFormats).toBeUndefined();
  });

  it('entrada nula/indefinida passa direto', () => {
    expect(migrateConditionalFormats(undefined)).toBeUndefined();
    expect(migrateConditionalFormats(null)).toBeNull();
  });
});
