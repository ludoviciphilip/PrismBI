// Definições data-driven do ribbon em abas (Página Inicial | Transformar |
// Adicionar Coluna | Exibir), no estilo do editor do Power Query Desktop.
// Cada folha sabe construir o EtlStep certo (ou abrir um diálogo/parâmetro);
// os testes de "menu config -> EtlStep builders" iteram esta estrutura.

import { DistributiveOmit, EtlStep, EtlType } from '../../../types/etl';
import { CheckCheck, Check, X, RefreshCw, Code2 } from 'lucide-react';
import {
  OpDef,
  ParamFieldSpec,
  TEXT_EXTRACT_OPS,
  NUMBER_STANDARD_OPS,
  NUMBER_STANDARD_COMBINE_OPS,
  NUMBER_SCIENTIFIC_OPS,
  NUMBER_ROUNDING_OPS,
  NUMBER_INFO_OPS,
  DATE_OP_GROUPS,
  buildTransformColumnsStep,
  buildAddCustomColumnStep,
  buildCombineStep,
} from './opDefs';

export type RibbonDialogKind =
  | 'changeType'
  | 'filterRows'
  | 'groupBy'
  | 'mergeQueries'
  | 'splitColumn'
  | 'mergeColumns'
  | 'replaceValues'
  | 'customColumn'
  | 'conditionalColumn'
  | 'indexColumn'
  | 'appendQueries'
  | 'pivot'
  | 'advancedEditor'
  | 'chooseColumns'
  | 'goToColumn';

export interface RibbonCtx {
  selectedColumns: string[];
  columnTypes: Record<string, EtlType | string>;
  allColumns: string[];
  addStep: (step: DistributiveOmit<EtlStep, 'id'>) => void;
  openDialog: (kind: RibbonDialogKind) => void;
  openParamDialog: (title: string, fields: ParamFieldSpec[], onSubmit: (values: Record<string, any>) => void) => void;
}

export type MenuNode =
  | { kind: 'action'; id: string; label: string; icon?: any; tooltip?: string; run: (ctx: RibbonCtx) => void; enabled?: (ctx: RibbonCtx) => boolean; disabledReason?: string }
  | { kind: 'submenu'; id: string; label: string; icon?: any; tooltip?: string; items: MenuNode[]; enabled?: (ctx: RibbonCtx) => boolean; disabledReason?: string };

export interface RibbonGroupDef {
  title: string;
  items: MenuNode[];
}

export interface RibbonTabDef {
  id: string;
  label: string;
  groups: RibbonGroupDef[];
}

// ---------------------------------------------------------------------------
// Helpers de habilitação por tipo de coluna selecionada
// ---------------------------------------------------------------------------
const TEXT_TYPES: EtlType[] = ['text'];
const NUMBER_TYPES: EtlType[] = ['integer', 'decimal', 'currency'];
const DATE_TYPES: EtlType[] = ['date', 'datetime'];

function allOfType(ctx: RibbonCtx, types: EtlType[]): boolean {
  return ctx.selectedColumns.length > 0 && ctx.selectedColumns.every(c => types.includes(ctx.columnTypes[c] as EtlType));
}

function reasonFor(types: EtlType[]): string {
  if (types === TEXT_TYPES) return 'Selecione uma coluna de texto';
  if (types === NUMBER_TYPES) return 'Selecione uma coluna de número';
  if (types === DATE_TYPES) return 'Selecione uma coluna de data';
  return 'Selecione uma coluna';
}

function hasSelection(ctx: RibbonCtx): boolean {
  return ctx.selectedColumns.length > 0;
}

// ---------------------------------------------------------------------------
// Folhas data-driven a partir de OpDef (compartilhadas por Transformar/Adicionar Coluna)
// ---------------------------------------------------------------------------
function opLeaf(op: OpDef, mode: 'inplace' | 'new', types: EtlType[]): MenuNode {
  return {
    kind: 'action',
    id: `${mode}-${op.id}`,
    label: op.label,
    enabled: ctx => allOfType(ctx, types) && (mode === 'inplace' || ctx.selectedColumns.length === 1),
    disabledReason: reasonFor(types),
    run: ctx => {
      const col = ctx.selectedColumns[0];
      const apply = (params: Record<string, any>) =>
        mode === 'inplace' ? ctx.addStep(buildTransformColumnsStep(op, ctx.selectedColumns, params)) : ctx.addStep(buildAddCustomColumnStep(op, col, params));
      if (op.params?.length) ctx.openParamDialog(op.label, op.params, apply);
      else apply({});
    },
  };
}

