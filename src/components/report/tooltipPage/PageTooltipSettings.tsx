import React from 'react';
import { CanvasSizeConfig, Page } from '../../../types/bi';
import { CANVAS_PRESETS } from '../../../engine/shared/canvasPresets';

interface PageTooltipSettingsProps {
  page: Page;
  availableFields: string[];
  onChange: (tooltipPage: NonNullable<Page['tooltipPage']>) => void;
  onSuggestCanvasSize: (canvasSize: CanvasSizeConfig) => void;
}

const TOOLTIP_PRESET = CANVAS_PRESETS.find(p => p.id === 'tooltip')!;

/**
 * "Informações da página → Permitir uso como dica de ferramenta", no painel
 * de formato quando nenhum visual está selecionado. Ao ativar, sugere o
 * tamanho de tela 'Dica de ferramenta' (320×240) se a página ainda não
 * estiver nesse tamanho.
 */
export const PageTooltipSettings: React.FC<PageTooltipSettingsProps> = ({
  page,
  availableFields,
  onChange,
  onSuggestCanvasSize,
}) => {
  const tooltipPage = page.tooltipPage || { enabled: false, fields: [] as string[] };
  const isTooltipSize = page.canvasSize?.preset === 'tooltip';

  const toggleField = (field: string) => {
    const fields = tooltipPage.fields || [];
    const next = fields.includes(field) ? fields.filter(f => f !== field) : [...fields, field];
    onChange({ ...tooltipPage, fields: next });
  };

  return (
    <div className="p-3 border-b border-slate-200 dark:border-slate-800 space-y-2" data-testid="page-tooltip-settings">
      <label className="text-[11px] font-semibold text-slate-700 dark:text-slate-300 block">Informações da página</label>

      <div className="flex items-center justify-between">
        <span className="text-[11px] text-slate-600 dark:text-slate-300">Permitir uso como dica de ferramenta</span>
        <input
          type="checkbox"
          data-testid="tooltip-page-enabled-checkbox"
          checked={tooltipPage.enabled}
          onChange={(e) => {
            const enabled = e.target.checked;
            onChange({ ...tooltipPage, enabled });
            if (enabled && !isTooltipSize) {
              onSuggestCanvasSize({ preset: 'tooltip', width: TOOLTIP_PRESET.width, height: TOOLTIP_PRESET.height });
            }
          }}
          className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
        />
      </div>

      {tooltipPage.enabled && (
        <>
          <p className="text-[10px] text-slate-400">
            Páginas de dica ficam ocultas na navegação do Modo Apresentação.
          </p>

          <div className="space-y-1">
            <span className="text-[10px] uppercase tracking-wide text-slate-400 font-semibold">Campos da dica</span>
            <div className="max-h-28 overflow-y-auto border border-slate-200 dark:border-slate-700 rounded p-1.5 space-y-1">
              {availableFields.length === 0 && (
                <span className="text-[10px] text-slate-400">Sem campos disponíveis no modelo.</span>
              )}
              {availableFields.map(f => (
                <label key={f} className="flex items-center gap-1.5 text-[11px] text-slate-600 dark:text-slate-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={(tooltipPage.fields || []).includes(f)}
                    onChange={() => toggleField(f)}
                    className="rounded border-slate-300 text-amber-500 focus:ring-amber-400"
                  />
                  <span className="truncate">{f}</span>
                </label>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
};
