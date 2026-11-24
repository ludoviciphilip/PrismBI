import React, { useState } from 'react';
import {
  LayoutDashboard,
  Target,
  Bell,
  Plus,
  Trash2,
  ExternalLink,
  CheckCircle2,
  AlertTriangle,
  Clock,
  TrendingUp,
} from 'lucide-react';
import { useBiStore } from '../../store/useBiStore';
import { Goal } from '../../types/bi';
import { formatCurrency, formatPercent } from '../../engine/dataEngine';

export const DashboardsView: React.FC = () => {
  const {
    dashboardTiles,
    removeDashboardTile,
    goals,
    addGoal,
    updateGoal,
    pages,
    setActivePageId,
    setActiveView,
  } = useBiStore();

  const [activeTab, setActiveTab] = useState<'tiles' | 'goals'>('tiles');
  const [isGoalModalOpen, setIsGoalModalOpen] = useState(false);

  // New Goal form
  const [newGoalName, setNewGoalName] = useState('');
  const [newGoalTarget, setNewGoalTarget] = useState(500000);
  const [newGoalCurrent, setNewGoalCurrent] = useState(420000);
  const [newGoalUnit, setNewGoalUnit] = useState<'R$' | '%' | 'unidades'>('R$');
  const [newGoalOwner, setNewGoalOwner] = useState('Diretoria');
  const [newGoalDeadline, setNewGoalDeadline] = useState('31/12/2025');

  const handleCreateGoal = () => {
    if (!newGoalName) return;
    const g: Goal = {
      id: 'goal_' + Math.random().toString(36).substr(2, 6),
      name: newGoalName,
      target: Number(newGoalTarget),
      current: Number(newGoalCurrent),
      unit: newGoalUnit,
      owner: newGoalOwner,
      deadline: newGoalDeadline,
      status: Number(newGoalCurrent) >= Number(newGoalTarget) ? 'on_track' : 'at_risk',
      history: [
        { date: 'Mês Anterior', value: Number(newGoalCurrent) * 0.9 },
        { date: 'Mês Atual', value: Number(newGoalCurrent) },
      ],
    };
    addGoal(g);
    setIsGoalModalOpen(false);
    setNewGoalName('');
  };

  const getVisualDetails = (visualId: string, pageId: string) => {
    const page = pages.find(p => p.id === pageId);
    const visual = page?.visuals.find(v => v.id === visualId);
    return { page, visual };
  };

  return (
    <div className="flex-1 flex flex-col h-full bg-slate-100 dark:bg-slate-950 overflow-hidden text-xs select-none text-slate-800 dark:text-slate-100">
      {/* Top Bar */}
      <div className="bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 p-3 shadow-xs flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="flex bg-slate-200/70 dark:bg-slate-800 p-1 rounded-lg">
            <button
              onClick={() => setActiveTab('tiles')}
              className={`flex items-center gap-1.5 px-3 py-1 rounded font-bold transition-all ${
                activeTab === 'tiles' ? 'bg-white dark:bg-slate-700 text-slate-900 dark:text-slate-100 shadow-xs' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
              }`}
            >
              <LayoutDashboard className="w-4 h-4 text-amber-500" />
              <span>Painel de Blocos (Dashboard)</span>
            </button>
            <button
              onClick={() => setActiveTab('goals')}
              className={`flex items-center gap-1.5 px-3 py-1 rounded font-bold transition-all ${
                activeTab === 'goals' ? 'bg-white dark:bg-slate-700 text-slate-900 dark:text-slate-100 shadow-xs' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
              }`}
            >
              <Target className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
              <span>Scorecards & Metas Corporativas</span>
            </button>
          </div>
        </div>

        {activeTab === 'goals' && (
          <button
            onClick={() => setIsGoalModalOpen(true)}
            className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white font-bold"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Nova Meta</span>
          </button>
        )}
      </div>

      {/* CONTENT: TILES */}
      {activeTab === 'tiles' && (
        <div className="flex-1 overflow-auto p-6">
          <div className="max-w-6xl mx-auto space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-base font-bold text-slate-800 dark:text-slate-100">Blocos Fixados do Relatório</h2>
                <p className="text-slate-500 dark:text-slate-400 text-[11px]">
                  Clique em qualquer bloco para navegar diretamente ao relatório analítico de origem.
                </p>
              </div>
            </div>

            {dashboardTiles.length === 0 ? (
              <div className="p-12 text-center bg-white dark:bg-slate-900 rounded-xl border border-slate-200 dark:border-slate-800 text-slate-400 dark:text-slate-500">
                <LayoutDashboard className="w-10 h-10 mx-auto mb-2 text-slate-300 dark:text-slate-600" />
                <p className="font-semibold text-slate-600 dark:text-slate-300">Nenhum bloco fixado</p>
                <p className="text-[11px] mt-1">No canvas de relatório, clique no ícone de alfinete (📌) em qualquer visual.</p>
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {dashboardTiles.map(tile => {
                  const { page, visual } = getVisualDetails(tile.visualId, tile.pageId);

                  return (
                    <div
                      key={tile.id}
                      className="bg-white dark:bg-slate-900 rounded-xl shadow-xs border border-slate-200 dark:border-slate-800 p-4 flex flex-col justify-between hover:shadow-md transition-shadow group relative"
                    >
                      <div className="flex items-start justify-between">
                        <div>
                          <span className="text-[10px] text-amber-600 dark:text-amber-400 font-bold uppercase tracking-wider">
                            {page?.name || 'Relatório'}
                          </span>
                          <h3 className="font-bold text-slate-800 dark:text-slate-100 text-sm mt-0.5">{tile.title}</h3>
                        </div>
                        <div className="flex items-center gap-1">
                          <button
                            onClick={() => {
                              setActivePageId(tile.pageId);
                              setActiveView('report');
                            }}
                            className="p-1 text-slate-400 hover:text-amber-600 rounded"
                            title="Ir para relatório de origem"
                          >
                            <ExternalLink className="w-4 h-4" />
                          </button>
                          <button
                            onClick={() => removeDashboardTile(tile.id)}
                            className="p-1 text-slate-400 hover:text-red-600 rounded"
                            title="Desafixar bloco"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </div>

                      {/* Tile Value */}
                      <div className="my-4">
                        <div className="text-3xl font-extrabold text-slate-900 dark:text-slate-100">
                          {tile.title.includes('Faturamento')
                            ? 'R$ 1.280.000,00'
                            : tile.title.includes('Margem')
                            ? 'R$ 512.000,00'
                            : '84% Meta'}
                        </div>
                        <div className="flex items-center gap-1 text-[11px] text-emerald-600 dark:text-emerald-400 font-semibold mt-1">
                          <TrendingUp className="w-3.5 h-3.5" />
                          <span>+14,2% vs trimestre anterior</span>
                        </div>
                      </div>

                      {/* Alert notice if threshold defined */}
                      {tile.alertThreshold && (
                        <div className="p-2 rounded bg-amber-50 dark:bg-amber-950/40 border border-amber-200 dark:border-amber-800 text-[10px] text-amber-800 dark:text-amber-400 flex items-center gap-1.5">
                          <Bell className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
                          <span>Alerta ativo: Acima de {formatCurrency(tile.alertThreshold)}</span>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}

      {/* CONTENT: SCORECARDS & GOALS */}
      {activeTab === 'goals' && (
        <div className="flex-1 overflow-auto p-6">
          <div className="max-w-5xl mx-auto space-y-4">
            <div>
              <h2 className="text-base font-bold text-slate-800 dark:text-slate-100">Scorecard Estratégico de Metas</h2>
              <p className="text-slate-500 dark:text-slate-400 text-[11px]">
                Acompanhamento contínuo dos objetivos corporativos (OKRs/KPIs) com status e responsáveis.
              </p>
            </div>

            <div className="space-y-3">
              {goals.map(goal => {
                const progressPct = Math.min(100, Math.round((goal.current / goal.target) * 100));

                return (
                  <div
                    key={goal.id}
                    className="bg-white dark:bg-slate-900 rounded-xl shadow-xs border border-slate-200 dark:border-slate-800 p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 hover:shadow-md transition-shadow"
                  >
                    <div className="flex-1 space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="font-bold text-slate-900 dark:text-slate-100 text-sm">{goal.name}</span>
                        <span
                          className={`text-[10px] font-bold px-2 py-0.5 rounded-full flex items-center gap-1 ${
                            goal.status === 'on_track'
                              ? 'bg-emerald-100 dark:bg-emerald-950/60 text-emerald-800 dark:text-emerald-300'
                              : goal.status === 'at_risk'
                              ? 'bg-amber-100 dark:bg-amber-950/60 text-amber-800 dark:text-amber-300'
                              : 'bg-red-100 dark:bg-red-950/60 text-red-800 dark:text-red-300'
                          }`}
                        >
                          {goal.status === 'on_track' ? (
                            <CheckCircle2 className="w-3 h-3" />
                          ) : (
                            <AlertTriangle className="w-3 h-3" />
                          )}
                          {goal.status === 'on_track' ? 'No Prazo' : 'Em Risco'}
                        </span>
                      </div>

                      <div className="text-[11px] text-slate-500 dark:text-slate-400 flex items-center gap-4">
                        <span>Responsável: <strong className="text-slate-700 dark:text-slate-200">{goal.owner}</strong></span>
                        <span>Prazo: <strong className="text-slate-700 dark:text-slate-200">{goal.deadline}</strong></span>
                      </div>

                      {/* Progress Bar */}
                      <div className="w-full max-w-md pt-1.5 space-y-1">
                        <div className="flex justify-between text-[11px] font-semibold">
                          <span className="text-slate-700 dark:text-slate-200">
                            Atual: {goal.unit === 'R$' ? formatCurrency(goal.current) : `${goal.current} ${goal.unit}`}
                          </span>
                          <span className="text-slate-500 dark:text-slate-400">
                            Meta: {goal.unit === 'R$' ? formatCurrency(goal.target) : `${goal.target} ${goal.unit}`} ({progressPct}%)
                          </span>
                        </div>
                        <div className="w-full h-2 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
                          <div
                            style={{ width: `${progressPct}%` }}
                            className={`h-full rounded-full transition-all ${
                              goal.status === 'on_track' ? 'bg-emerald-500' : 'bg-amber-500'
                            }`}
                          ></div>
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => {
                          const nextVal = prompt('Atualizar valor corrente da meta:', String(goal.current));
                          if (nextVal !== null) {
                            updateGoal(goal.id, { current: Number(nextVal) });
                          }
                        }}
                        className="px-3 py-1.5 rounded border border-slate-300 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-200 font-semibold text-xs transition-colors"
                      >
                        Atualizar Valor
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {/* Modal: New Goal */}
      {isGoalModalOpen && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-xs z-50 flex items-center justify-center p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl max-w-md w-full p-5 text-xs space-y-3">
            <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-2">
              <span className="font-bold text-slate-900 dark:text-slate-100 text-sm">Cadastrar Nova Meta Estratégica</span>
              <button onClick={() => setIsGoalModalOpen(false)} className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200">✕</button>
            </div>

            <div>
              <label className="font-semibold text-slate-700 dark:text-slate-300 block mb-1">Nome do Indicador / Meta</label>
              <input
                type="text"
                placeholder="Ex.: Margem EBITDA Semestral"
                value={newGoalName}
                onChange={(e) => setNewGoalName(e.target.value)}
                className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
              />
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="font-semibold text-slate-700 dark:text-slate-300 block mb-1">Valor Alvo (Meta)</label>
                <input
                  type="number"
                  value={newGoalTarget}
                  onChange={(e) => setNewGoalTarget(Number(e.target.value))}
                  className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                />
              </div>
              <div>
                <label className="font-semibold text-slate-700 dark:text-slate-300 block mb-1">Valor Realizado Atual</label>
                <input
                  type="number"
                  value={newGoalCurrent}
                  onChange={(e) => setNewGoalCurrent(Number(e.target.value))}
                  className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="font-semibold text-slate-700 dark:text-slate-300 block mb-1">Responsável</label>
                <input
                  type="text"
                  value={newGoalOwner}
                  onChange={(e) => setNewGoalOwner(e.target.value)}
                  className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                />
              </div>
              <div>
                <label className="font-semibold text-slate-700 dark:text-slate-300 block mb-1">Prazo / Deadline</label>
                <input
                  type="text"
                  value={newGoalDeadline}
                  onChange={(e) => setNewGoalDeadline(e.target.value)}
                  className="w-full border border-slate-300 dark:border-slate-700 rounded p-1.5 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100"
                />
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-2 border-t border-slate-200 dark:border-slate-800">
              <button
                onClick={() => setIsGoalModalOpen(false)}
                className="px-3 py-1.5 rounded border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800"
              >
                Cancelar
              </button>
              <button
                onClick={handleCreateGoal}
                className="px-4 py-1.5 rounded bg-emerald-600 hover:bg-emerald-700 text-white font-bold"
              >
                Salvar Meta
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
