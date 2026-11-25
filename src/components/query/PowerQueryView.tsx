import React, { useEffect, useMemo, useRef, useState } from 'react';
import { ChevronDown, Filter as FilterIcon } from 'lucide-react';
import { useBiStore } from '../../store/useBiStore';
import { EtlStep, EtlPreviewResult, Cond, DistributiveOmit, EtlType } from '../../types/etl';
import { etlPreview, etlToM, etlFromM, etlStepM, ETL_BROWSER_FALLBACK_NOTICE } from '../../services/etl/etlClient';
import { editStepFormula, FormulaBarEditError } from '../../services/etl/formulaBarEdit';
import { isTauriRuntime } from '../../bridge/tauriBridge';
import { ColumnTypeIcon, etlTypeFromDuckType } from './ColumnTypeIcon';
import { ColumnFilterMenu } from './ColumnFilterMenu';
import { ColumnContextMenu, ColumnMenuAction } from './ColumnContextMenu';
import { StepsPanel } from './StepsPanel';
import { ConsultasPane, QueryListEntry } from './ConsultasPane';
import { useColumnSelection } from './hooks/useColumnSelection';
import { planClearSort } from './sortUtils';
import { computeColumnQuality } from './profiling/quality';
import { computeColumnDistribution } from './profiling/distribution';
import { computeColumnProfile } from './profiling/profile';
import { QualityBar } from './profiling/QualityBar';
import { DistributionBar } from './profiling/DistributionBar';
import { ProfilePanel } from './profiling/ProfilePanel';
import { Ribbon } from './ribbon/Ribbon';
import { homeTab, transformTab, addColumnTab, viewTab, RibbonCtx, RibbonDialogKind, RibbonTabDef, ViewToggles } from './ribbon/menus';
import { ParamFieldSpec } from './ribbon/opDefs';
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
import { ParamDialog } from './dialogs/ParamDialog';
import { ChooseColumnsDialog } from './dialogs/ChooseColumnsDialog';
import { GoToColumnDialog } from './dialogs/GoToColumnDialog';

let idCounter = 0;
function newStepId(): string {
  idCounter += 1;
  return `step_${Date.now().toString(36)}_${idCounter}`;
}

type DialogKind = RibbonDialogKind | null;