function textExtractMenu(mode: 'inplace' | 'new'): MenuNode {
  return { kind: 'submenu', id: `extract-${mode}`, label: 'Extrair', items: TEXT_EXTRACT_OPS.map(op => opLeaf(op, mode, TEXT_TYPES)) };
}

const TEXT_FORMAT_OPS: OpDef[] = [
  { id: 'lower', label: 'minúsculas', type: 'text', expr: ref => `Text.Lower(${ref})`, newColumnName: 'minúsculas' },
  { id: 'upper', label: 'MAIÚSCULAS', type: 'text', expr: ref => `Text.Upper(${ref})`, newColumnName: 'MAIÚSCULAS' },
  { id: 'proper', label: 'Capitalizar Cada Palavra', type: 'text', expr: ref => `Text.Proper(${ref})`, newColumnName: 'Capitalizado' },
  { id: 'trim', label: 'Aparar', type: 'text', expr: ref => `Text.Trim(${ref})`, newColumnName: 'Aparado' },
  { id: 'clean', label: 'Limpar', type: 'text', expr: ref => `Text.Clean(${ref})`, newColumnName: 'Limpo' },
  { id: 'prefix', label: 'Adicionar Prefixo', type: 'text', params: [{ key: 'prefix', label: 'Prefixo', type: 'text', defaultValue: '' }], expr: (ref, p) => `"${p.prefix}" & ${ref}`, newColumnName: 'Prefixo Adicionado' },
  { id: 'suffix', label: 'Adicionar Sufixo', type: 'text', params: [{ key: 'suffix', label: 'Sufixo', type: 'text', defaultValue: '' }], expr: (ref, p) => `${ref} & "${p.suffix}"`, newColumnName: 'Sufixo Adicionado' },
];

function textFormatMenu(mode: 'inplace' | 'new'): MenuNode {
  return { kind: 'submenu', id: `format-${mode}`, label: 'Formato', items: TEXT_FORMAT_OPS.map(op => opLeaf(op, mode, TEXT_TYPES)) };
}

function numberStandardMenu(mode: 'inplace' | 'new'): MenuNode {
  return {
    kind: 'submenu',
    id: `numstd-${mode}`,
    label: 'Padrão',
    items: NUMBER_STANDARD_OPS.map(op => ({
      kind: 'action',
      id: `numstd-${mode}-${op.id}`,
      label: op.label,
      enabled: ctx => allOfType(ctx, NUMBER_TYPES),
      disabledReason: reasonFor(NUMBER_TYPES),
      run: ctx => {
        if (ctx.selectedColumns.length === 2) {
          const combineOp = NUMBER_STANDARD_COMBINE_OPS[op.id];
          ctx.addStep(buildCombineStep(combineOp, ctx.selectedColumns[0], ctx.selectedColumns[1]));
          return;
        }
        const col = ctx.selectedColumns[0];
        ctx.openParamDialog(op.label, op.params!, params =>
          mode === 'inplace' ? ctx.addStep(buildTransformColumnsStep(op, ctx.selectedColumns, params)) : ctx.addStep(buildAddCustomColumnStep(op, col, params)),
        );
      },
    })),
  };
}

function numberScientificMenus(mode: 'inplace' | 'new'): MenuNode[] {
  return NUMBER_SCIENTIFIC_OPS.map(g =>
    g.ops.length === 1
      ? opLeaf(g.ops[0], mode, NUMBER_TYPES)
      : { kind: 'submenu', id: `${g.group}-${mode}`, label: g.group, items: g.ops.map(op => opLeaf(op, mode, NUMBER_TYPES)) },
  );
}

