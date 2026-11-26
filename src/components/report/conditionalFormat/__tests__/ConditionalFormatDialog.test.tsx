// @vitest-environment jsdom
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { ConditionalFormatDialog } from '../ConditionalFormatDialog';
import type { Visual } from '../../../../types/bi';

/**
 * jsdom não calcula layout real, então `getBoundingClientRect` retorna 0
 * por padrão. Para o teste de regressão de largura (ver skill
 * tailwind-width-class-override — campos colapsando a ~20px quando uma
 * classe `w-full` compartilhada vence a largura pretendida), simulamos as
 * larguras reais das utilidades Tailwind usadas no diálogo a partir do
 * `className` do elemento. Isso detecta a mesma classe de bug: se um input
 * perder sua classe de largura (ou herdar um `w-full`/`w-20` conflitante
 * de uma string compartilhada), o mapeamento cai para o padrão de 18px.
 */
const TAILWIND_WIDTH_PX: Record<string, number> = {
  'w-10': 40,
  'w-14': 56,
  'w-16': 64,
  'w-20': 80,
  'w-24': 96,
  'w-28': 112,
  'w-full': 600,
};

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: function (this: HTMLElement) {
      const classes = (this.className || '').split(/\s+/).filter(Boolean);
      let width = 18; // colapsado, como no bug real
      for (const cls of classes) {
        if (TAILWIND_WIDTH_PX[cls]) {
          width = TAILWIND_WIDTH_PX[cls];
          break;
        }
      }
      if (classes.includes('flex-1')) width = Math.max(width, 140);
      return { width, height: 28, top: 0, left: 0, right: width, bottom: 28, x: 0, y: 0, toJSON() {} };
    },
  });
});

afterAll(() => {
  // @ts-expect-error restaura o comportamento padrão do jsdom
  delete HTMLElement.prototype.getBoundingClientRect;
});

const visual: Visual = {
  id: 'v1',
  type: 'table',
  title: 'Tabela de Vendas',
  x: 0,
  y: 0,
  w: 400,
  h: 300,
  dataConfig: { tableId: 'Vendas', yAxisFields: ['Valor', 'Custo'], xAxisField: 'Produto' },
  formatConfig: { showTitle: true, showDataLabels: false, showLegend: true },
};

function allFormFields() {
  const dialog = screen.getByTestId('condfmt-dialog');
  return within(dialog).getAllByRole('combobox').concat(
    Array.from(dialog.querySelectorAll('input')) as unknown as HTMLElement[]
  );
}

