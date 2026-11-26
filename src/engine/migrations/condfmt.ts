/**
 * Migração v0.10 (Frente B): `VisualFormatConfig.conditionalFormatting`
 * (legado, um único campo + lista de `ConditionalFormatRule`, cor de fundo
 * implícita) para `conditionalFormats` (lista de `ConditionalFormat`,
 * suportando gradiente, valor de campo, barras de dados e ícones).
 *
 * Registrada em src/engine/migrations.ts (`migrateVisualFormatConfig`).
 */
import type { ConditionalFormat, VisualFormatConfig } from '../../types/bi';

/**
 * Migra um único `VisualFormatConfig`. Idempotente: se `conditionalFormats`
 * já existir, retorna o objeto como está (projetos já salvos no formato
 * novo não são tocados). Se só o campo legado existir, ele é convertido
 * para um `ConditionalFormat` de `target: 'background'`/`mode: 'rules'` e o
 * legado é preservado (não removido) para não quebrar leituras antigas.
 */
export function migrateConditionalFormats(formatConfig: VisualFormatConfig | undefined | null): VisualFormatConfig | undefined | null {
  if (!formatConfig) return formatConfig;
  if (formatConfig.conditionalFormats) return formatConfig;

  const legacy = formatConfig.conditionalFormatting;
  if (!legacy || !legacy.field || !Array.isArray(legacy.rules) || legacy.rules.length === 0) {
    return formatConfig;
  }

  const migrated: ConditionalFormat = {
    id: `cf_migrated_${legacy.field}`,
    target: 'background',
    applyTo: legacy.field,
    mode: 'rules',
    rules: legacy.rules.map(r => ({
      operator: r.operator,
      value: r.value,
      value2: r.value2,
      color: r.color,
      valueKind: 'number',
    })),
  };

  return {
    ...formatConfig,
    conditionalFormats: [migrated],
  };
}
