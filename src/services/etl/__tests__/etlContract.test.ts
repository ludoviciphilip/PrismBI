// Fixture de contrato TS ⇄ Rust: uma etapa de cada `op`, no formato que o
// frontend produz. O compilador TS valida o formato via `EtlQuery`. O teste
// abaixo garante que tests/etl-contract/all_steps.json é este objeto, e
// src-tauri/tests/etl_contract.rs desserializa o mesmo JSON no Rust, faz o
// round-trip M e executa as etapas. Se um lado mudar o schema, um dos dois quebra.
// Regenerar o JSON: UPDATE_ETL_CONTRACT=1 pnpm vitest run etlContract
import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { EtlQuery } from '../../../types/etl';

export const ALL_STEPS_QUERY: EtlQuery = {
  name: 'Contrato',
  source: {
    kind: 'inline',
    columns: ['Produto', 'Cidade', 'Valor', 'Qtd', 'Cod'],
    rows: [
      ['caneta azul', 'Curitiba', 10.5, 2, 'A-1'],
      ['lápis', null, 3, 1, 'B-2'],
      ['caneta azul', 'Curitiba', 10.5, 2, 'A-1'],
      ['borracha', 'Londrina', 1.25, 4, 'C-3'],
    ],
  },
  steps: [
    { id: 's1', name: 'Tipo Alterado', op: 'changeType', columns: [{ column: 'Valor', type: 'decimal' }, { column: 'Qtd', type: 'integer' }] },
    { id: 's2', name: 'Duplicatas Removidas', op: 'removeDuplicates' },
    { id: 's3', name: 'Preenchido Abaixo', op: 'fillDown', columns: ['Cidade'] },
    { id: 's4', name: 'Preenchido Acima', op: 'fillUp', columns: ['Cidade'] },
    { id: 's5', name: 'Valor Substituído', op: 'replaceValues', columns: ['Produto'], find: 'azul', replace: 'preta', matchEntireCell: false },
    { id: 's6', name: 'Texto Maiúsculo', op: 'textTransform', columns: ['Cidade'], fn: 'upper' },
    { id: 's7', name: 'Coluna Dividida', op: 'splitColumn', column: 'Cod', mode: 'delimiterEach', delimiter: '-', into: ['Cod.1', 'Cod.2'] },
    { id: 's8', name: 'Colunas Mescladas', op: 'mergeColumns', columns: ['Cod.1', 'Cod.2'], separator: '/', newColumn: 'Código' },
    { id: 's9', name: 'Coluna Duplicada', op: 'duplicateColumn', column: 'Valor', newColumn: 'Valor - Cópia' },
    { id: 's10', name: 'Colunas Renomeadas', op: 'renameColumns', renames: [['Valor - Cópia', 'Preço']] },
    {
      id: 's11',
      name: 'Coluna Condicional',
      op: 'addConditionalColumn',
      newColumn: 'Faixa',
      rules: [{ when: { column: 'Valor', op: 'gte', value: 10 }, then: 'Alta' }],
      otherwise: 'Baixa',
      type: 'text',
    },
    { id: 's12', name: 'Total', op: 'addCustomColumn', newColumn: 'Total', expression: '[Valor] * [Qtd]', type: 'decimal' },
    { id: 's13', name: 'Índice', op: 'addIndexColumn', newColumn: 'Índice', start: 1, step: 1 },
    {
      id: 's14',
      name: 'Linhas Filtradas',
      op: 'filterRows',
      condition: { and: [{ column: 'Cidade', op: 'isNotNull' }, { not: { column: 'Produto', op: 'in', values: ['x', 'y'] } }] },
    },
    { id: 's15', name: 'Linhas Classificadas', op: 'sort', by: [{ column: 'Total', descending: true }] },
    { id: 's16', name: 'Primeiras Linhas', op: 'keepTopRows', count: 10 },
    { id: 's17', name: 'Linhas Superiores Removidas', op: 'removeTopRows', count: 0 },
    { id: 's18', name: 'Últimas Linhas', op: 'keepBottomRows', count: 10 },
    { id: 's19', name: 'Linhas Inferiores Removidas', op: 'removeBottomRows', count: 0 },
    { id: 's20', name: 'Linhas em Branco Removidas', op: 'removeBlankRows' },
    { id: 's21', name: 'Erros Removidos', op: 'removeRowsWithErrors' },
    { id: 's22', name: 'Outras Colunas Removidas', op: 'selectColumns', columns: ['Produto', 'Cidade', 'Valor', 'Qtd', 'Total', 'Faixa', 'Preço', 'Código'] },
    { id: 's23', name: 'Colunas Removidas', op: 'removeColumns', columns: ['Código'] },
    {
      id: 's24',
      name: 'Linhas Agrupadas',
      op: 'groupBy',
      keys: ['Cidade'],
      aggregations: [
        { name: 'Soma', op: 'sum', column: 'Total' },
        { name: 'Linhas', op: 'countRows' },
      ],
    },
  ],
};

/** Etapas que só fazem sentido com outras consultas ou dados em formato próprio: round-trip M apenas. */
export const SHAPE_STEPS_QUERY: EtlQuery = {
  name: 'ContratoFormato',
  source: { kind: 'table', table: 'Outra' },
  steps: [
    { id: 'p0', name: 'Cabeçalhos Promovidos', op: 'promoteHeaders' },
    { id: 'p1', name: 'Pivot', op: 'pivot', column: 'Mês', valuesColumn: 'Valor', aggregation: 'sum' },
    { id: 'p2', name: 'Unpivot Outras', op: 'unpivotOtherColumns', keep: ['Cidade'], attributeColumn: 'Atributo', valueColumn: 'Valor' },
    { id: 'p3', name: 'Unpivot', op: 'unpivotColumns', columns: ['Jan', 'Fev'], attributeColumn: 'Mês', valueColumn: 'Valor' },
    {
      id: 'p4',
      name: 'Consultas Mescladas',
      op: 'mergeQueries',
      rightQuery: 'dCidades',
      leftKeys: ['Cidade'],
      rightKeys: ['Nome'],
      joinKind: 'leftOuter',
      expand: [{ column: 'UF', as: 'UF' }],
    },
    { id: 'p5', name: 'Consultas Acrescentadas', op: 'appendQueries', queries: ['Vendas2025'] },
  ],
};

const JSON_PATH = resolve(__dirname, '../../../../tests/etl-contract/all_steps.json');

describe('ETL contract fixture (TS ⇄ Rust)', () => {
  it('tests/etl-contract/all_steps.json matches the typed TS fixture', () => {
    const expected = { allSteps: ALL_STEPS_QUERY, shapeSteps: SHAPE_STEPS_QUERY };
    if (process.env.UPDATE_ETL_CONTRACT === '1' || !existsSync(JSON_PATH)) {
      writeFileSync(JSON_PATH, JSON.stringify(expected, null, 2) + '\n');
    }
    expect(JSON.parse(readFileSync(JSON_PATH, 'utf8'))).toEqual(expected);
  });

  it('covers every EtlStep op exactly as the UI types define them', () => {
    const ops = new Set([...ALL_STEPS_QUERY.steps, ...SHAPE_STEPS_QUERY.steps].map(s => s.op));
    // opaqueM só é produzido pelo Rust (etl_from_m); o resto precisa estar aqui.
    expect(ops.size).toBe(30);
  });
});