describe('ConditionalFormatDialog', () => {
  it('cabe em 1366×768 e todo campo tem largura > 60px (regressão do bug de colapso)', () => {
    render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={() => {}}
        onClose={() => {}}
      />
    );

    const dialog = screen.getByTestId('condfmt-dialog');
    const dialogRect = dialog.getBoundingClientRect();
    expect(dialogRect.width).toBeLessThan(1366);

    for (const field of allFormFields()) {
      if (field.getAttribute('type') === 'checkbox') continue; // caixas de seleção não são campos de largura
      const rect = field.getBoundingClientRect();
      expect(rect.width).toBeGreaterThan(60);
    }
  });

  it('digita em todos os campos do modo "rules" e salva o ConditionalFormat correto', () => {
    const onSave = vi.fn();
    render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={onSave}
        onClose={() => {}}
      />
    );

    fireEvent.change(screen.getByLabelText('Aplicado a'), { target: { value: 'Valor' } });
    fireEvent.change(screen.getByLabelText('O que formatar'), { target: { value: 'background' } });
    fireEvent.change(screen.getByLabelText('Com base em'), { target: { value: 'Custo' } });
    fireEvent.change(screen.getByLabelText('Tipo do campo base'), { target: { value: 'column' } });
    fireEvent.change(screen.getByLabelText('Estilo de formato'), { target: { value: 'rules' } });

    fireEvent.change(screen.getByLabelText('Operador da regra 1'), { target: { value: '>=' } });
    fireEvent.change(screen.getByLabelText('Valor da regra 1'), { target: { value: '1000' } });
    fireEvent.change(screen.getByLabelText('Unidade da regra 1'), { target: { value: 'percent' } });
    fireEvent.change(screen.getByLabelText('Cor da regra 1'), { target: { value: '#112233' } });

    fireEvent.click(screen.getByText('Adicionar regra'));
    fireEvent.change(screen.getByLabelText('Operador da regra 2'), { target: { value: 'between' } });
    fireEvent.change(screen.getByLabelText('Valor da regra 2'), { target: { value: '10' } });
    fireEvent.change(screen.getByLabelText('Valor 2 da regra 2'), { target: { value: '20' } });
    fireEvent.change(screen.getByLabelText('Cor da regra 2'), { target: { value: '#445566' } });

    fireEvent.click(screen.getByText('OK'));

    expect(onSave).toHaveBeenCalledTimes(1);
    const saved = onSave.mock.calls[0][0];
    expect(saved.applyTo).toBe('Valor');
    expect(saved.target).toBe('background');
    expect(saved.basedOn).toEqual({ field: 'Custo', kind: 'column' });
    expect(saved.mode).toBe('rules');
    expect(saved.rules).toEqual([
      { operator: '>=', value: 1000, color: '#112233', valueKind: 'percent' },
      { operator: 'between', value: 10, value2: 20, color: '#445566', valueKind: 'number' },
    ]);
  });

  it('digita em todos os campos do modo "gradient" (2 e 3 cores) e salva', () => {
    const onSave = vi.fn();
    render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={onSave}
        onClose={() => {}}
      />
    );

    fireEvent.change(screen.getByLabelText('Estilo de formato'), { target: { value: 'gradient' } });
    fireEvent.change(screen.getByLabelText('Mínimo - tipo'), { target: { value: 'number' } });
    fireEvent.change(screen.getByLabelText('Mínimo - valor'), { target: { value: '5' } });
    fireEvent.change(screen.getByLabelText('Mínimo - cor'), { target: { value: '#ff0000' } });

    fireEvent.click(screen.getByLabelText('Ativar ponto médio'));
    fireEvent.change(screen.getByLabelText('Médio - tipo'), { target: { value: 'percent' } });
    fireEvent.change(screen.getByLabelText('Médio - valor'), { target: { value: '50' } });
    fireEvent.change(screen.getByLabelText('Médio - cor'), { target: { value: '#ffff00' } });

    fireEvent.change(screen.getByLabelText('Máximo - tipo'), { target: { value: 'number' } });
    fireEvent.change(screen.getByLabelText('Máximo - valor'), { target: { value: '100' } });
    fireEvent.change(screen.getByLabelText('Máximo - cor'), { target: { value: '#00ff00' } });

    expect(screen.getByTestId('gradient-preview')).toBeTruthy();

    fireEvent.click(screen.getByText('OK'));
    const saved = onSave.mock.calls[0][0];
    expect(saved.gradient).toEqual({
      min: { kind: 'number', value: 5, color: '#ff0000' },
      mid: { kind: 'percent', value: 50, color: '#ffff00' },
      max: { kind: 'number', value: 100, color: '#00ff00' },
    });
  });

  it('digita nos campos de barra de dados e de ícones, e salva cada um', () => {
    const onSaveBar = vi.fn();
    const { unmount } = render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={onSaveBar}
        onClose={() => {}}
      />
    );
    fireEvent.change(screen.getByLabelText('O que formatar'), { target: { value: 'dataBar' } });
    fireEvent.change(screen.getByLabelText('Cor positiva da barra'), { target: { value: '#00ff00' } });
    fireEvent.change(screen.getByLabelText('Cor negativa da barra'), { target: { value: '#ff0000' } });
    fireEvent.click(screen.getByLabelText('Mostrar somente a barra'));
    fireEvent.click(screen.getByText('OK'));
    const savedBar = onSaveBar.mock.calls[0][0];
    expect(savedBar.target).toBe('dataBar');
    expect(savedBar.dataBar).toEqual({ positiveColor: '#00ff00', negativeColor: '#ff0000', showBarOnly: true });
    unmount();

    const onSaveIcon = vi.fn();
    render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={onSaveIcon}
        onClose={() => {}}
      />
    );
    fireEvent.change(screen.getByLabelText('O que formatar'), { target: { value: 'icon' } });
    fireEvent.change(screen.getByLabelText('Conjunto de ícones'), { target: { value: 'arrows' } });
    fireEvent.change(screen.getByLabelText('Operador do ícone 1'), { target: { value: '>' } });
    fireEvent.change(screen.getByLabelText('Valor do ícone 1'), { target: { value: '80' } });
    fireEvent.change(screen.getByLabelText('Operador do ícone 2'), { target: { value: '>' } });
    fireEvent.change(screen.getByLabelText('Valor do ícone 2'), { target: { value: '40' } });
    fireEvent.change(screen.getByLabelText('Operador do ícone 3'), { target: { value: '<=' } });
    fireEvent.change(screen.getByLabelText('Valor do ícone 3'), { target: { value: '40' } });
    fireEvent.click(screen.getByText('OK'));
    const savedIcon = onSaveIcon.mock.calls[0][0];
    expect(savedIcon.target).toBe('icon');
    expect(savedIcon.icons.set).toBe('arrows');
    expect(savedIcon.icons.rules).toEqual([
      { operator: '>', value: 80, color: '' },
      { operator: '>', value: 40, color: '' },
      { operator: '<=', value: 40, color: '' },
    ]);
  });

  it('modo "fieldValue" não exige configuração extra e salva sem rules/gradient', () => {
    const onSave = vi.fn();
    render(
      <ConditionalFormatDialog
        visual={visual}
        availableFields={['Valor', 'Custo', 'Produto']}
        measureFields={['Valor', 'Custo']}
        initial={null}
        onSave={onSave}
        onClose={() => {}}
      />
    );
    fireEvent.change(screen.getByLabelText('O que formatar'), { target: { value: 'fontColor' } });
    fireEvent.change(screen.getByLabelText('Estilo de formato'), { target: { value: 'fieldValue' } });
    fireEvent.click(screen.getByText('OK'));
    const saved = onSave.mock.calls[0][0];
    expect(saved.mode).toBe('fieldValue');
    expect(saved.rules).toBeUndefined();
    expect(saved.gradient).toBeUndefined();
  });
});
