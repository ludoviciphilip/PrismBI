import React, { useEffect, useRef, useState } from 'react';
import {
  RefreshCw,
  CheckCircle,
  Code2,
  Filter as FilterIcon,
  Rows,
  Columns as ColumnsIcon,
  Type,
  Combine,
  Replace,
  Group,
  PanelsTopLeft,
  GitMerge,
  ListPlus,
  SplitSquareHorizontal,
  AlignVerticalJustifyCenter,
  ListOrdered,
  Sparkles,
  ChevronDown,
  Trash2,
} from 'lucide-react';
import { useBiStore } from '../../store/useBiStore';
import { EtlStep, EtlPreviewResult, Cond, DistributiveOmit } from '../../types/etl';
import { etlPreview, etlToM, etlFromM, ETL_BROWSER_FALLBACK_NOTICE } from '../../services/etl/etlClient';
import { isTauriRuntime } from '../../bridge/tauriBridge';
import { ColumnTypeIcon } from './ColumnTypeIcon';
import { ColumnFilterMenu } from './ColumnFilterMenu';
import { ColumnContextMenu, ColumnMenuAction } from './ColumnContextMenu';
import { StepsPanel } from './StepsPanel';
import { ChangeTypeDialog } from './dialogs/ChangeTypeDialog';
import { FilterRowsDialog } from './dialogs/FilterRowsDialog';
import { GroupByDialog } from './dialogs/GroupByDialog';
import { MergeQueriesDialog } from './dialogs/MergeQueriesDialog';
import { SplitColumnDialog } from './dialogs/SplitColumnDialog';
import { MergeColumnsDialog } from './dialogs/MergeColumnsDialog';
import { ReplaceValuesDialog } from './dialogs/ReplaceValuesDialog';
import { CustomColumnDialog } from './dialogs/CustomColumnDialog';
import { ConditionalColumnDialog } from './dialogs/ConditionalColumnDialog';
import { IndexColumnDialog } from './dialogs/IndexColumnDialog';
import { AppendQueriesDialog } from './dialogs/AppendQueriesDialog';
import { PivotDialog } from './dialogs/PivotDialog';
import { AdvancedEditorModal } from './dialogs/AdvancedEditorModal';

let idCounter = 0;
function newStepId(): string {
  idCounter += 1;
  return `step_${Date.now().toString(36)}_${idCounter}`;
}

type DialogKind =
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
  | null;

