import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls, selectCls } from './DialogShell';
import { Cond, Scalar } from '../../../types/etl';

interface SimpleRule {
  column: string;
  op: Cond extends infer C ? (C extends { op: infer O } ? O : never) : never;
  value: string;
}

const OPS: { value: string; label: string; needsValue: boolean }[] = [
  { value: 'eq', label: 'é igual a', needsValue: true },
  { value: 'neq', label: 'é diferente de', needsValue: true },
  { value: 'gt', label: 'é maior que', needsValue: true },
  { value: 'gte', label: 'é maior ou igual a', needsValue: true },
  { value: 'lt', label: 'é menor que', needsValue: true },
  { value: 'lte', label: 'é menor ou igual a', needsValue: true },
  { value: 'contains', label: 'contém', needsValue: true },
  { value: 'notContains', label: 'não contém', needsValue: true },
  { value: 'startsWith', label: 'começa com', needsValue: true },
  { value: 'endsWith', label: 'termina com', needsValue: true },
  { value: 'isNull', label: 'é nulo', needsValue: false },
  { value: 'isNotNull', label: 'não é nulo', needsValue: false },
  { value: 'isBlank', label: 'está em branco', needsValue: false },
  { value: 'isNotBlank', label: 'não está em branco', needsValue: false },
];

function ruleToCond(rule: SimpleRule): Cond {
  const opDef = OPS.find(o => o.value === rule.op);
  if (!opDef?.needsValue) {
    return { column: rule.column, op: rule.op as any };
  }
  const num = Number(rule.value);
  const value: Scalar = rule.value !== '' && !Number.isNaN(num) && /^-?\d+(\.\d+)?$/.test(rule.value) ? num : rule.value;
  return { column: rule.column, op: rule.op as any, value };
}

/** Inverso de `ruleToCond`: um `Cond` salvo -> estado editável do construtor básico. */
function condToRule(cond: Extract<Cond, { column: string }>): SimpleRule {
  const value = 'value' in cond ? String(cond.value ?? '') : '';
  return { column: cond.column, op: cond.op as any, value };
}

function hasColumn(cond: Cond): cond is Extract<Cond, { column: string }> {
  return 'column' in cond;
}

/** Só o construtor básico sabe editar `and`/`or` de cláusulas simples; o resto cai pro JSON. */
function condToRules(cond: Cond): SimpleRule[] | null {
  if ('and' in cond) return cond.and.every(hasColumn) ? cond.and.map(condToRule) : null;
  if ('or' in cond) return cond.or.every(hasColumn) ? cond.or.map(condToRule) : null;
  if (hasColumn(cond)) return [condToRule(cond)];
  return null;
}

function combinatorOf(cond: Cond): 'and' | 'or' {
  return 'or' in cond ? 'or' : 'and';
}

export const FilterRowsDialog: React.FC<{
  columns: string[];
  initialCondition?: Cond;
  onClose: () => void;
  onApply: (condition: Cond) => void;
}> = ({ columns, initialCondition, onClose, onApply }) => {
  const simpleRules = initialCondition ? condToRules(initialCondition) : null;
  // Condição complexa demais pro construtor básico (not/aninhada): abre direto
  // no modo JSON avançado, em vez de mostrar uma cláusula em branco enganosa.
  const [combine, setCombine] = useState<'and' | 'or'>(initialCondition ? combinatorOf(initialCondition) : 'and');
  const [rules, setRules] = useState<SimpleRule[]>(simpleRules ?? [{ column: columns[0] || '', op: 'eq' as any, value: '' }]);
  const [advanced, setAdvanced] = useState(!!initialCondition && !simpleRules);
  const [advancedJson, setAdvancedJson] = useState(() => JSON.stringify(initialCondition ?? { column: columns[0] || '', op: 'eq', value: '' }, null, 2));

  const addRule = () => setRules([...rules, { column: columns[0] || '', op: 'eq' as any, value: '' }]);
  const removeRule = (i: number) => setRules(rules.filter((_, idx) => idx !== i));
  const updateRule = (i: number, patch: Partial<SimpleRule>) => setRules(rules.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));

  const handleApply = () => {
    if (advanced) {
      try {
        onApply(JSON.parse(advancedJson));
      } catch {
        // mantém o modal aberto em caso de JSON inválido
      }
      return;
    }
    const conds = rules.map(ruleToCond);
    const condition: Cond = conds.length === 1 ? conds[0] : combine === 'and' ? { and: conds } : { or: conds };
    onApply(condition);
  };

  return (
    <DialogShell title="Filtrar Linhas" onClose={onClose} onConfirm={handleApply} confirmLabel="Aplicar">
      <div className="flex items-center justify-between">
        <span className="font-semibold text-slate-600 dark:text-slate-300">Modo</span>
        <button
          className="text-[11px] underline text-amber-600"
          onClick={() => setAdvanced(a => !a)}
        >
          {advanced ? 'Usar construtor básico' : 'Usar JSON avançado'}
        </button>
      </div>

      {advanced ? (
        <textarea
          data-testid="filter-advanced-json"
          value={advancedJson}
          onChange={e => setAdvancedJson(e.target.value)}
          className={`${inputCls} font-mono h-40`}
        />
      ) : (
        <div className="space-y-2">
          {rules.map((rule, i) => {
            const opDef = OPS.find(o => o.value === rule.op);
            return (
              <div key={i} className="flex items-center gap-2" data-testid={`filter-rule-${i}`}>
                {i > 0 && (
                  <select
                    value={combine}
                    onChange={e => setCombine(e.target.value as 'and' | 'or')}
                    className={`${selectCls} w-20`}
                  >
                    <option value="and">E</option>
                    <option value="or">Ou</option>
                  </select>
                )}
                <select value={rule.column} onChange={e => updateRule(i, { column: e.target.value })} className={`${selectCls} flex-1`}>
                  {columns.map(c => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
                <select value={rule.op} onChange={e => updateRule(i, { op: e.target.value as any })} className={`${selectCls} flex-1`}>
                  {OPS.map(o => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
                {opDef?.needsValue && (
                  <input
                    value={rule.value}
                    onChange={e => updateRule(i, { value: e.target.value })}
                    className={`${inputCls} flex-1`}
                    placeholder="Valor"
                  />
                )}
                <button onClick={() => removeRule(i)} className="text-slate-400 hover:text-red-500 px-1">
                  ✕
                </button>
              </div>
            );
          })}
          <button onClick={addRule} className="text-[11px] font-semibold text-amber-600 hover:underline">
            + Adicionar cláusula
          </button>
        </div>
      )}
    </DialogShell>
  );
};