export const PowerQueryView: React.FC = () => {
  const {
    tables,
    appliedSteps,
    getOrInitQuery,
    ensureQuery,
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
  const [formulaBarError, setFormulaBarError] = useState<string | null>(null);
  const [advancedM, setAdvancedM] = useState('');
  const [advancedWarnings, setAdvancedWarnings] = useState<string[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [activeTabId, setActiveTabId] = useState('home');
  const [paramDialog, setParamDialog] = useState<{ title: string; fields: ParamFieldSpec[]; onSubmit: (v: Record<string, any>) => void } | null>(null);
  const [toggles, setToggles] = useState<ViewToggles>({ formulaBar: true, quality: true, distribution: true, profile: true });
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const query = getOrInitQuery(selectedTableId);
  const selectedStepId = selectedStepByTable[selectedTableId];
  const uptoStep = selectedStepId ? query.steps.findIndex(s => s.id === selectedStepId) : query.steps.length - 1;

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2500);
  };

  // Tabelas de .pbip: converte o M em etapas editáveis na primeira abertura.
  useEffect(() => {
    if (selectedTableId) void ensureQuery(selectedTableId);
  }, [selectedTableId, ensureQuery]);

  const runPreview = () => {
    if (!selectedTableId) return;
    etlPreview({ query, uptoStep: uptoStep >= 0 ? uptoStep : undefined, limit: 200 })
      .then(setPreview)
      .catch((e: any) => setPreview({ columns: [], rows: [], rowCount: 0, warnings: [e?.message || 'Erro ao gerar preview'] }));
  };

  useEffect(() => {
    if (!selectedTableId) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(runPreview, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(query), uptoStep, selectedTableId]);

  useEffect(() => {
    const idx = query.steps.findIndex(s => s.id === selectedStepId);
    if (idx < 0) {
      setFormulaBarText('');
      setFormulaBarError(null);
      return;
    }
    etlStepM({ query, index: idx })
      .then(m => {
        setFormulaBarText(m);
        setFormulaBarError(null);
      })
      .catch(() => setFormulaBarText(''));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedStepId, selectedTableId]);

  const applyFormulaBarEdit = async (text: string) => {
    const idx = query.steps.findIndex(s => s.id === selectedStepId);
    if (idx < 0) return;
    try {
      const { query: nextQuery } = await editStepFormula(query, idx, text);
      setQuery(selectedTableId, nextQuery);
      setFormulaBarError(null);
    } catch (e: any) {
      setFormulaBarError(e instanceof FormulaBarEditError ? e.message : e?.message ?? 'Erro ao editar a fórmula');
    }
  };

  const columns = preview.columns.length
    ? preview.columns
    : (tables.find(t => t.id === selectedTableId)?.columns || []).map(c => ({ name: c.name, type: c.dataType }));
  const columnNames = columns.map(c => c.name);
  const columnTypes: Record<string, EtlType> = useMemo(() => {
    const map: Record<string, EtlType> = {};
    columns.forEach(c => {
      map[c.name] = (['text', 'integer', 'decimal', 'currency', 'date', 'datetime', 'boolean'].includes(c.type) ? (c.type as EtlType) : etlTypeFromDuckType(c.type));
    });
    return map;
  }, [columns]);

  const { selected: selectedColumns, isSelected, handleColumnClick, clearSelection, setSelected: setSelectedColumns } = useColumnSelection(columnNames);

  // Troca de consulta: limpa a seleção múltipla para não vazar colunas de outra tabela.
  useEffect(() => {
    clearSelection();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedTableId]);

  const otherTables = tables
    .filter(t => t.id !== selectedTableId)
    .map(t => ({ id: t.id, name: t.originalName || t.name, columns: t.columns.map(c => c.name) }));

  const queryEntries: QueryListEntry[] = tables.map(t => ({
    id: t.id,
    name: t.originalName || t.name,
    stepCount: (appliedSteps[t.id]?.steps || []).length,
  }));

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

  const handleSort = (column: string, sort: { column: string; descending: boolean } | null) => {
    if (sort) {
      addStep({ op: 'sort', by: [{ column, descending: sort.descending }], name: 'Classificado' });
      return;
    }
    const plan = planClearSort(query.steps, column);
    if (!plan) return;
    if (plan.action === 'remove') removeQueryStep(selectedTableId, plan.stepId);
    else updateQueryStep(selectedTableId, plan.stepId, { by: plan.by } as Partial<EtlStep>);
  };

  const ribbonCtx: RibbonCtx = {
    selectedColumns,
    columnTypes,
    allColumns: columnNames,
    addStep,
    openDialog: kind => {
      if (kind === 'chooseColumns' || kind === 'goToColumn') setActiveColumn(selectedColumns[0] || activeColumn);
      setDialog(kind);
    },
    openParamDialog: (title, fields, onSubmit) => setParamDialog({ title, fields, onSubmit }),
  };

  const tabs: RibbonTabDef[] = [
    homeTab({ onCloseAndApply: () => closeAndApplyQuery(selectedTableId).then(() => showToast('Consulta aplicada')), onRefreshPreview: () => runPreview() }),
    transformTab(),
    addColumnTab(),
    viewTab(toggles, key => setToggles(t => ({ ...t, [key]: !t[key] }))),
  ];

  const headerContextActions = (col: string): ColumnMenuAction[] => {
    const cols = selectedColumns.includes(col) && selectedColumns.length > 1 ? selectedColumns : [col];
    return [
      { label: 'Remover', onClick: () => addStep({ op: 'removeColumns', columns: cols, name: 'Colunas Removidas' }) },
      { label: 'Remover Outras Colunas', onClick: () => addStep({ op: 'selectColumns', columns: cols, name: 'Outras Colunas Removidas' }) },
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
          { label: 'Texto', onClick: () => addStep({ op: 'changeType', columns: cols.map(c => ({ column: c, type: 'text' as const })), name: 'Tipo Alterado' }) },
          { label: 'Número Inteiro', onClick: () => addStep({ op: 'changeType', columns: cols.map(c => ({ column: c, type: 'integer' as const })), name: 'Tipo Alterado' }) },
          { label: 'Decimal', onClick: () => addStep({ op: 'changeType', columns: cols.map(c => ({ column: c, type: 'decimal' as const })), name: 'Tipo Alterado' }) },
          { label: 'Data', onClick: () => addStep({ op: 'changeType', columns: cols.map(c => ({ column: c, type: 'date' as const })), name: 'Tipo Alterado' }) },
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
          { label: 'Para Baixo', onClick: () => addStep({ op: 'fillDown', columns: cols, name: 'Preenchido para Baixo' }) },
          { label: 'Para Cima', onClick: () => addStep({ op: 'fillUp', columns: cols, name: 'Preenchido para Cima' }) },
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
            keep: cols,
            attributeColumn: 'Atributo',
            valueColumn: 'Valor',
            name: 'Unpivot de Outras Colunas',
          }),
      },
    ];
  };

  const profileColumn = activeColumn && columnNames.includes(activeColumn) ? activeColumn : columnNames[0];
  const profile = profileColumn ? computeColumnProfile(preview.rows, profileColumn, columnTypes[profileColumn]) : null;

  return (
    <div className="flex flex-col h-full bg-slate-50 dark:bg-slate-950 text-xs" data-testid="power-query-view">
      <Ribbon tabs={tabs} activeTabId={activeTabId} onTabChange={setActiveTabId} ctx={ribbonCtx} />

      {toggles.formulaBar && (
        <div className="flex items-center gap-2 px-3 py-1.5 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <span className="font-mono text-slate-400">fx</span>
          <input
            value={formulaBarText}
            onChange={e => {
              setFormulaBarText(e.target.value);
              setFormulaBarError(null);
            }}
            onKeyDown={e => {
              if (e.key === 'Enter') void applyFormulaBarEdit(formulaBarText);
            }}
            className={`flex-1 bg-slate-50 dark:bg-slate-800 border rounded px-2 py-1 font-mono text-[11px] text-slate-700 dark:text-slate-200 ${
              formulaBarError ? 'border-red-500' : 'border-slate-200 dark:border-slate-700'
            }`}
          />
        </div>
      )}
      {formulaBarError && (
        <div className="px-3 py-1 bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-200 text-[11px] border-b border-red-200 dark:border-red-800">
          {formulaBarError}
        </div>
      )}

      {!isTauriRuntime() && (
        <div className="px-3 py-1 bg-amber-50 dark:bg-amber-900/30 text-amber-800 dark:text-amber-200 text-[11px] border-b border-amber-200 dark:border-amber-800">
          {ETL_BROWSER_FALLBACK_NOTICE}
        </div>
      )}

      <div className="flex flex-1 min-h-0">
        <ConsultasPane queries={queryEntries} selectedId={selectedTableId} onSelect={setSelectedTableId} />

        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex-1 overflow-auto">
            <table className="min-w-full border-collapse text-[11px]">
              <thead>
                <tr>
                  {columns.map(col => {
                    const quality = toggles.quality ? computeColumnQuality(preview.rows, col.name) : null;
                    const distribution = toggles.distribution ? computeColumnDistribution(preview.rows, col.name) : null;
                    const selectedHere = isSelected(col.name);
                    return (
                      <th
                        key={col.name}
                        data-testid={`col-header-${col.name}`}
                        className={`relative border-b border-r border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900 px-2 py-1.5 text-left cursor-pointer align-top ${
                          selectedHere ? 'ring-2 ring-inset ring-amber-400 bg-amber-50 dark:bg-amber-900/30' : activeColumn === col.name ? 'ring-1 ring-inset ring-amber-300' : ''
                        }`}
                        onClick={e => {
                          handleColumnClick(col.name, { ctrlKey: e.ctrlKey, metaKey: e.metaKey, shiftKey: e.shiftKey });
                          setActiveColumn(col.name);
                        }}
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
                        {quality && <QualityBar quality={quality} />}
                        {distribution && <DistributionBar distribution={distribution} />}
                        {openMenu?.column === col.name && openMenu.type === 'filter' && (
                          <ColumnFilterMenu
                            column={col.name}
                            rows={preview.rows}
                            onClose={() => setOpenMenu(null)}
                            onApply={(condition: Cond) => addStep({ op: 'filterRows', condition, name: 'Linhas Filtradas' })}
                            onSort={sort => handleSort(col.name, sort)}
                          />
                        )}
                        {openMenu?.column === col.name && openMenu.type === 'context' && (
                          <ColumnContextMenu column={col.name} onClose={() => setOpenMenu(null)} actions={headerContextActions(col.name)} />
                        )}
                      </th>
                    );
                  })}
                </tr>
              </thead>
              <tbody>
                {preview.rows.map((row, i) => (
                  <tr key={i} className="odd:bg-white even:bg-slate-50 dark:odd:bg-slate-950 dark:even:bg-slate-900">
                    {columns.map(col => (
                      <td
                        key={col.name}
                        className={`border-r border-b border-slate-100 dark:border-slate-800 px-2 py-1 whitespace-nowrap ${
                          isSelected(col.name) ? 'bg-amber-50/60 dark:bg-amber-900/10' : ''
                        }`}
                      >
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

          {toggles.profile && profile && profileColumn && <ProfilePanel column={profileColumn} type={columnTypes[profileColumn]} profile={profile} />}
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

      {paramDialog && (
        <ParamDialog
          title={paramDialog.title}
          fields={paramDialog.fields}
          onClose={() => setParamDialog(null)}
          onApply={values => {
            paramDialog.onSubmit(values);
            setParamDialog(null);
          }}
        />
      )}

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
          column={activeColumn || selectedColumns[0] || ''}
          onClose={() => {
            setDialog(null);
            setEditingStepId(null);
          }}
          onApply={args => updateOrAddStep({ op: 'splitColumn', column: activeColumn || selectedColumns[0], ...args })}
        />
      )}
      {dialog === 'mergeColumns' && (
        <MergeColumnsDialog
          columns={selectedColumns.length >= 2 ? selectedColumns : columnNames}
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
          onApply={args => updateOrAddStep({ op: 'replaceValues', columns: selectedColumns.length ? selectedColumns : [activeColumn], ...args })}
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
          columns={columns}
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
      {dialog === 'chooseColumns' && (
        <ChooseColumnsDialog
          columns={columnNames}
          onClose={() => setDialog(null)}
          onApply={cols => addStep({ op: 'selectColumns', columns: cols, name: 'Colunas Escolhidas' })}
        />
      )}
      {dialog === 'goToColumn' && (
        <GoToColumnDialog
          columns={columnNames}
          onClose={() => setDialog(null)}
          onSelect={col => {
            setActiveColumn(col);
            setSelectedColumns([col]);
          }}
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
