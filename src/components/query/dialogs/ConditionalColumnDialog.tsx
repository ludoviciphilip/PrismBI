import React, { useState } from 'react';
import { Plus, Trash2, ChevronUp, ChevronDown } from 'lucide-react';
import { DialogShell, FieldLabel, inputCls, selectCls } from './DialogShell';
import { Cond, EtlType, Scalar } from '../../../types/etl';

// inputCls/selectCls já trazem `w-full`. Tailwind resolve classes de largura
// conflitantes pela ORDEM EM QUE FORAM GERADAS no stylesheet, não pela ordem
// no atributo className — então `${inputCls} w-20` pode continuar com w-full
// vencendo e colapsar o campo (foi exatamente o bug relatado). Para qualquer
// campo que precise de uma largura própria (fixa ou flex), parte-se de uma
// base SEM `w-full` em vez de tentar sobrepor.
const fieldBase = inputCls.replace(/\bw-full\b\s*/, '');
const selectBase = selectCls.replace(/\bw-full\b\s*/, '');

export interface ConditionalColumnDialogColumn {
  name: string;
  type?: string;
}

type ValueOrColumn = { kind: 'value'; value: string } | { kind: 'column'; column: string };

interface Clause {
  column: string;
  op: string;
  value: ValueOrColumn;
  then: ValueOrColumn;
}

// Operadores filtrados pelo tipo da coluna escolhida na cláusula, como no Power BI.
const TEXT_OPS = [
  ['eq', 'é igual a'],
  ['neq', 'é diferente de'],
  ['contains', 'contém'],
  ['notContains', 'não contém'],
  ['startsWith', 'começa com'],
  ['endsWith', 'termina com'],
  ['isBlank', 'está em branco'],
  ['isNotBlank', 'não está em branco'],
] as const;

const NUMBER_OPS = [
  ['eq', 'é igual a'],
  ['neq', 'é diferente de'],
  ['gt', 'é maior que'],
  ['gte', 'é maior ou igual a'],
  ['lt', 'é menor que'],
  ['lte', 'é menor ou igual a'],
  ['isNull', 'é nulo'],
  ['isNotNull', 'não é nulo'],
] as const;

const GENERIC_OPS = [
  ['eq', 'é igual a'],
  ['neq', 'é diferente de'],
  ['isNull', 'é nulo'],
  ['isNotNull', 'não é nulo'],
] as const;

const NUMERIC_TYPES = new Set(['integer', 'decimal', 'currency']);
const TEXT_TYPES = new Set(['text']);
const NO_VALUE_OPS = new Set(['isNull', 'isNotNull', 'isBlank', 'isNotBlank']);

function opsForType(type: string | undefined): readonly (readonly [string, string])[] {
  if (type && NUMERIC_TYPES.has(type)) return NUMBER_OPS;
  if (type && TEXT_TYPES.has(type)) return TEXT_OPS;
  if (!type) return TEXT_OPS;
  return GENERIC_OPS;
}

function typeOf(columns: ConditionalColumnDialogColumn[], name: string): string | undefined {
  return columns.find(c => c.name === name)?.type;
}

function scalarFrom(raw: string, type: string | undefined): Scalar {
  if (type && NUMERIC_TYPES.has(type) && raw !== '' && !Number.isNaN(Number(raw))) return Number(raw);
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  return raw;
}

function vocToOutput(v: ValueOrColumn, columnType?: string): Scalar | { column: string } {
  if (v.kind === 'column') return { column: v.column };
  return scalarFrom(v.value, columnType);
}

/** Inverso de `vocToOutput`: valor/coluna salvo na etapa -> estado editável do campo. */
function valueOrColumnFrom(v: Scalar | { column: string }): ValueOrColumn {
  if (v !== null && typeof v === 'object' && 'column' in v) return { kind: 'column', column: v.column };
  return { kind: 'value', value: v === null ? '' : String(v) };
}

/** Inverso do que `handleApply` monta: um `Cond` + saída salvos -> uma cláusula editável. */
function condToClause(when: Cond, then: Scalar | { column: string }, fallbackColumn: string): Clause {
  if ('column' in when) {
    return {
      column: when.column,
      op: when.op,
      value: 'value' in when ? valueOrColumnFrom(when.value) : { kind: 'value', value: '' },
      then: valueOrColumnFrom(then),
    };
  }
  // `and`/`or`/`not` não vêm desta UI (ela só produz cláusulas simples) — melhor
  // esforço: mostra uma cláusula em branco em vez de quebrar o diálogo.
  return { column: fallbackColumn, op: 'eq', value: { kind: 'value', value: '' }, then: valueOrColumnFrom(then) };
}