function numberRoundingMenu(mode: 'inplace' | 'new'): MenuNode {
  return { kind: 'submenu', id: `round-${mode}`, label: 'Arredondamento', items: NUMBER_ROUNDING_OPS.map(op => opLeaf(op, mode, NUMBER_TYPES)) };
}

function numberInfoMenu(mode: 'inplace' | 'new'): MenuNode {
  return { kind: 'submenu', id: `info-${mode}`, label: 'Informação', items: NUMBER_INFO_OPS.map(op => opLeaf(op, mode, NUMBER_TYPES)) };
}

function dateMenu(mode: 'inplace' | 'new'): MenuNode[] {
  return DATE_OP_GROUPS.map(g =>
    g.group === 'Idade'
      ? opLeaf(g.ops[0], mode, DATE_TYPES)
      : { kind: 'submenu', id: `date-${mode}-${g.group}`, label: g.group, items: g.ops.map(op => opLeaf(op, mode, DATE_TYPES)) },
  );
}

// ---------------------------------------------------------------------------
// Ações simples (sem parâmetro, sem diálogo) que só montam o EtlStep direto
// ---------------------------------------------------------------------------
const action = (id: string, label: string, run: (ctx: RibbonCtx) => void, enabled?: (ctx: RibbonCtx) => boolean, disabledReason?: string, icon?: any, tooltip?: string): MenuNode => ({
  kind: 'action',
  id,
  label,
  run,
  enabled,
  disabledReason,
  icon,
  tooltip,
});

const dialogAction = (id: string, label: string, dialog: RibbonDialogKind, enabled?: (ctx: RibbonCtx) => boolean, disabledReason?: string, icon?: any): MenuNode =>
  action(id, label, ctx => ctx.openDialog(dialog), enabled, disabledReason, icon);

