/**
 * Diálogo "Formatação condicional" (Power BI-like), v0.10 Frente B.
 * Cria/edita um único `ConditionalFormat` do visual selecionado.
 * Precisa caber em 1366×768 (ver skill tailwind-width-class-override).
 */
import React, { useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { X, Plus, Trash2 } from 'lucide-react';
import type {
  CondFormatStop,
  CondFormatTarget,
  ConditionalFormat,
  ConditionalFormatRule,
  Visual,
} from '../../../types/bi';
import { interpolateColor } from '../../../engine/conditionalFormat';
import { ICON_SET_LABELS, ICON_SET_PREVIEW, ConditionalIcon } from './icons';

type RuleWithKind = ConditionalFormatRule & { valueKind?: 'number' | 'percent' };

const TARGET_OPTIONS: { value: CondFormatTarget; label: string }[] = [
  { value: 'background', label: 'Cor de fundo' },
  { value: 'fontColor', label: 'Cor da fonte' },
  { value: 'dataBar', label: 'Barras de dados' },
  { value: 'icon', label: 'Ícones' },
];

const OPERATOR_OPTIONS: { value: ConditionalFormatRule['operator']; label: string }[] = [
  { value: '>', label: 'maior que' },
  { value: '>=', label: 'maior ou igual a' },
  { value: '<', label: 'menor que' },
  { value: '<=', label: 'menor ou igual a' },
  { value: '==', label: 'igual a' },
  { value: 'between', label: 'entre' },
];

function makeId(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

function defaultRule(): RuleWithKind {
  return { operator: '>', value: 0, color: '#3b82f6', valueKind: 'number' };
}

function defaultStop(kind: CondFormatStop['kind'], color: string): CondFormatStop {
  return { kind, color, value: kind === 'number' || kind === 'percent' ? 0 : undefined };
}

export interface ConditionalFormatDialogProps {
  visual: Visual;
  availableFields: string[];
  measureFields: string[];
  initial: ConditionalFormat | null;
  onSave: (format: ConditionalFormat) => void;
  onClose: () => void;
}

export const ConditionalFormatDialog: React.FC<ConditionalFormatDialogProps> = ({
  visual,
  availableFields,
  measureFields,
  initial,
  onSave,
  onClose,
}) => {
  const [applyTo, setApplyTo] = useState(initial?.applyTo || availableFields[0] || '');
  const [target, setTarget] = useState<CondFormatTarget>(initial?.target || 'background');
  const [mode, setMode] = useState<ConditionalFormat['mode']>(initial?.mode || 'rules');
  const [basedOnField, setBasedOnField] = useState<string>(initial?.basedOn?.field || '');
  const [basedOnKind, setBasedOnKind] = useState<'measure' | 'column'>(initial?.basedOn?.kind || 'measure');

  const [rules, setRules] = useState<RuleWithKind[]>(initial?.rules && initial.rules.length ? initial.rules : [defaultRule()]);
  const [gradMin, setGradMin] = useState<CondFormatStop>(initial?.gradient?.min || defaultStop('lowest', '#dc2626'));
  const [gradMid, setGradMid] = useState<CondFormatStop | null>(initial?.gradient?.mid || null);
  const [gradMax, setGradMax] = useState<CondFormatStop>(initial?.gradient?.max || defaultStop('highest', '#16a34a'));
  const [dataBarPos, setDataBarPos] = useState(initial?.dataBar?.positiveColor || '#3b82f6');
  const [dataBarNeg, setDataBarNeg] = useState(initial?.dataBar?.negativeColor || '#ef4444');
  const [dataBarShowBarOnly, setDataBarShowBarOnly] = useState(Boolean(initial?.dataBar?.showBarOnly));
  const [iconSet, setIconSet] = useState<'arrows' | 'traffic' | 'flags'>(initial?.icons?.set || 'traffic');
  const [iconRules, setIconRules] = useState<ConditionalFormatRule[]>(
    initial?.icons?.rules && initial.icons.rules.length
      ? initial.icons.rules
      : [
          { operator: '>', value: 66, color: '' },
          { operator: '>', value: 33, color: '' },
          { operator: '<=', value: 33, color: '' },
        ]
  );

  const previewStyle = useMemo(() => {
    const stops: { pct: number; color: string }[] = [{ pct: 0, color: gradMin.color }];
    if (gradMid) stops.push({ pct: 50, color: gradMid.color });
    stops.push({ pct: 100, color: gradMax.color });
    const css = stops.map(s => `${s.color} ${s.pct}%`).join(', ');
    return { background: `linear-gradient(to right, ${css})` };
  }, [gradMin, gradMid, gradMax]);

  const updateRule = (idx: number, patch: Partial<RuleWithKind>) => {
    setRules(prev => prev.map((r, i) => (i === idx ? { ...r, ...patch } : r)));
  };

  const updateIconRule = (idx: number, patch: Partial<ConditionalFormatRule>) => {
    setIconRules(prev => prev.map((r, i) => (i === idx ? { ...r, ...patch } : r)));
  };

  const handleSave = () => {
    if (!applyTo) return;
    const format: ConditionalFormat = {
      id: initial?.id || makeId('cf'),
      target,
      applyTo,
      mode: target === 'dataBar' || target === 'icon' ? mode : mode,
      ...(basedOnField ? { basedOn: { field: basedOnField, kind: basedOnKind } } : {}),
    };
    if (target === 'dataBar') {
      format.dataBar = { positiveColor: dataBarPos, negativeColor: dataBarNeg, showBarOnly: dataBarShowBarOnly };
    } else if (target === 'icon') {
      format.icons = { set: iconSet, rules: iconRules };
    } else if (mode === 'gradient') {
      format.gradient = { min: gradMin, mid: gradMid || undefined, max: gradMax };
    } else if (mode === 'rules') {
      format.rules = rules;
    }
    // mode === 'fieldValue': nenhuma config adicional (a cor vem do próprio valor)
    onSave(format);
  };

  const allFields = [...new Set([...availableFields, ...measureFields])];

  return createPortal(
    <div className="fixed inset-0 bg-black/60 z-[10001] flex items-center justify-center p-4" data-testid="condfmt-dialog-overlay">
      <div
        className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl w-full max-w-xl p-5 text-xs flex flex-col max-h-[700px] overflow-hidden"
        data-testid="condfmt-dialog"
      >
        <div className="flex items-center justify-between pb-2 border-b border-slate-200 dark:border-slate-800 mb-3">
          <h2 className="font-bold text-sm text-slate-800 dark:text-slate-100">Formatação condicional</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer" title="Fechar" aria-label="Fechar">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto space-y-3 pr-1">
          {/* Campo aplicado a */}
          <div className="space-y-1">
            <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 block">Aplicado a</label>
            <select
              aria-label="Aplicado a"
              value={applyTo}
              onChange={e => setApplyTo(e.target.value)}
              className="w-full flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
            >
              {allFields.map(f => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </div>

          {/* O que formatar */}
          <div className="space-y-1">
            <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 block">O que formatar</label>
            <select
              aria-label="O que formatar"
              value={target}
              onChange={e => setTarget(e.target.value as CondFormatTarget)}
              className="w-full flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
            >
              {TARGET_OPTIONS.map(o => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>

          {/* Com base em */}
          <div className="space-y-1">
            <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 block">Com base em (opcional)</label>
            <div className="flex gap-2">
              <select
                aria-label="Com base em"
                value={basedOnField}
                onChange={e => setBasedOnField(e.target.value)}
                className="flex-1 w-20 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
              >
                <option value="">(o próprio campo)</option>
                {allFields.map(f => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
              {basedOnField && (
                <select
                  aria-label="Tipo do campo base"
                  value={basedOnKind}
                  onChange={e => setBasedOnKind(e.target.value as 'measure' | 'column')}
                  className="w-28 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
                >
                  <option value="measure">Medida</option>
                  <option value="column">Coluna</option>
                </select>
              )}
            </div>
          </div>

          {(target === 'background' || target === 'fontColor') && (
            <div className="space-y-1">
              <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 block">Estilo de formato</label>
              <select
                aria-label="Estilo de formato"
                value={mode}
                onChange={e => setMode(e.target.value as ConditionalFormat['mode'])}
                className="w-full flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
              >
                <option value="gradient">Gradiente de cores</option>
                <option value="rules">Regras</option>
                <option value="fieldValue">Valor do campo</option>
              </select>
            </div>
          )}

          {(target === 'background' || target === 'fontColor') && mode === 'fieldValue' && (
            <p className="text-[11px] text-slate-500 dark:text-slate-400 italic">
              A cor vem do próprio valor do campo (ex.: <code>#ff0000</code> ou <code>red</code>).
            </p>
          )}

          {(target === 'background' || target === 'fontColor') && mode === 'gradient' && (
            <div className="space-y-2 border border-slate-200 dark:border-slate-800 rounded-lg p-2.5">
              <div className="h-4 w-full rounded" style={previewStyle} data-testid="gradient-preview" />
              <GradientStopEditor label="Mínimo" stop={gradMin} onChange={setGradMin} kinds={['lowest', 'number', 'percent']} />
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-slate-600 dark:text-slate-400">Ponto médio (3 cores)</span>
                <input
                  type="checkbox"
                  aria-label="Ativar ponto médio"
                  checked={!!gradMid}
                  onChange={e => setGradMid(e.target.checked ? defaultStop('percent', '#f59e0b') : null)}
                  className="rounded border-slate-300"
                />
              </div>
              {gradMid && <GradientStopEditor label="Médio" stop={gradMid} onChange={setGradMid} kinds={['number', 'percent']} />}
              <GradientStopEditor label="Máximo" stop={gradMax} onChange={setGradMax} kinds={['highest', 'number', 'percent']} />
            </div>
          )}

          {(target === 'background' || target === 'fontColor') && mode === 'rules' && (
            <RulesEditor rules={rules} onUpdate={updateRule} onAdd={() => setRules(p => [...p, defaultRule()])} onRemove={idx => setRules(p => p.filter((_, i) => i !== idx))} />
          )}

          {target === 'dataBar' && (
            <div className="space-y-2 border border-slate-200 dark:border-slate-800 rounded-lg p-2.5">
              <div className="flex items-center gap-2">
                <label className="text-[11px] text-slate-600 dark:text-slate-400 w-24">Cor positiva</label>
                <input
                  type="color"
                  aria-label="Cor positiva da barra"
                  value={dataBarPos}
                  onChange={e => setDataBarPos(e.target.value)}
                  className="w-16 h-7 flex-shrink-0"
                />
              </div>
              <div className="flex items-center gap-2">
                <label className="text-[11px] text-slate-600 dark:text-slate-400 w-24">Cor negativa</label>
                <input
                  type="color"
                  aria-label="Cor negativa da barra"
                  value={dataBarNeg}
                  onChange={e => setDataBarNeg(e.target.value)}
                  className="w-16 h-7 flex-shrink-0"
                />
              </div>
              <label className="flex items-center gap-2 text-[11px] text-slate-600 dark:text-slate-400">
                <input
                  type="checkbox"
                  aria-label="Mostrar somente a barra"
                  checked={dataBarShowBarOnly}
                  onChange={e => setDataBarShowBarOnly(e.target.checked)}
                  className="rounded border-slate-300"
                />
                Mostrar somente a barra (ocultar o número)
              </label>
            </div>
          )}

          {target === 'icon' && (
            <div className="space-y-2 border border-slate-200 dark:border-slate-800 rounded-lg p-2.5">
              <div className="space-y-1">
                <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 block">Conjunto de ícones</label>
                <select
                  aria-label="Conjunto de ícones"
                  value={iconSet}
                  onChange={e => setIconSet(e.target.value as 'arrows' | 'traffic' | 'flags')}
                  className="w-full flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100"
                >
                  {(['arrows', 'traffic', 'flags'] as const).map(s => (
                    <option key={s} value={s}>
                      {ICON_SET_LABELS[s]}
                    </option>
                  ))}
                </select>
                <div className="flex gap-2 pt-1">
                  {ICON_SET_PREVIEW[iconSet].map(k => (
                    <ConditionalIcon key={k} icon={k} size={16} />
                  ))}
                </div>
              </div>
              {iconRules.map((r, idx) => (
                <div key={idx} className="flex items-center gap-1.5">
                  <ConditionalIcon icon={ICON_SET_PREVIEW[iconSet][Math.min(idx, 2)]} size={14} />
                  <select
                    aria-label={`Operador do ícone ${idx + 1}`}
                    value={r.operator}
                    onChange={e => updateIconRule(idx, { operator: e.target.value as ConditionalFormatRule['operator'] })}
                    className="flex-1 w-20 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
                  >
                    {OPERATOR_OPTIONS.filter(o => o.value !== 'between').map(o => (
                      <option key={o.value} value={o.value}>
                        {o.label}
                      </option>
                    ))}
                  </select>
                  <input
                    aria-label={`Valor do ícone ${idx + 1}`}
                    type="number"
                    value={r.value}
                    onChange={e => updateIconRule(idx, { value: Number(e.target.value) })}
                    className="w-20 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
                  />
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-200 dark:border-slate-800 mt-3">
          <button onClick={onClose} className="px-3 py-1.5 rounded border border-slate-300 dark:border-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 cursor-pointer">
            Cancelar
          </button>
          <button onClick={handleSave} className="px-3 py-1.5 rounded bg-amber-500 text-slate-950 font-semibold hover:bg-amber-400 cursor-pointer">
            OK
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
};

function GradientStopEditor({
  label,
  stop,
  onChange,
  kinds,
}: {
  label: string;
  stop: CondFormatStop;
  onChange: (s: CondFormatStop) => void;
  kinds: CondFormatStop['kind'][];
}) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="text-[11px] text-slate-600 dark:text-slate-400 w-14 flex-shrink-0">{label}</span>
      <select
        aria-label={`${label} - tipo`}
        value={stop.kind}
        onChange={e => onChange({ ...stop, kind: e.target.value as CondFormatStop['kind'] })}
        className="flex-1 w-20 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
      >
        {kinds.map(k => (
          <option key={k} value={k}>
            {k === 'lowest' ? 'Menor valor' : k === 'highest' ? 'Maior valor' : k === 'number' ? 'Número' : 'Percentual'}
          </option>
        ))}
      </select>
      {(stop.kind === 'number' || stop.kind === 'percent') && (
        <input
          aria-label={`${label} - valor`}
          type="number"
          value={stop.value ?? 0}
          onChange={e => onChange({ ...stop, value: Number(e.target.value) })}
          className="w-20 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
        />
      )}
      <input
        aria-label={`${label} - cor`}
        type="color"
        value={stop.color}
        onChange={e => onChange({ ...stop, color: e.target.value })}
        className="w-16 h-7 flex-shrink-0"
      />
    </div>
  );
}

function RulesEditor({
  rules,
  onUpdate,
  onAdd,
  onRemove,
}: {
  rules: RuleWithKind[];
  onUpdate: (idx: number, patch: Partial<RuleWithKind>) => void;
  onAdd: () => void;
  onRemove: (idx: number) => void;
}) {
  return (
    <div className="space-y-2 border border-slate-200 dark:border-slate-800 rounded-lg p-2.5">
      {rules.map((r, idx) => (
        <div key={idx} className="flex items-center gap-1.5">
          <select
            aria-label={`Operador da regra ${idx + 1}`}
            value={r.operator}
            onChange={e => onUpdate(idx, { operator: e.target.value as ConditionalFormatRule['operator'] })}
            className="flex-1 w-20 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
          >
            {OPERATOR_OPTIONS.map(o => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <input
            aria-label={`Valor da regra ${idx + 1}`}
            type="number"
            value={r.value}
            onChange={e => onUpdate(idx, { value: Number(e.target.value) })}
            className="w-20 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
          />
          {r.operator === 'between' && (
            <input
              aria-label={`Valor 2 da regra ${idx + 1}`}
              type="number"
              value={r.value2 ?? 0}
              onChange={e => onUpdate(idx, { value2: Number(e.target.value) })}
              className="w-20 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
            />
          )}
          <select
            aria-label={`Unidade da regra ${idx + 1}`}
            value={r.valueKind || 'number'}
            onChange={e => onUpdate(idx, { valueKind: e.target.value as 'number' | 'percent' })}
            className="w-24 flex-shrink-0 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-slate-800 dark:text-slate-100"
          >
            <option value="number">Número</option>
            <option value="percent">Percentual</option>
          </select>
          <input
            aria-label={`Cor da regra ${idx + 1}`}
            type="color"
            value={r.color}
            onChange={e => onUpdate(idx, { color: e.target.value })}
            className="w-16 h-7 flex-shrink-0"
          />
          <button
            type="button"
            aria-label={`Remover regra ${idx + 1}`}
            onClick={() => onRemove(idx)}
            className="p-1 text-slate-400 hover:text-rose-600 cursor-pointer flex-shrink-0"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
      <button
        type="button"
        onClick={onAdd}
        className="flex items-center gap-1 text-[11px] text-amber-600 hover:text-amber-700 font-semibold cursor-pointer"
      >
        <Plus className="w-3.5 h-3.5" /> Adicionar regra
      </button>
    </div>
  );
}

// Reexporta para uso em testes de interpolação na prévia, se necessário.
export { interpolateColor };
