// Tabela "menu config -> EtlStep builders": percorre a árvore de cada aba do
// ribbon, localiza a folha por id e garante que o EtlStep produzido bate
// exatamente com o contrato de docs/ETL_V09.md (sem contar `id`).

import { describe, it, expect, vi } from 'vitest';
import { homeTab, transformTab, addColumnTab, viewTab, MenuNode, RibbonCtx, RibbonTabDef } from '../menus';

function findNode(nodes: MenuNode[], id: string): MenuNode | undefined {
  for (const n of nodes) {
    if (n.id === id) return n;
    if (n.kind === 'submenu') {
      const found = findNode(n.items, id);
      if (found) return found;
    }
  }
  return undefined;
}

function findInTab(tab: RibbonTabDef, id: string): MenuNode {
  for (const g of tab.groups) {
    const found = findNode(g.items, id);
    if (found) return found;
  }
  throw new Error(`menu node not found: ${id}`);
}

function makeCtx(overrides: Partial<RibbonCtx> = {}): RibbonCtx & { addStep: ReturnType<typeof vi.fn>; openDialog: ReturnType<typeof vi.fn>; openParamDialog: ReturnType<typeof vi.fn> } {
  return {
    selectedColumns: ['Nome'],
    columnTypes: { Nome: 'text', Valor: 'decimal', Data: 'date', A: 'decimal', B: 'decimal' },
    allColumns: ['Nome', 'Valor', 'Data', 'A', 'B'],
    addStep: vi.fn(),
    openDialog: vi.fn(),
    openParamDialog: vi.fn(),
    ...overrides,
  } as any;
}

function run(node: MenuNode, ctx: RibbonCtx) {
  if (node.kind !== 'action') throw new Error('not a leaf action');
  node.run(ctx);
}

const homeTabDef = () => homeTab({ onCloseAndApply: vi.fn(), onApply: vi.fn(), onClose: vi.fn(), onRefreshPreview: vi.fn() });

describe('Home tab: direct-step actions produce the exact contract JSON', () => {
  it.each([
    ['removeColumns', { op: 'removeColumns', columns: ['Nome'], name: 'Colunas Removidas' }],
    ['removeOtherColumns', { op: 'selectColumns', columns: ['Nome'], name: 'Outras Colunas Removidas' }],
    ['removeDup', { op: 'removeDuplicates', columns: ['Nome'], name: 'Duplicadas Removidas' }],
    ['removeBlank', { op: 'removeBlankRows', name: 'Linhas em Branco Removidas' }],
    ['removeErrors', { op: 'removeRowsWithErrors', columns: ['Nome'], name: 'Linhas com Erros Removidas' }],
    ['keepDup', { op: 'keepDuplicates', columns: ['Nome'], name: 'Duplicatas Mantidas' }],
    ['keepErrors', { op: 'keepRowsWithErrors', columns: ['Nome'], name: 'Linhas com Erros Mantidas' }],
    ['sortAsc', { op: 'sort', by: [{ column: 'Nome', descending: false }], name: 'Classificado' }],
    ['sortDesc', { op: 'sort', by: [{ column: 'Nome', descending: true }], name: 'Classificado' }],
    ['promoteHeaders', { op: 'promoteHeaders', name: 'Cabeçalho Promovido' }],
    ['demoteHeaders', { op: 'demoteHeaders', name: 'Cabeçalhos Rebaixados' }],
  ])('%s', (id, expected) => {
    const ctx = makeCtx();
    run(findInTab(homeTabDef(), id), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith(expected);
  });

  it('keepTop opens a param dialog and emits keepTopRows on submit', () => {
    const ctx = makeCtx();
    run(findInTab(homeTabDef(), 'keepTop'), ctx);
    expect(ctx.openParamDialog).toHaveBeenCalled();
    const [, fields, onSubmit] = ctx.openParamDialog.mock.calls[0];
    expect(fields[0].key).toBe('count');
    onSubmit({ count: 50 });
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'keepTopRows', count: 50, name: 'Linhas Superiores Mantidas' });
  });

  it('removeAlternate collects offset/remove/keep and emits removeAlternateRows', () => {
    const ctx = makeCtx();
    run(findInTab(homeTabDef(), 'removeAlternate'), ctx);
    const [, , onSubmit] = ctx.openParamDialog.mock.calls[0];
    onSubmit({ offset: 1, remove: 2, keep: 3 });
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'removeAlternateRows', offset: 1, remove: 2, keep: 3, name: 'Linhas Alternadas Removidas' });
  });

  it('dialog-backed actions call openDialog with the right kind', () => {
    const ctx = makeCtx();
    run(findInTab(homeTabDef(), 'changeType'), ctx);
    expect(ctx.openDialog).toHaveBeenCalledWith('changeType');
    run(findInTab(homeTabDef(), 'mergeQueries'), ctx);
    expect(ctx.openDialog).toHaveBeenCalledWith('mergeQueries');
  });

  it('disables column-dependent actions when nothing is selected', () => {
    const ctx = makeCtx({ selectedColumns: [] });
    const node = findInTab(homeTabDef(), 'removeColumns');
    expect(node.kind === 'action' && node.enabled?.(ctx)).toBe(false);
    expect(node.kind === 'action' && node.disabledReason).toBe('Selecione uma coluna');
  });
});