// ---------------------------------------------------------------------------
// Aba "Página Inicial"
// ---------------------------------------------------------------------------
export function homeTab(extra: { onCloseAndApply: () => void; onApply: () => void; onClose: () => void; onRefreshPreview: () => void }): RibbonTabDef {
  return {
    id: 'home',
    label: 'Página Inicial',
    groups: [
      {
        title: 'Geral',
        items: [
          action('closeApply', 'Fechar e Aplicar', extra.onCloseAndApply, undefined, undefined, CheckCheck),
          action('apply', 'Aplicar', extra.onApply, undefined, undefined, Check),
          action('close', 'Fechar', extra.onClose, undefined, undefined, X),
          action(
            'refreshPreview',
            'Atualizar Visualização',
            extra.onRefreshPreview,
            undefined,
            undefined,
            RefreshCw,
            'Relê a fonte e atualiza só a visualização (preview). Não muda o relatório — use "Fechar e Aplicar" para isso.',
          ),
          dialogAction('advancedEditor', 'Editor Avançado', 'advancedEditor', undefined, undefined, Code2),
        ],
      },
      {
        title: 'Gerenciar Colunas',
        items: [
          dialogAction('chooseColumns', 'Escolher Colunas', 'chooseColumns'),
          {
            kind: 'submenu',
            id: 'removeColumnsMenu',
            label: 'Remover Colunas',
            items: [
              action('removeColumns', 'Remover', ctx => ctx.addStep({ op: 'removeColumns', columns: ctx.selectedColumns, name: 'Colunas Removidas' }), hasSelection, 'Selecione uma coluna'),
              action(
                'removeOtherColumns',
                'Remover Outras Colunas',
                ctx => ctx.addStep({ op: 'selectColumns', columns: ctx.selectedColumns, name: 'Outras Colunas Removidas' }),
                hasSelection,
                'Selecione uma coluna',
              ),
            ],
          },
        ],
      },
      {
        title: 'Reduzir Linhas',
        items: [
          {
            kind: 'submenu',
            id: 'keepRowsMenu',
            label: 'Manter Linhas',
            items: [
              action('keepTop', 'Primeiras Linhas', ctx => ctx.openParamDialog('Manter Primeiras Linhas', [{ key: 'count', label: 'Número de linhas', type: 'integer', defaultValue: 1000 }], p => ctx.addStep({ op: 'keepTopRows', count: p.count, name: 'Linhas Superiores Mantidas' }))),
              action('keepBottom', 'Últimas Linhas', ctx => ctx.openParamDialog('Manter Últimas Linhas', [{ key: 'count', label: 'Número de linhas', type: 'integer', defaultValue: 1000 }], p => ctx.addStep({ op: 'keepBottomRows', count: p.count, name: 'Linhas Inferiores Mantidas' }))),
              action('keepRange', 'Intervalo de Linhas', ctx => ctx.openParamDialog('Manter Intervalo de Linhas', [{ key: 'offset', label: 'Primeira linha (0-based)', type: 'integer', defaultValue: 0 }, { key: 'count', label: 'Número de linhas', type: 'integer', defaultValue: 100 }], p => ctx.addStep({ op: 'keepRange', offset: p.offset, count: p.count, name: 'Intervalo de Linhas Mantido' }))),
              action('keepDup', 'Duplicatas', ctx => ctx.addStep({ op: 'keepDuplicates', columns: ctx.selectedColumns.length ? ctx.selectedColumns : undefined, name: 'Duplicatas Mantidas' })),
              action('keepErrors', 'Erros', ctx => ctx.addStep({ op: 'keepRowsWithErrors', columns: ctx.selectedColumns.length ? ctx.selectedColumns : undefined, name: 'Linhas com Erros Mantidas' })),
            ],
          },
          {
            kind: 'submenu',
            id: 'removeRowsMenu',
            label: 'Remover Linhas',
            items: [
              action('removeTop', 'Primeiras Linhas', ctx => ctx.openParamDialog('Remover Primeiras Linhas', [{ key: 'count', label: 'Número de linhas', type: 'integer', defaultValue: 1 }], p => ctx.addStep({ op: 'removeTopRows', count: p.count, name: 'Linhas Superiores Removidas' }))),
              action('removeBottom', 'Últimas Linhas', ctx => ctx.openParamDialog('Remover Últimas Linhas', [{ key: 'count', label: 'Número de linhas', type: 'integer', defaultValue: 1 }], p => ctx.addStep({ op: 'removeBottomRows', count: p.count, name: 'Linhas Inferiores Removidas' }))),
              action('removeAlternate', 'Linhas Alternadas', ctx => ctx.openParamDialog('Remover Linhas Alternadas', [{ key: 'offset', label: 'A partir da linha', type: 'integer', defaultValue: 0 }, { key: 'remove', label: 'Remover (nº de linhas)', type: 'integer', defaultValue: 1 }, { key: 'keep', label: 'Manter (nº de linhas)', type: 'integer', defaultValue: 1 }], p => ctx.addStep({ op: 'removeAlternateRows', offset: p.offset, remove: p.remove, keep: p.keep, name: 'Linhas Alternadas Removidas' }))),
              action('removeDup', 'Duplicatas', ctx => ctx.addStep({ op: 'removeDuplicates', columns: ctx.selectedColumns.length ? ctx.selectedColumns : undefined, name: 'Duplicadas Removidas' })),
              action('removeBlank', 'Em Branco', ctx => ctx.addStep({ op: 'removeBlankRows', name: 'Linhas em Branco Removidas' })),
              action('removeErrors', 'Erros', ctx => ctx.addStep({ op: 'removeRowsWithErrors', columns: ctx.selectedColumns.length ? ctx.selectedColumns : undefined, name: 'Linhas com Erros Removidas' })),
            ],
          },
        ],
      },
      {
        title: 'Classificar',
        items: [
          action('sortAsc', 'Crescente', ctx => ctx.addStep({ op: 'sort', by: ctx.selectedColumns.map(column => ({ column, descending: false })), name: 'Classificado' }), hasSelection, 'Selecione uma coluna'),
          action('sortDesc', 'Decrescente', ctx => ctx.addStep({ op: 'sort', by: ctx.selectedColumns.map(column => ({ column, descending: true })), name: 'Classificado' }), hasSelection, 'Selecione uma coluna'),
        ],
      },
      {
        title: 'Transformar',
        items: [
          dialogAction('splitColumn', 'Dividir Coluna', 'splitColumn', ctx => ctx.selectedColumns.length === 1, 'Selecione uma coluna'),
          dialogAction('groupBy', 'Agrupar Por', 'groupBy'),
          dialogAction('changeType', 'Tipo de Dados', 'changeType', hasSelection, 'Selecione uma coluna'),
          action('promoteHeaders', 'Usar 1ª Linha como Cabeçalho', ctx => ctx.addStep({ op: 'promoteHeaders', name: 'Cabeçalho Promovido' })),
          action('demoteHeaders', 'Usar Cabeçalhos como 1ª Linha', ctx => ctx.addStep({ op: 'demoteHeaders', name: 'Cabeçalhos Rebaixados' })),
          dialogAction('replaceValues', 'Substituir Valores', 'replaceValues', hasSelection, 'Selecione uma coluna'),
        ],
      },
      {
        title: 'Combinar',
        items: [dialogAction('mergeQueries', 'Mesclar Consultas', 'mergeQueries'), dialogAction('appendQueries', 'Acrescentar Consultas', 'appendQueries')],
      },
    ],
  };
}