export const PowerQueryView: React.FC = () => {
  const {
    tables,
    appliedSteps,
    getOrInitQuery,
    setQuery,
    addQueryStep,
    updateQueryStep,
    removeQueryStep,
    moveQueryStepUp,
    moveQueryStepDown,
    renameQueryStep,
    selectQueryStep,
    selectedStepByTable,
    closeAndApplyQuery,
    refreshTableData,
  } = useBiStore();

  const [selectedTableId, setSelectedTableId] = useState<string>(tables[0]?.id || '');
  const [activeColumn, setActiveColumn] = useState<string>('');
  const [openMenu, setOpenMenu] = useState<{ type: 'filter' | 'context'; column: string } | null>(null);
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [editingStepId, setEditingStepId] = useState<string | null>(null);
  const [preview, setPreview] = useState<EtlPreviewResult>({ columns: [], rows: [], rowCount: 0, warnings: [] });
  const [formulaBarText, setFormulaBarText] = useState('');
  const [advancedM, setAdvancedM] = useState('');
  const [advancedWarnings, setAdvancedWarnings] = useState<string[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const query = getOrInitQuery(selectedTableId);
  const selectedStepId = selectedStepByTable[selectedTableId];
  const uptoStep = selectedStepId ? query.steps.findIndex(s => s.id === selectedStepId) : query.steps.length - 1;

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2500);
  };

  useEffect(() => {
    if (!selectedTableId) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const result = await etlPreview({ query, uptoStep: uptoStep >= 0 ? uptoStep : undefined, limit: 200 });
        setPreview(result);
      } catch (e: any) {
        setPreview({ columns: [], rows: [], rowCount: 0, warnings: [e?.message || 'Erro ao gerar preview'] });
      }
    }, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(query), uptoStep, selectedTableId]);

  useEffect(() => {
    const step = query.steps.find(s => s.id === selectedStepId);
    if (!step) {
      setFormulaBarText('');
      return;
    }
    etlToM({ query: { ...query, steps: [step] } })
      .then(setFormulaBarText)
      .catch(() => setFormulaBarText(''));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedStepId, selectedTableId]);

  const columns = preview.columns.length
    ? preview.columns
    : (tables.find(t => t.id === selectedTableId)?.columns || []).map(c => ({ name: c.name, type: c.dataType }));
  const columnNames = columns.map(c => c.name);

  const otherTables = tables
    .filter(t => t.id !== selectedTableId)
    .map(t => ({ id: t.id, name: t.originalName || t.name, columns: t.columns.map(c => c.name) }));

  const addStep = (step: DistributiveOmit<EtlStep, 'id'>) => {
    addQueryStep(selectedTableId, { ...step, id: newStepId() } as EtlStep);
    setDialog(null);
  };

  const openEditDialog = (stepId: string) => {
    const step = query.steps.find(s => s.id === stepId);
    if (!step) return;
    setEditingStepId(stepId);
    const dialogForOp: Record<string, DialogKind> = {
      changeType: 'changeType',
      filterRows: 'filterRows',
      groupBy: 'groupBy',
      mergeQueries: 'mergeQueries',
      splitColumn: 'splitColumn',
      mergeColumns: 'mergeColumns',
      replaceValues: 'replaceValues',
      addCustomColumn: 'customColumn',
      addConditionalColumn: 'conditionalColumn',
      addIndexColumn: 'indexColumn',
      appendQueries: 'appendQueries',
      pivot: 'pivot',
    };
    setDialog(dialogForOp[step.op] || null);
  };

  const updateOrAddStep = (opBuilder: DistributiveOmit<EtlStep, 'id' | 'name'> & { name?: string }) => {
    if (editingStepId) {
      updateQueryStep(selectedTableId, editingStepId, opBuilder as Partial<EtlStep>);
      setEditingStepId(null);
    } else {
      addQueryStep(selectedTableId, { ...opBuilder, id: newStepId(), name: opBuilder.name || defaultStepName(opBuilder.op as string) } as EtlStep);
    }
    setDialog(null);
  };

  const openAdvancedEditor = async () => {
    try {
      const m = await etlToM({ query });
      setAdvancedM(m);
      setAdvancedWarnings([]);
      setDialog('advancedEditor');
    } catch (e: any) {
      showToast(e?.message || 'Falha ao gerar M');
    }
  };

  const applyAdvancedEditor = async (m: string) => {
    try {
      const result = await etlFromM({ name: query.name, m });
      setQuery(selectedTableId, result.query);
      setAdvancedWarnings(result.warnings);
      if (result.warnings.length) showToast(result.warnings[0]);
      setDialog(null);
    } catch (e: any) {
      showToast(e?.message || 'Falha ao aplicar M');
    }
  };

  const headerContextActions = (col: string): ColumnMenuAction[] => [
    { label: 'Remover', onClick: () => addStep({ op: 'removeColumns', columns: [col], name: 'Colunas Removidas' }) },
    { label: 'Remover Outras Colunas', onClick: () => addStep({ op: 'selectColumns', columns: [col], name: 'Outras Colunas Removidas' }) },
    { label: 'Duplicar', onClick: () => addStep({ op: 'duplicateColumn', column: col, newColumn: `${col} - Cópia`, name: `Duplicar ${col}` }) },
    {
      label: 'Renomear',
      onClick: () => {
        const name = window.prompt('Novo nome da coluna', col);
        if (name) addStep({ op: 'renameColumns', renames: [[col, name]], name: 'Nome Alterado' });
      },
    },
    {
      label: 'Alterar Tipo',
      onClick: () => {},
      submenu: [
        { label: 'Texto', onClick: () => addStep({ op: 'changeType', columns: [{ column: col, type: 'text' }], name: 'Tipo Alterado' }) },
        { label: 'Número Inteiro', onClick: () => addStep({ op: 'changeType', columns: [{ column: col, type: 'integer' }], name: 'Tipo Alterado' }) },
        { label: 'Decimal', onClick: () => addStep({ op: 'changeType', columns: [{ column: col, type: 'decimal' }], name: 'Tipo Alterado' }) },
        { label: 'Data', onClick: () => addStep({ op: 'changeType', columns: [{ column: col, type: 'date' }], name: 'Tipo Alterado' }) },
        {
          label: '(mais opções…)',
          onClick: () => {
            setActiveColumn(col);
            setDialog('changeType');
          },
        },
      ],
    },
    {
      label: 'Substituir Valores',
      onClick: () => {
        setActiveColumn(col);
        setDialog('replaceValues');
      },
    },
    {
      label: 'Preencher',
      onClick: () => {},
      submenu: [
        { label: 'Para Baixo', onClick: () => addStep({ op: 'fillDown', columns: [col], name: 'Preenchido para Baixo' }) },
        { label: 'Para Cima', onClick: () => addStep({ op: 'fillUp', columns: [col], name: 'Preenchido para Cima' }) },
      ],
    },
    {
      label: 'Dividir Coluna',
      onClick: () => {
        setActiveColumn(col);
        setDialog('splitColumn');
      },
    },
    {
      label: 'Agrupar Por',
      onClick: () => {
        setActiveColumn(col);
        setDialog('groupBy');
      },
    },
    {
      label: 'Unpivot',
      onClick: () =>
        addStep({
          op: 'unpivotOtherColumns',
          keep: [col],
          attributeColumn: 'Atributo',
          valueColumn: 'Valor',
          name: 'Unpivot de Outras Colunas',
        }),
    },
  ];

  return (
    <div className="flex flex-col h-full bg-slate-50 dark:bg-slate-950 text-xs" data-testid="power-query-view">
      <div className="border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-3 py-2 flex items-center gap-4 flex-wrap">
        <RibbonGroup title="Página Inicial">
          <RibbonButton icon={RefreshCw} label="Atualizar Dados" onClick={() => refreshTableData(selectedTableId).then(() => showToast('Dados atualizados'))} />
          <RibbonButton icon={CheckCircle} label="Fechar e Aplicar" onClick={() => closeAndApplyQuery(selectedTableId).then(() => showToast('Consulta aplicada'))} />
          <RibbonButton icon={Code2} label="Editor Avançado" onClick={openAdvancedEditor} />
          <RibbonButton icon={GitMerge} label="Mesclar Consultas" onClick={() => setDialog('mergeQueries')} />
          <RibbonButton icon={ListPlus} label="Acrescentar Consultas" onClick={() => setDialog('appendQueries')} />
        </RibbonGroup>
        <RibbonGroup title="Transformar">
          <RibbonButton icon={PanelsTopLeft} label="1ª Linha como Cabeçalho" onClick={() => addStep({ op: 'promoteHeaders', name: 'Cabeçalho Promovido' })} />
          <RibbonButton icon={Rows} label="Remover Linhas em Branco" onClick={() => addStep({ op: 'removeBlankRows', name: 'Linhas em Branco Removidas' })} />
          <RibbonButton icon={Trash2} label="Remover Duplicadas" onClick={() => addStep({ op: 'removeDuplicates', name: 'Duplicadas Removidas' })} />
          <RibbonButton icon={Type} label="Alterar Tipo" onClick={() => activeColumn && setDialog('changeType')} disabled={!activeColumn} />
          <RibbonButton icon={SplitSquareHorizontal} label="Dividir Coluna" onClick={() => activeColumn && setDialog('splitColumn')} disabled={!activeColumn} />
          <RibbonButton icon={Combine} label="Mesclar Colunas" onClick={() => setDialog('mergeColumns')} />
          <RibbonButton icon={Replace} label="Substituir Valores" onClick={() => activeColumn && setDialog('replaceValues')} disabled={!activeColumn} />
          <RibbonButton icon={Group} label="Agrupar Por" onClick={() => setDialog('groupBy')} />
          <RibbonButton icon={AlignVerticalJustifyCenter} label="Dinamizar Coluna" onClick={() => setDialog('pivot')} />
          <RibbonButton icon={FilterIcon} label="Filtrar Linhas" onClick={() => setDialog('filterRows')} />
        </RibbonGroup>
        <RibbonGroup title="Adicionar Coluna">
          <RibbonButton icon={Sparkles} label="Coluna Personalizada" onClick={() => setDialog('customColumn')} />
          <RibbonButton icon={ColumnsIcon} label="Coluna Condicional" onClick={() => setDialog('conditionalColumn')} />
          <RibbonButton icon={ListOrdered} label="Coluna de Índice" onClick={() => setDialog('indexColumn')} />
        </RibbonGroup>
        <div className="ml-auto">
          <select
            value={selectedTableId}
            onChange={e => setSelectedTableId(e.target.value)}
            className="border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1 bg-slate-50 dark:bg-slate-800 font-semibold"
          >
            {tables.map(t => (
              <option key={t.id} value={t.id}>
                {t.originalName || t.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
        <span className="font-mono text-slate-400">fx</span>
        <input
          readOnly
          value={formulaBarText}
          className="flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded px-2 py-1 font-mono text-[11px] text-slate-700 dark:text-slate-200"
        />
      </div>

      {!isTauriRuntime() && (
        <div className="px-3 py-1 bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-200 text-[11px] border-b border-amber-200 dark:border-amber-800">
          {ETL_BROWSER_FALLBACK_NOTICE}
        </div>
      )}

      <div className="flex flex-1 min-h-0">
        <div className="flex-1 overflow-auto">
          <table className="min-w-full border-collapse text-[11px]">
            <thead>
              <tr>
                {columns.map(col => (
                  <th
                    key={col.name}
                    className={`relative border-b border-r border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900 px-2 py-1.5 text-left cursor-pointer ${
                      activeColumn === col.name ? 'ring-1 ring-inset ring-amber-400' : ''
                    }`}
                    onClick={() => setActiveColumn(col.name)}
                  >
                    <div className="flex items-center gap-1">
                      <ColumnTypeIcon type={col.type} />
                      <span className="font-semibold">{col.name}</span>
                      <button
                        data-testid={`filter-btn-${col.name}`}
                        onClick={e => {
                          e.stopPropagation();
                          setOpenMenu(openMenu?.column === col.name && openMenu.type === 'filter' ? null : { type: 'filter', column: col.name });
                        }}
                        className="ml-auto"
                      >
                        <FilterIcon className="w-3 h-3 text-slate-400" />
                      </button>
                      <button
                        data-testid={`menu-btn-${col.name}`}
                        onClick={e => {
                          e.stopPropagation();
                          setOpenMenu(openMenu?.column === col.name && openMenu.type === 'context' ? null : { type: 'context', column: col.name });
                        }}
                      >
                        <ChevronDown className="w-3 h-3 text-slate-400" />
                      </button>
                    </div>
                    {openMenu?.column === col.name && openMenu.type === 'filter' && (
                      <ColumnFilterMenu
                        column={col.name}
                        rows={preview.rows}
                        onClose={() => setOpenMenu(null)}
                        onApply={(condition: Cond) => addStep({ op: 'filterRows', condition, name: 'Linhas Filtradas' })}
                      />
                    )}
                    {openMenu?.column === col.name && openMenu.type === 'context' && (
                      <ColumnContextMenu column={col.name} onClose={() => setOpenMenu(null)} actions={headerContextActions(col.name)} />
                    )}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {preview.rows.map((row, i) => (
                <tr key={i} className="odd:bg-white even:bg-slate-50 dark:odd:bg-slate-950 dark:even:bg-slate-900">
                  {columns.map(col => (
                    <td key={col.name} className="border-r border-b border-slate-100 dark:border-slate-800 px-2 py-1 whitespace-nowrap">
                      {String(row[col.name] ?? '')}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
          {preview.failedStep && (
            <div className="px-3 py-2 bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-200 border-t border-red-200 dark:border-red-800">
              Etapa com erro ({preview.failedStep.index + 1}): {preview.failedStep.message}
            </div>
          )}
          <div className="px-3 py-1 text-slate-400">{preview.rowCount} linhas</div>
        </div>

        <div className="w-64 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shrink-0">
          <StepsPanel
            steps={query.steps}
            selectedStepId={selectedStepId}
            failedStep={preview.failedStep}
            onSelect={stepId => selectQueryStep(selectedTableId, stepId)}
            onRename={(stepId, name) => renameQueryStep(selectedTableId, stepId, name)}
            onDelete={stepId => removeQueryStep(selectedTableId, stepId)}
            onMoveUp={stepId => moveQueryStepUp(selectedTableId, stepId)}
            onMoveDown={stepId => moveQueryStepDown(selectedTableId, stepId)}
            onEdit={openEditDialog}
          />
        </div>
      </div>

      {toast && <div className="fixed bottom-4 right-4 bg-slate-900 text-white px-3 py-2 rounded-lg shadow-xl text-[11px]">{toast}</div>}

      {dialog === 'changeType' && (
        <ChangeTypeDialog
          column={activeColumn}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={type => updateOrAddStep({ op: 'changeType', columns: [{ column: activeColumn, type }] })}
        />
      )}
      {dialog === 'filterRows' && (
        <FilterRowsDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={condition => updateOrAddStep({ op: 'filterRows', condition })}
        />
      )}
      {dialog === 'groupBy' && (
        <GroupByDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={({ keys, aggregations }) => updateOrAddStep({ op: 'groupBy', keys, aggregations })}
        />
      )}
      {dialog === 'mergeQueries' && (
        <MergeQueriesDialog
          leftColumns={columnNames}
          availableTables={otherTables}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={result => updateOrAddStep({ op: 'mergeQueries', ...result, name: 'Consultas Mescladas' })}
        />
      )}
      {dialog === 'splitColumn' && (
        <SplitColumnDialog
          column={activeColumn}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'splitColumn', column: activeColumn, ...args })}
        />
      )}
      {dialog === 'mergeColumns' && (
        <MergeColumnsDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'mergeColumns', ...args })}
        />
      )}
      {dialog === 'replaceValues' && (
        <ReplaceValuesDialog
          column={activeColumn}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'replaceValues', columns: [activeColumn], ...args })}
        />
      )}
      {dialog === 'customColumn' && (
        <CustomColumnDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'addCustomColumn', ...args })}
        />
      )}
      {dialog === 'conditionalColumn' && (
        <ConditionalColumnDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={({ newColumn, rules, otherwise }) => updateOrAddStep({ op: 'addConditionalColumn', newColumn, rules, otherwise })}
        />
      )}
      {dialog === 'indexColumn' && (
        <IndexColumnDialog
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'addIndexColumn', ...args })}
        />
      )}
      {dialog === 'appendQueries' && (
        <AppendQueriesDialog
          availableTables={otherTables}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={queries => updateOrAddStep({ op: 'appendQueries', queries, name: 'Consultas Acrescentadas' })}
        />
      )}
      {dialog === 'pivot' && (
        <PivotDialog
          columns={columnNames}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'pivot', ...args })}
        />
      )}
      {dialog === 'advancedEditor' && <AdvancedEditorModal m={advancedM} warnings={advancedWarnings} onClose={() => setDialog(null)} onApply={applyAdvancedEditor} />}
    </div>
  );
};