describe('Transform tab: table / column ops', () => {
  it.each([
    ['transpose', { op: 'transpose', name: 'Tabela Transposta' }],
    ['reverseRows', { op: 'reverseRows', name: 'Linhas Invertidas' }],
    ['countRows', { op: 'groupBy', keys: [], aggregations: [{ name: 'Contagem', op: 'countRows' }], name: 'Linhas Contadas' }],
    ['fillDown', { op: 'fillDown', columns: ['Nome'], name: 'Preenchido para Baixo' }],
    ['fillUp', { op: 'fillUp', columns: ['Nome'], name: 'Preenchido para Cima' }],
    ['unpivotSelected', { op: 'unpivotColumns', columns: ['Nome'], attributeColumn: 'Atributo', valueColumn: 'Valor', name: 'Unpivot de Colunas' }],
    ['unpivotOthers', { op: 'unpivotOtherColumns', keep: ['Nome'], attributeColumn: 'Atributo', valueColumn: 'Valor', name: 'Unpivot de Outras Colunas' }],
  ])('%s', (id, expected) => {
    const ctx = makeCtx();
    run(findInTab(transformTab(), id), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith(expected);
  });

  it('detectType changes type to the type currently inferred from the preview', () => {
    const ctx = makeCtx({ selectedColumns: ['Valor'] });
    run(findInTab(transformTab(), 'detectType'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'changeType', columns: [{ column: 'Valor', type: 'decimal' }], name: 'Tipo Detectado' });
  });

  it('move reorders the full column list (reorderColumns)', () => {
    const ctx = makeCtx({ selectedColumns: ['Valor'] });
    run(findInTab(transformTab(), 'moveStart'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'reorderColumns', columns: ['Valor', 'Nome', 'Data', 'A', 'B'], name: 'Colunas Reordenadas' });
  });

  it('text format "inplace-upper" emits transformColumns with Text.Upper(_)', () => {
    const ctx = makeCtx();
    run(findInTab(transformTab(), 'inplace-upper'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'transformColumns', columns: ['Nome'], expression: 'Text.Upper(_)', type: 'text', name: 'MAIÚSCULAS' });
  });

  it('date "inplace-year" is disabled unless a date column is selected', () => {
    const node = findInTab(transformTab(), 'inplace-year');
    expect(node.kind === 'action' && node.enabled?.(makeCtx({ selectedColumns: ['Nome'] }))).toBe(false);
    expect(node.kind === 'action' && node.enabled?.(makeCtx({ selectedColumns: ['Data'] }))).toBe(true);
  });

  it('number standard "numstd-inplace-multiply" prompts a value for a single column', () => {
    const ctx = makeCtx({ selectedColumns: ['Valor'] });
    run(findInTab(transformTab(), 'numstd-inplace-multiply'), ctx);
    const [, , onSubmit] = ctx.openParamDialog.mock.calls[0];
    onSubmit({ value: 3 });
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'transformColumns', columns: ['Valor'], expression: '_ * 3', type: 'decimal', name: 'Multiplicar' });
  });

  it('number standard "numstd-inplace-multiply" combines two selected columns instead', () => {
    const ctx = makeCtx({ selectedColumns: ['A', 'B'] });
    run(findInTab(transformTab(), 'numstd-inplace-multiply'), ctx);
    expect(ctx.openParamDialog).not.toHaveBeenCalled();
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'addCustomColumn', newColumn: 'Multiplicação', expression: '[A] * [B]', type: 'decimal', name: 'Multiplicação Adicionado' });
  });
});

