import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { EtlQuery } from '../../types/etl';

// O comando Rust etl_from_m é simulado: aqui interessa o que a store faz com ele.
const fromM = vi.fn();
const apply = vi.fn();
vi.mock('../../services/etl/etlClient', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/etl/etlClient')>();
  return { ...actual, etlFromM: (...a: unknown[]) => fromM(...a), etlApply: (...a: unknown[]) => apply(...a) };
});

const { useBiStore } = await import('../useBiStore');

const M = `let
    Source = Excel.Workbook(File.Contents("\\\\tsclient\\Linux\\Rhino.xlsx"), null, true),
    Sheet = Source{[Item="Categorias",Kind="Sheet"]}[Data],
    #"Promoted Headers" = Table.PromoteHeaders(Sheet, [PromoteAllScalars=true])
in
    #"Promoted Headers"`;

function addPbipTable(id: string, extra: Record<string, unknown> = {}) {
  useBiStore.getState().addTable({
    id,
    name: 'dCategorias',
    originalName: 'dCategorias',
    columns: [
      { id: 'c_cat', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
      { id: 'c_dre', name: 'DRE', originalName: 'DRE', dataType: 'text' },
    ],
    rows: [{ Categoria: 'Aluguel', DRE: 'Sim' }],
    sourceConfig: { type: 'xlsx', filePath: '/home/x/Rhino.xlsx', sheetName: 'Categorias', mScript: M, ...extra },
  } as any);
}

describe('ensureQuery / closeAndApplyQuery (round-trip .pbip)', () => {
  beforeEach(() => {
    fromM.mockReset();
    apply.mockReset();
    useBiStore.setState({ appliedSteps: {}, selectedStepByTable: {} });
  });

  it('turns the table M into editable steps and keeps the import-resolved file path', async () => {
    const parsed: EtlQuery = {
      name: 'dCategorias',
      source: { kind: 'excel', path: '\\\\tsclient\\Linux\\Rhino.xlsx', sheet: 'Categorias' },
      steps: [{ id: '', name: 'Promoted Headers', op: 'promoteHeaders' }],
    };
    fromM.mockResolvedValue({ query: parsed, warnings: [] });
    addPbipTable('t_pbip');
    const q = await useBiStore.getState().ensureQuery('t_pbip');
    expect(fromM).toHaveBeenCalledWith({ name: 'dCategorias', m: M });
    expect(q.source).toEqual({ kind: 'excel', path: '/home/x/Rhino.xlsx', sheet: 'Categorias' });
    expect(q.steps.map(s => s.op)).toEqual(['promoteHeaders']);
    expect(useBiStore.getState().appliedSteps.t_pbip).toEqual(q);
    // Segunda chamada não reconverte.
    await useBiStore.getState().ensureQuery('t_pbip');
    expect(fromM).toHaveBeenCalledTimes(1);
  });

  it('falls back to a frozen inline snapshot (never a self-referencing table source) when M is opaque', async () => {
    fromM.mockResolvedValue({ query: { name: 'dCategorias', source: { kind: 'opaqueM', m: 'x' }, steps: [] }, warnings: ['w'] });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    addPbipTable('t_opaque', { filePath: undefined, type: 'pbip' });
    const q = await useBiStore.getState().ensureQuery('t_opaque');
    warn.mockRestore();
    expect(q.source).toEqual({ kind: 'inline', columns: ['Categoria', 'DRE'], rows: [['Aluguel', 'Sim']] });
    expect(q.steps).toEqual([]);
  });

  it('closeAndApplyQuery keeps column ids by name when steps reorder or drop columns', async () => {
    addPbipTable('t_ids');
    useBiStore.getState().setQuery('t_ids', { name: 'dCategorias', source: { kind: 'excel', path: '/x' }, steps: [] });
    apply.mockResolvedValue({
      columns: [
        { name: 'DRE', originalName: 'DRE', dataType: 'text' },
        { name: 'Nova', originalName: 'Nova', dataType: 'text' },
      ],
      rows: [{ DRE: 'Sim', Nova: 'a' }],
    });
    await useBiStore.getState().closeAndApplyQuery('t_ids');
    const cols = useBiStore.getState().tables.find(t => t.id === 't_ids')!.columns;
    expect(cols[0]).toMatchObject({ id: 'c_dre', name: 'DRE' });
    expect(cols[1].name).toBe('Nova');
    expect(cols[1].id).not.toBe('c_cat');
  });

  it('dedupes duplicate step/column names coming from imported M (Observação do print v0.11 R2)', async () => {
    // Reproduz o M que gerou o print do Luis: duas etapas "Mesclado
    // Adicionado" (um merge feito duas vezes, sem perceber a colisão de
    // nomes), ambas produzindo uma coluna "Mesclado". Sem dedup, a segunda
    // etapa se autorreferencia no M gerado e a segunda coluna sobrescreve a
    // primeira na grade.
    const parsed: EtlQuery = {
      name: 'dCategorias',
      source: { kind: 'excel', path: '\\\\tsclient\\Linux\\Rhino.xlsx', sheet: 'Categorias' },
      steps: [
        {
          id: 's1',
          name: 'Mesclado Adicionado',
          op: 'addCustomColumn',
          newColumn: 'Mesclado',
          expression: 'Text.Combine({Text.From([Categoria]), [DRE]}, "/")',
          type: 'text',
        },
        {
          id: 's2',
          name: 'Mesclado Adicionado',
          op: 'addCustomColumn',
          newColumn: 'Mesclado',
          expression: 'Text.Combine({[DRE], Text.From([Categoria])}, "-")',
          type: 'text',
        },
      ],
    };
    fromM.mockResolvedValue({ query: parsed, warnings: [] });
    addPbipTable('t_dup');
    const q = await useBiStore.getState().ensureQuery('t_dup');
    expect(q.steps.map(s => s.name)).toEqual(['Mesclado Adicionado', 'Mesclado Adicionado1']);
    expect(q.steps.map(s => (s as any).newColumn)).toEqual(['Mesclado', 'Mesclado.1']);
  });
});