/** Campo reutilizável: alterna entre digitar um valor literal ou escolher uma coluna. */
const ValueOrColumnField: React.FC<{
  value: ValueOrColumn;
  onChange: (v: ValueOrColumn) => void;
  columns: ConditionalColumnDialogColumn[];
  testId: string;
  placeholder?: string;
}> = ({ value, onChange, columns, testId, placeholder }) => (
  <div className="flex items-stretch gap-1" data-testid={testId}>
    <select
      aria-label={`${testId}-kind`}
      value={value.kind}
      onChange={e => {
        const kind = e.target.value as 'value' | 'column';
        onChange(kind === 'value' ? { kind: 'value', value: '' } : { kind: 'column', column: columns[0]?.name ?? '' });
      }}
      className={`${selectBase} w-20 shrink-0 px-1`}
    >
      <option value="value">Valor</option>
      <option value="column">Coluna</option>
    </select>
    {value.kind === 'value' ? (
      <input
        aria-label={`${testId}-value`}
        value={value.value}
        onChange={e => onChange({ kind: 'value', value: e.target.value })}
        className={`${fieldBase} min-w-0 flex-1`}
        placeholder={placeholder}
      />
    ) : (
      <select
        aria-label={`${testId}-column`}
        value={value.column}
        onChange={e => onChange({ kind: 'column', column: e.target.value })}
        className={`${selectBase} min-w-0 flex-1`}
      >
        {columns.map(c => (
          <option key={c.name} value={c.name}>
            {c.name}
          </option>
        ))}
      </select>
    )}
  </div>
);