// ---------------------------------------------------------------------------
// Aba "Transformar" (in-place: muda a coluna existente)
// ---------------------------------------------------------------------------
export function transformTab(): RibbonTabDef {
  return {
    id: 'transform',
    label: 'Transformar',
    groups: [
      {
        title: 'Tabela',
        items: [
          dialogAction('groupBy2', 'Agrupar Por', 'groupBy'),
          action('promoteHeaders2', 'Usar 1ª Linha como Cabeçalho', ctx => ctx.addStep({ op: 'promoteHeaders', name: 'Cabeçalho Promovido' })),
          action('transpose', 'Transpor', ctx => ctx.addStep({ op: 'transpose', name: 'Tabela Transposta' })),
          action('reverseRows', 'Inverter Linhas', ctx => ctx.addStep({ op: 'reverseRows', name: 'Linhas Invertidas' })),
          action('countRows', 'Contar Linhas', ctx => ctx.addStep({ op: 'groupBy', keys: [], aggregations: [{ name: 'Contagem', op: 'countRows' }], name: 'Linhas Contadas' })),
        ],
      },
      {
        title: 'Qualquer Coluna',
        items: [
          dialogAction('changeType2', 'Tipo de Dados', 'changeType', hasSelection, 'Selecione uma coluna'),
          action(
            'detectType',
            'Detectar Tipo',
            ctx => ctx.addStep({ op: 'changeType', columns: ctx.selectedColumns.map(column => ({ column, type: ctx.columnTypes[column] as EtlType })), name: 'Tipo Detectado' }),
            hasSelection,
            'Selecione uma coluna',
          ),
          action('rename', 'Renomear', ctx => {
            const col = ctx.selectedColumns[0];
            const name = window.prompt('Novo nome da coluna', col);
            if (name) ctx.addStep({ op: 'renameColumns', renames: [[col, name]], name: 'Nome Alterado' });
          }, ctx => ctx.selectedColumns.length === 1, 'Selecione uma coluna'),
          {
            kind: 'submenu',
            id: 'replaceValuesMenu',
            label: 'Substituir Valores',
            enabled: hasSelection,
            disabledReason: 'Selecione uma coluna',
            items: [
              dialogAction('replaceValues2', 'Substituir Valores', 'replaceValues'),
              action('replaceErrors', 'Substituir Erros', ctx => ctx.openParamDialog('Substituir Erros', [{ key: 'value', label: 'Novo valor', type: 'text', defaultValue: '' }], p => ctx.addStep({ op: 'replaceErrors', replacements: ctx.selectedColumns.map(column => ({ column, value: p.value })), name: 'Erros Substituídos' }))),
            ],
          },
          {
            kind: 'submenu',
            id: 'fillMenu',
            label: 'Preencher',
            enabled: hasSelection,
            disabledReason: 'Selecione uma coluna',
            items: [
              action('fillDown', 'Abaixo', ctx => ctx.addStep({ op: 'fillDown', columns: ctx.selectedColumns, name: 'Preenchido para Baixo' })),
              action('fillUp', 'Acima', ctx => ctx.addStep({ op: 'fillUp', columns: ctx.selectedColumns, name: 'Preenchido para Cima' })),
            ],
          },
          dialogAction('pivot', 'Dinamizar Coluna', 'pivot', ctx => ctx.selectedColumns.length === 1, 'Selecione uma coluna'),
          {
            kind: 'submenu',
            id: 'unpivotMenu',
            label: 'Transformar Colunas em Linhas',
            items: [
              action('unpivotSelected', 'Colunas Selecionadas', ctx => ctx.addStep({ op: 'unpivotColumns', columns: ctx.selectedColumns, attributeColumn: 'Atributo', valueColumn: 'Valor', name: 'Unpivot de Colunas' }), hasSelection, 'Selecione uma coluna'),
              action('unpivotOthers', 'Outras Colunas', ctx => ctx.addStep({ op: 'unpivotOtherColumns', keep: ctx.selectedColumns, attributeColumn: 'Atributo', valueColumn: 'Valor', name: 'Unpivot de Outras Colunas' }), hasSelection, 'Selecione uma coluna'),
              action(
                'unpivotOnlySelected',
                'Somente Selecionadas',
                ctx => ctx.addStep({ op: 'unpivotColumns', columns: ctx.selectedColumns, attributeColumn: 'Atributo', valueColumn: 'Valor', name: 'Unpivot de Colunas' }),
                hasSelection,
                'Selecione uma coluna',
              ),
            ],
          },
          {
            kind: 'submenu',
            id: 'moveMenu',
            label: 'Mover',
            enabled: hasSelection,
            disabledReason: 'Selecione uma coluna',
            items: [
              action('moveLeft', 'Esquerda', ctx => moveColumns(ctx, 'left')),
              action('moveRight', 'Direita', ctx => moveColumns(ctx, 'right')),
              action('moveStart', 'Para o Início', ctx => moveColumns(ctx, 'start')),
              action('moveEnd', 'Para o Fim', ctx => moveColumns(ctx, 'end')),
            ],
          },
        ],
      },
      { title: 'Coluna de Texto', items: [dialogAction('splitColumn2', 'Dividir', 'splitColumn', ctx => ctx.selectedColumns.length === 1, 'Selecione uma coluna'), textFormatMenu('inplace'), dialogAction('mergeColumns2', 'Mesclar Colunas', 'mergeColumns', ctx => ctx.selectedColumns.length >= 2, 'Selecione 2 ou mais colunas'), textExtractMenu('inplace')] },
      { title: 'Coluna de Número', items: [numberStandardMenu('inplace'), ...numberScientificMenus('inplace'), numberRoundingMenu('inplace'), numberInfoMenu('inplace')] },
      { title: 'Coluna de Data', items: dateMenu('inplace') },
    ],
  };
}