function defaultStepName(op: string): string {
  const names: Record<string, string> = {
    changeType: 'Tipo Alterado',
    filterRows: 'Linhas Filtradas',
    groupBy: 'Agrupadas Linhas',
    mergeQueries: 'Consultas Mescladas',
    splitColumn: 'Coluna Dividida',
    mergeColumns: 'Colunas Mescladas',
    replaceValues: 'Valores Substituídos',
    addCustomColumn: 'Coluna Personalizada Adicionada',
    addConditionalColumn: 'Coluna Condicional Adicionada',
    addIndexColumn: 'Índice Adicionado',
    appendQueries: 'Consultas Acrescentadas',
    pivot: 'Coluna Dinamizada',
  };
  return names[op] || op;
}

const RibbonGroup: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <div className="flex flex-col">
    <div className="flex items-center gap-1">{children}</div>
    <div className="text-center text-[10px] text-slate-400 mt-1">{title}</div>
  </div>
);

const RibbonButton: React.FC<{ icon: React.FC<{ className?: string }>; label: string; onClick: () => void; disabled?: boolean }> = ({
  icon: Icon,
  label,
  onClick,
  disabled,
}) => (
  <button
    onClick={onClick}
    disabled={disabled}
    title={label}
    className="flex flex-col items-center justify-center gap-0.5 px-2 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed min-w-[56px]"
  >
    <Icon className="w-4 h-4 text-slate-600 dark:text-slate-300" />
    <span className="text-[9px] text-slate-500 dark:text-slate-400 leading-tight text-center">{label}</span>
  </button>
);