export const ConditionalColumnDialog: React.FC<{
  columns: (string | ConditionalColumnDialogColumn)[];
  initial?: {
    newColumn: string;
    rules: { when: Cond; then: Scalar | { column: string } }[];
    otherwise: Scalar | { column: string };
    type?: EtlType;
  };
  onClose: () => void;
  onApply: (args: {
    newColumn: string;
    rules: { when: Cond; then: Scalar | { column: string } }[];
    otherwise: Scalar | { column: string };
    type?: EtlType;
  }) => void;
}> = ({ columns: rawColumns, initial, onClose, onApply }) => {
  const columns: ConditionalColumnDialogColumn[] = rawColumns.map(c => (typeof c === 'string' ? { name: c } : c));
  const firstCol = columns[0]?.name ?? '';
  const [newColumn, setNewColumn] = useState(initial?.newColumn ?? 'Coluna_Condicional');
  const [outputType, setOutputType] = useState<EtlType | ''>(initial?.type ?? '');
  const [clauses, setClauses] = useState<Clause[]>(
    initial ? initial.rules.map(r => condToClause(r.when, r.then, firstCol)) : [{ column: firstCol, op: 'eq', value: { kind: 'value', value: '' }, then: { kind: 'value', value: '' } }],
  );
  const [otherwise, setOtherwise] = useState<ValueOrColumn>(initial ? valueOrColumnFrom(initial.otherwise) : { kind: 'value', value: '' });

  const addClause = () =>
    setClauses([...clauses, { column: firstCol, op: 'eq', value: { kind: 'value', value: '' }, then: { kind: 'value', value: '' } }]);
  const removeClause = (i: number) => setClauses(clauses.filter((_, idx) => idx !== i));
  const update = (i: number, patch: Partial<Clause>) => setClauses(clauses.map((c, idx) => (idx === i ? { ...c, ...patch } : c)));
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= clauses.length) return;
    const next = [...clauses];
    [next[i], next[j]] = [next[j], next[i]];
    setClauses(next);
  };

  const handleApply = () => {
    const rules = clauses.map(c => {
      const colType = typeOf(columns, c.column);
      const when: Cond = NO_VALUE_OPS.has(c.op)
        ? ({ column: c.column, op: c.op as any })
        : ({ column: c.column, op: c.op as any, value: vocToOutput(c.value, colType) as Scalar });
      return { when, then: vocToOutput(c.then, outputType || undefined) };
    });
    onApply({
      newColumn,
      rules,
      otherwise: vocToOutput(otherwise, outputType || undefined),
      ...(outputType ? { type: outputType } : {}),
    });
  };

  return (
    <DialogShell title="Coluna Condicional" onClose={onClose} onConfirm={handleApply} confirmLabel="OK" widthClass="max-w-4xl">
      <div className="grid grid-cols-2 gap-3">
        <div>
          <FieldLabel>Nome da nova coluna</FieldLabel>
          <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
        </div>
        <div>
          <FieldLabel>Tipo de dados de saída</FieldLabel>
          <select value={outputType} onChange={e => setOutputType(e.target.value as EtlType | '')} className={`${selectCls} w-full`}>
            <option value="">Automático</option>
            <option value="text">Texto</option>
            <option value="integer">Número Inteiro</option>
            <option value="decimal">Decimal</option>
            <option value="currency">Moeda</option>
            <option value="date">Data</option>
            <option value="datetime">Data/Hora</option>
            <option value="boolean">Verdadeiro/Falso</option>
          </select>
        </div>
      </div>

      <div className="grid grid-cols-[28px_minmax(110px,1fr)_minmax(140px,1fr)_minmax(170px,1.4fr)_28px_minmax(170px,1.4fr)_52px] gap-x-2 gap-y-1 items-center text-[11px]">
        <div />
        <div className="font-semibold text-slate-500">Nome da coluna</div>
        <div className="font-semibold text-slate-500">Operador</div>
        <div className="font-semibold text-slate-500">Valor</div>
        <div />
        <div className="font-semibold text-slate-500">Saída</div>
        <div />

        {clauses.map((c, i) => {
          const ops = opsForType(typeOf(columns, c.column));
          return (
            <React.Fragment key={i}>
              <span className="text-slate-400 text-center">{i === 0 ? 'Se' : 'Senão se'}</span>
              <select
                data-testid={`clause-${i}-column`}
                value={c.column}
                onChange={e => update(i, { column: e.target.value })}
                className={`${selectCls} min-w-0`}
              >
                {columns.map(col => (
                  <option key={col.name} value={col.name}>
                    {col.name}
                  </option>
                ))}
              </select>
              <select
                data-testid={`clause-${i}-op`}
                value={c.op}
                onChange={e => update(i, { op: e.target.value })}
                className={`${selectCls} min-w-0`}
              >
                {ops.map(([val, label]) => (
                  <option key={val} value={val}>
                    {label}
                  </option>
                ))}
              </select>
              {NO_VALUE_OPS.has(c.op) ? (
                <div />
              ) : (
                <ValueOrColumnField
                  testId={`clause-${i}-value`}
                  value={c.value}
                  onChange={v => update(i, { value: v })}
                  columns={columns}
                  placeholder="valor"
                />
              )}
              <span className="text-slate-400 text-center">então</span>
              <ValueOrColumnField testId={`clause-${i}-then`} value={c.then} onChange={v => update(i, { then: v })} columns={columns} placeholder="saída" />
              <div className="flex items-center gap-0.5 justify-end">
                <button onClick={() => move(i, -1)} disabled={i === 0} className="text-slate-400 hover:text-slate-700 disabled:opacity-30" aria-label="Mover para cima">
                  <ChevronUp className="w-3.5 h-3.5" />
                </button>
                <button onClick={() => move(i, 1)} disabled={i === clauses.length - 1} className="text-slate-400 hover:text-slate-700 disabled:opacity-30" aria-label="Mover para baixo">
                  <ChevronDown className="w-3.5 h-3.5" />
                </button>
                <button onClick={() => removeClause(i)} className="text-slate-400 hover:text-red-500" aria-label="Remover cláusula">
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </React.Fragment>
          );
        })}
      </div>

      <button onClick={addClause} className="text-[11px] font-semibold text-amber-600 flex items-center gap-1">
        <Plus className="w-3 h-3" /> Adicionar cláusula
      </button>

      <div className="flex items-center gap-2 pt-1 border-t border-slate-200 dark:border-slate-800">
        <span className="text-slate-400 text-[11px] w-16 shrink-0">Caso contrário</span>
        <ValueOrColumnField testId="otherwise" value={otherwise} onChange={setOtherwise} columns={columns} placeholder="saída" />
      </div>
    </DialogShell>
  );
};