function moveColumns(ctx: RibbonCtx, dir: 'left' | 'right' | 'start' | 'end') {
  const order = [...ctx.allColumns];
  const sel = new Set(ctx.selectedColumns);
  const selectedInOrder = order.filter(c => sel.has(c));
  const rest = order.filter(c => !sel.has(c));
  let next: string[];
  if (dir === 'start') next = [...selectedInOrder, ...rest];
  else if (dir === 'end') next = [...rest, ...selectedInOrder];
  else {
    next = [...order];
    const step = dir === 'left' ? -1 : 1;
    const indices = selectedInOrder.map(c => next.indexOf(c));
    const edge = dir === 'left' ? Math.min(...indices) : Math.max(...indices);
    const targetEdge = edge + step;
    if (targetEdge < 0 || targetEdge >= next.length) return;
    // move o bloco selecionado inteiro uma posição na direção pedida
    const without = next.filter(c => !sel.has(c));
    const insertAt = dir === 'left' ? targetEdge : targetEdge - selectedInOrder.length + 1;
    const clampedAt = Math.max(0, Math.min(insertAt, without.length));
    next = [...without.slice(0, clampedAt), ...selectedInOrder, ...without.slice(clampedAt)];
  }
  ctx.addStep({ op: 'reorderColumns', columns: next, name: 'Colunas Reordenadas' });
}