describe('Adicionar Coluna tab: always creates a new column', () => {
  it('duplicateColumn adds a copy of the selected column', () => {
    const ctx = makeCtx({ selectedColumns: ['Nome'] });
    run(findInTab(addColumnTab(), 'duplicateColumn'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'duplicateColumn', column: 'Nome', newColumn: 'Nome - Cópia', name: 'Duplicar Nome' });
  });

  it('indexFrom0 / indexFrom1 add an index column with the documented start', () => {
    const ctx = makeCtx();
    run(findInTab(addColumnTab(), 'indexFrom0'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'addIndexColumn', newColumn: 'Índice', start: 0, step: 1, name: 'Índice Adicionado' });
    run(findInTab(addColumnTab(), 'indexFrom1'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'addIndexColumn', newColumn: 'Índice', start: 1, step: 1, name: 'Índice Adicionado' });
  });

  it('new-length adds Text.Length([Nome]) as "Comprimento"', () => {
    const ctx = makeCtx({ selectedColumns: ['Nome'] });
    run(findInTab(addColumnTab(), 'new-length'), ctx);
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'addCustomColumn', newColumn: 'Comprimento', expression: 'Text.Length([Nome])', type: 'integer', name: 'Comprimento Adicionado' });
  });

  it('new-year adds Date.Year([Data]) as "Ano", disabled for non-date columns', () => {
    const okCtx = makeCtx({ selectedColumns: ['Data'] });
    const node = findInTab(addColumnTab(), 'new-year');
    expect(node.kind === 'action' && node.enabled?.(okCtx)).toBe(true);
    run(node, okCtx);
    expect(okCtx.addStep).toHaveBeenCalledWith({ op: 'addCustomColumn', newColumn: 'Ano', expression: 'Date.Year([Data])', type: 'integer', name: 'Ano Adicionado' });

    const badCtx = makeCtx({ selectedColumns: ['Nome'] });
    expect(node.kind === 'action' && node.enabled?.(badCtx)).toBe(false);
  });

  it('mergeColumnsNew keeps the originals (addCustomColumn, not mergeColumns)', () => {
    const ctx = makeCtx({ selectedColumns: ['A', 'B'] });
    run(findInTab(addColumnTab(), 'mergeColumnsNew'), ctx);
    const [, , onSubmit] = ctx.openParamDialog.mock.calls[0];
    onSubmit({ separator: '-' });
    expect(ctx.addStep).toHaveBeenCalledWith({ op: 'addCustomColumn', newColumn: 'Mesclado', expression: '[A] & "-" & [B]', name: 'Mesclado Adicionado' });
  });
});

describe('Exibir tab: toggles and navigation', () => {
  it('toggle actions call back with the toggle key, prefixed with ✓ when on', () => {
    const onToggle = vi.fn();
    const tab = viewTab({ formulaBar: true, quality: false, distribution: false, profile: false }, onToggle);
    const node = findInTab(tab, 'toggle-quality');
    expect(node.kind === 'action' && node.label).toBe('Qualidade da Coluna');
    run(node, makeCtx());
    expect(onToggle).toHaveBeenCalledWith('quality');

    const onNode = findInTab(tab, 'toggle-formulaBar');
    expect(onNode.kind === 'action' && onNode.label).toBe('✓ Barra de Fórmulas');
  });

  it('goToColumn opens the navigation dialog', () => {
    const ctx = makeCtx();
    const tab = viewTab({ formulaBar: true, quality: true, distribution: true, profile: true }, vi.fn());
    run(findInTab(tab, 'goToColumn'), ctx);
    expect(ctx.openDialog).toHaveBeenCalledWith('goToColumn');
  });
});