// ---------------------------------------------------------------------------
// Aba "Adicionar Coluna" (sempre cria uma nova coluna)
// ---------------------------------------------------------------------------
export function addColumnTab(): RibbonTabDef {
  return {
    id: 'addColumn',
    label: 'Adicionar Coluna',
    groups: [
      {
        title: 'Geral',
        items: [
          dialogAction('customColumn', 'Coluna Personalizada', 'customColumn'),
          dialogAction('conditionalColumn', 'Coluna Condicional', 'conditionalColumn'),
          {
            kind: 'submenu',
            id: 'indexMenu',
            label: 'Coluna de Índice',
            items: [
              action('indexFrom0', 'De 0', ctx => ctx.addStep({ op: 'addIndexColumn', newColumn: 'Índice', start: 0, step: 1, name: 'Índice Adicionado' })),
              action('indexFrom1', 'De 1', ctx => ctx.addStep({ op: 'addIndexColumn', newColumn: 'Índice', start: 1, step: 1, name: 'Índice Adicionado' })),
              dialogAction('indexCustom', 'Personalizado', 'indexColumn'),
            ],
          },
          action(
            'duplicateColumn',
            'Duplicar Coluna',
            ctx => {
              const col = ctx.selectedColumns[0];
              ctx.addStep({ op: 'duplicateColumn', column: col, newColumn: `${col} - Cópia`, name: `Duplicar ${col}` });
            },
            ctx => ctx.selectedColumns.length === 1,
            'Selecione uma coluna',
          ),
        ],
      },
      {
        title: 'Texto',
        items: [
          textExtractMenu('new'),
          textFormatMenu('new'),
          action(
            'mergeColumnsNew',
            'Mesclar Colunas',
            ctx => ctx.openParamDialog('Mesclar Colunas', [{ key: 'separator', label: 'Separador', type: 'text', defaultValue: '-' }], p => {
              // Como no Power BI: qualquer tipo entra em Text.Combine (que
              // converte com Text.From e ignora nulos), nunca o operador &
              // puro, que exige texto nos dois lados.
              const items = ctx.selectedColumns.map(c => (ctx.columnTypes[c] === 'text' ? `[${c}]` : `Text.From([${c}])`));
              ctx.addStep({
                op: 'addCustomColumn',
                newColumn: 'Mesclado',
                expression: `Text.Combine({${items.join(', ')}}, "${p.separator}")`,
                type: 'text',
                name: 'Mesclado Adicionado',
              });
            }),
            ctx => ctx.selectedColumns.length >= 2,
            'Selecione 2 ou mais colunas',
          ),
        ],
      },
      { title: 'Número', items: [numberStandardMenu('new'), ...numberScientificMenus('new'), numberRoundingMenu('new'), numberInfoMenu('new')] },
      { title: 'Data', items: dateMenu('new') },
    ],
  };
}

// ---------------------------------------------------------------------------
// Aba "Exibir"
// ---------------------------------------------------------------------------
export interface ViewToggles {
  formulaBar: boolean;
  quality: boolean;
  distribution: boolean;
  profile: boolean;
}

export function viewTab(toggles: ViewToggles, onToggle: (key: keyof ViewToggles) => void): RibbonTabDef {
  const toggle = (key: keyof ViewToggles, label: string): MenuNode => ({
    kind: 'action',
    id: `toggle-${key}`,
    label: `${toggles[key] ? '✓ ' : ''}${label}`,
    run: () => onToggle(key),
  });
  return {
    id: 'view',
    label: 'Exibir',
    groups: [
      {
        title: 'Layout',
        items: [toggle('formulaBar', 'Barra de Fórmulas')],
      },
      {
        title: 'Perfil de Dados',
        items: [toggle('quality', 'Qualidade da Coluna'), toggle('distribution', 'Distribuição da Coluna'), toggle('profile', 'Perfil da Coluna')],
      },
      {
        title: 'Navegar',
        items: [dialogAction('goToColumn', 'Ir para Coluna', 'goToColumn'), dialogAction('advancedEditor2', 'Editor Avançado', 'advancedEditor')],
      },
    ],
  };
}
