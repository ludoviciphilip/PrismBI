// @vitest-environment jsdom
import React from 'react';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent, cleanup, within, act } from '@testing-library/react';

/**
 * ECharts não desenha no jsdom: o mock guarda as opções e os handlers do último
 * gráfico, para o teste ler as categorias/valores e simular clique/botão direito.
 */
const chart: { option: any; onEvents: Record<string, (p: any) => void> } = { option: null, onEvents: {} };
vi.mock('echarts-for-react', () => ({
  default: (props: any) => {
    chart.option = props.option;
    chart.onEvents = props.onEvents || {};
    return <div data-testid="echarts" />;
  },
}));

/**
 * jsdom não tem layout: getBoundingClientRect devolve 0. Como no teste de larguras
 * do segmentador (skill tailwind-width-class-override), mapeamos as classes de
 * largura usadas pelos controles novos para px reais, com 18px de "colapsado".
 */
const TAILWIND_WIDTH_PX: Record<string, number> = { 'w-6': 24, 'w-52': 208 };
beforeAll(() => {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1366 });
  Object.defineProperty(window, 'innerHeight', { configurable: true, value: 768 });
  Object.defineProperty(HTMLElement.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: function (this: HTMLElement) {
      const classes = (this.className || '').toString().split(/\s+/).filter(Boolean);
      let width = 18;
      for (const cls of classes) if (TAILWIND_WIDTH_PX[cls]) width = TAILWIND_WIDTH_PX[cls];
      const left = parseFloat(this.style?.left || '0') || 0;
      const top = parseFloat(this.style?.top || '0') || 0;
      const height = classes.includes('h-6') ? 24 : 28;
      return { width, height, top, left, right: left + width, bottom: top + height, x: left, y: top, toJSON() {} };
    },
  });
});
afterAll(() => {
  // @ts-expect-error restaura o padrão do jsdom
  delete HTMLElement.prototype.getBoundingClientRect;
});
afterEach(() => cleanup());

import { useBiStore } from '../../../../store/useBiStore';
import { globalDataEngine } from '../../../../engine/dataEngine';
import { VisualCard } from '../../VisualCard';
import { Measure, Table, Visual } from '../../../../types/bi';
import { isDrillAll } from '../../../../engine/drill';

const pedidos: Table = {
  id: 'fp',
  name: 'fPedidos',
  originalName: 'fPedidos',
  columns: [
    { id: 'd', name: 'Data', originalName: 'Data', dataType: 'date' },
    { id: 'c', name: 'Categoria', originalName: 'Categoria', dataType: 'text' },
    { id: 'p', name: 'Produto', originalName: 'Produto', dataType: 'text' },
    { id: 'v', name: 'Valor', originalName: 'Valor', dataType: 'number' },
  ],
  rows: [
    { Data: '2023-11-05', Categoria: 'A', Produto: 'a1', Valor: 10 },
    { Data: '2024-01-10', Categoria: 'A', Produto: 'a2', Valor: 5 },
    { Data: '2024-02-20', Categoria: 'B', Produto: 'b1', Valor: 7 },
    { Data: '2024-04-01', Categoria: 'B', Produto: 'b1', Valor: 3 },
    { Data: '2024-12-31', Categoria: 'B', Produto: 'b2', Valor: 1 },
  ],
} as Table;

const lojas: Table = {
  id: 'fv',
  name: 'fVendas',
  originalName: 'fVendas',
  columns: ['Regiao', 'Cidade', 'Loja', 'Cliente'].map((n, i) => ({ id: `k${i}`, name: n, originalName: n, dataType: 'text' as const })),
  rows: [
    ['Sul', 'POA', 'L1', 'c1'],
    ['Sul', 'POA', 'L1', 'c2'],
    ['Sul', 'POA', 'L2', 'c1'],
    ['Sul', 'Curitiba', 'L3', 'c1'],
    ['Sul', 'Curitiba', 'L3', 'c3'],
    ['Norte', 'Manaus', 'L4', 'c2'],
    ['Norte', 'Manaus', 'L4', 'c4'],
  ].map(([Regiao, Cidade, Loja, Cliente]) => ({ Regiao, Cidade, Loja, Cliente })),
} as Table;

const measures: Measure[] = [
  { id: 'm1', name: 'Total', formula: 'SUM(fPedidos[Valor])', tableName: 'fPedidos', format: 'number' },
  { id: 'm2', name: 'Clientes', formula: 'DISTINCTCOUNT(fVendas[Cliente])', tableName: 'fVendas', format: 'number' },
];

function seed(visual: Visual) {
  const st = useBiStore.getState();
  st.newProject('Drill UI');
  st.setTables([pedidos, lojas]);
  useBiStore.setState({ measures, filters: [] });
  globalDataEngine.setSchema([pedidos, lojas], [], measures);
  const pageId = useBiStore.getState().activePageId;
  useBiStore.setState(s => ({ pages: [{ ...s.pages[0], id: pageId, visuals: [visual] }] }));
}

const LiveCard: React.FC<{ id: string }> = ({ id }) => {
  const v = useBiStore(s => s.pages.flatMap(p => p.visuals).find(x => x.id === id))!;
  return <VisualCard visual={v} isSelected={false} onSelect={() => {}} />;
};
const getVisual = () => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === 'v1')!;

const chartVisual = (over: Partial<Visual['dataConfig']> = {}, type: Visual['type'] = 'column'): Visual => ({
  id: 'v1',
  type,
  title: 'Vendas por data',
  x: 0,
  y: 0,
  w: 480,
  h: 300,
  dataConfig: { tableId: 'fp', xAxisField: 'Data', yAxisFields: ['Total'], aggregation: 'NONE', drillMode: 'drill', ...over },
  formatConfig: { showTitle: true },
});

const cats = () => chart.option?.xAxis?.data ?? chart.option?.yAxis?.data ?? chart.option?.series?.[0]?.data?.map((d: any) => d.name);
const vals = () => chart.option.series[0].data.map((d: any) => (typeof d === 'number' ? d : d.value ?? d.rawValue));
const card = () => document.querySelector('[data-visual-id="v1"]') as HTMLElement;
const hover = () => fireEvent.mouseEnter(card());

describe('Drill em gráficos: controles do cabeçalho (hover) e hierarquia de data automática', () => {
  it('controles só aparecem no hover; nível inicial = Ano', () => {
    seed(chartVisual());
    render(<LiveCard id="v1" />);
    expect(screen.queryByTestId('drill-controls')).toBeNull();
    expect(cats()).toEqual(['2023', '2024']);
    expect(vals()).toEqual([10, 16]);
    hover();
    const group = screen.getByTestId('drill-controls');
    expect(within(group).getAllByRole('button')).toHaveLength(4);
    const up = screen.getByRole('button', { name: 'Fazer drill up' }) as HTMLButtonElement;
    expect(up.disabled).toBe(true);
    expect(up.title).toMatch(/nível mais alto/);
    expect(up.className).toContain('disabled:opacity-40');
    fireEvent.mouseLeave(card());
    expect(screen.queryByTestId('drill-controls')).toBeNull();
  });

  it('⇊ próximo nível (todos os anos) e ↑ drill up', () => {
    seed(chartVisual());
    render(<LiveCard id="v1" />);
    hover();
    fireEvent.click(screen.getByRole('button', { name: 'Ir para o próximo nível' }));
    expect(getVisual().dataConfig.currentDrillLevel).toBe(1);
    expect(isDrillAll(getVisual().dataConfig.drillPath?.[0])).toBe(true);
    expect(cats()).toEqual(['Trim 1', 'Trim 2', 'Trim 4']);
    expect(vals()).toEqual([12, 3, 11]);
    fireEvent.click(screen.getByRole('button', { name: 'Fazer drill up' }));
    expect(getVisual().dataConfig.currentDrillLevel).toBe(0);
    expect(cats()).toEqual(['2023', '2024']);
  });

  it('↓ modo drill: clicar num ponto desce filtrando por ele', () => {
    seed(chartVisual());
    render(<LiveCard id="v1" />);
    hover();
    const toggle = screen.getByRole('button', { name: 'Modo drill down' });
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    act(() => chart.onEvents.click({ name: '2024' }));
    expect(getVisual().dataConfig.drillPath).toEqual([2024]);
    expect(cats()).toEqual(['Trim 1', 'Trim 2', 'Trim 4']);
    expect(vals()).toEqual([12, 3, 1]);
    act(() => chart.onEvents.click({ name: 'Trim 1' }));
    expect(getVisual().dataConfig.drillPath).toEqual([2024, 1]);
    expect(cats()).toEqual(['janeiro', 'fevereiro']);
    expect(vals()).toEqual([5, 7]);
    // Sem o modo drill, o clique não desce (fica no filtro cruzado normal).
    fireEvent.click(screen.getByRole('button', { name: 'Modo drill down' }));
    act(() => chart.onEvents.click({ name: 'janeiro' }));
    expect(getVisual().dataConfig.currentDrillLevel).toBe(2);
  });

  it('expandir todos os níveis concatena os rótulos ("2024 Trim 1")', () => {
    seed(chartVisual({}, 'bar'));
    render(<LiveCard id="v1" />);
    hover();
    fireEvent.click(screen.getByRole('button', { name: 'Expandir todos os níveis' }));
    expect(getVisual().dataConfig.drillMode).toBe('expand');
    expect(cats()).toEqual(['2023 Trim 4', '2024 Trim 1', '2024 Trim 2', '2024 Trim 4']);
    expect(vals()).toEqual([10, 12, 3, 1]);
  });

  it('pizza: drillFields explícitos (Categoria → Produto)', () => {
    seed(chartVisual({ xAxisField: 'Categoria', drillFields: ['Produto'] }, 'pie'));
    render(<LiveCard id="v1" />);
    expect(cats()).toEqual(['A', 'B']);
    hover();
    fireEvent.click(screen.getByRole('button', { name: 'Modo drill down' }));
    act(() => chart.onEvents.click({ name: 'B' }));
    expect(cats()).toEqual(['b1', 'b2']);
    expect(chart.option.series[0].data.map((d: any) => d.rawValue)).toEqual([10, 1]);
    // último nível: os botões de descer ficam desabilitados, com tooltip
    const next = screen.getByRole('button', { name: 'Ir para o próximo nível' }) as HTMLButtonElement;
    expect(next.disabled).toBe(true);
    expect(next.title).toMatch(/nível mais baixo/);
  });

  it('visual salvo antes do v0.10 (sem drillMode) num eixo de data não ganha hierarquia', () => {
    seed(chartVisual({ drillMode: undefined }));
    render(<LiveCard id="v1" />);
    expect(cats()).toEqual(['2023-11-05', '2024-01-10', '2024-02-20', '2024-04-01', '2024-12-31']);
    hover();
    expect(screen.queryByTestId('drill-controls')).toBeNull();
    cleanup();
    seed(chartVisual({ drillFields: [] }));
    render(<LiveCard id="v1" />);
    hover();
    expect(screen.queryByTestId('drill-controls')).toBeNull();
  });

  it('menu de contexto: drill down/up, mostrar como tabela e cabe em 1366×768', () => {
    seed(chartVisual());
    render(<LiveCard id="v1" />);
    const preventDefault = vi.fn();
    act(() => chart.onEvents.contextmenu({ name: '2024', event: { event: { clientX: 1360, clientY: 760, preventDefault } } }));
    expect(preventDefault).toHaveBeenCalled();
    const menu = screen.getByTestId('drill-context-menu');
    expect(within(menu).getAllByRole('menuitem').map(b => b.textContent)).toEqual([
      'Fazer drill down',
      'Fazer drill up',
      'Mostrar como tabela',
      'Drill-through',
    ]);
    const r = menu.getBoundingClientRect();
    expect(r.width).toBe(208);
    expect(r.right).toBeLessThanOrEqual(1366);
    expect(r.bottom).toBeLessThanOrEqual(768);
    expect((screen.getByRole('menuitem', { name: 'Fazer drill up' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('menuitem', { name: 'Drill-through' }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Fazer drill down' }));
    expect(getVisual().dataConfig.drillPath).toEqual([2024]);
    expect(screen.queryByTestId('drill-context-menu')).toBeNull();

    act(() => chart.onEvents.contextmenu({ name: 'Trim 1', event: { event: { clientX: 10, clientY: 10, preventDefault } } }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Fazer drill up' }));
    expect(getVisual().dataConfig.currentDrillLevel).toBe(0);

    act(() => chart.onEvents.contextmenu({ name: '2023', event: { event: { clientX: 10, clientY: 10, preventDefault } } }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Mostrar como tabela' }));
    expect(screen.queryByTestId('echarts')).toBeNull();
  });

  it('botões do cabeçalho mantêm 24px (não colapsam) e o grupo cabe no visual', () => {
    seed(chartVisual());
    render(<LiveCard id="v1" />);
    hover();
    const buttons = within(screen.getByTestId('drill-controls')).getAllByRole('button');
    const total = buttons.reduce((acc, b) => acc + b.getBoundingClientRect().width, 0);
    buttons.forEach(b => expect(b.getBoundingClientRect().width).toBe(24));
    expect(total).toBeLessThanOrEqual(480 / 4);
  });
});

const matrixVisual = (over: Partial<Visual['dataConfig']> = {}): Visual => ({
  id: 'v1',
  type: 'matrix',
  title: 'Clientes',
  x: 0,
  y: 0,
  w: 520,
  h: 360,
  dataConfig: {
    tableId: 'fv',
    xAxisField: 'Regiao',
    matrixRowFields: ['Regiao', 'Cidade', 'Loja'],
    yAxisFields: ['Clientes'],
    aggregation: 'NONE',
    ...over,
  },
  formatConfig: { showTitle: true },
});

const rowLabels = () =>
  Array.from(document.querySelectorAll('[data-testid="hierarchical-matrix"] tbody tr')).map(
    tr => `${tr.getAttribute('data-depth')}:${tr.querySelector('td span')?.textContent}=${tr.querySelectorAll('td')[1]?.textContent}`
  );

describe('Matriz com N níveis', () => {
  it('+/− por linha expande e recolhe; subtotais vêm do motor (DISTINCTCOUNT)', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3']);
    expect(screen.getByTestId('matrix-total-Clientes').textContent).toBe('4'); // não 2+3=5

    fireEvent.click(screen.getByRole('button', { name: 'Expandir Sul' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3', '1:Curitiba=2', '1:POA=2']);
    fireEvent.click(screen.getByRole('button', { name: 'Expandir POA' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3', '1:Curitiba=2', '1:POA=2', '2:L1=2', '2:L2=1']);
    expect(screen.getByRole('button', { name: 'Recolher POA' }).getAttribute('aria-expanded')).toBe('true');
    // L1/L2 são o último nível: sem botão +/−
    expect(screen.queryByRole('button', { name: /Expandir L1/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Recolher Sul' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3']);
  });

  it('cabeçalho (hover): expandir um nível, expandir tudo, recolher tudo', () => {
    seed(matrixVisual());
    render(<LiveCard id="v1" />);
    hover();
    const collapse = screen.getByRole('button', { name: 'Recolher tudo' }) as HTMLButtonElement;
    expect(collapse.disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Expandir um nível' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '1:Manaus=2', '0:Sul=3', '1:Curitiba=2', '1:POA=2']);
    fireEvent.click(screen.getByRole('button', { name: 'Expandir tudo' }));
    expect(rowLabels()).toHaveLength(9);
    expect((screen.getByRole('button', { name: 'Expandir tudo' }) as HTMLButtonElement).disabled).toBe(true);
    // recolher uma linha depois do "expandir tudo"
    fireEvent.click(screen.getByRole('button', { name: 'Recolher Norte' }));
    expect(rowLabels().filter(r => r.includes('Manaus'))).toEqual([]);
    fireEvent.click(screen.getByRole('button', { name: 'Recolher tudo' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3']);
    within(screen.getByTestId('matrix-drill-controls'))
      .getAllByRole('button')
      .forEach(b => expect(b.getBoundingClientRect().width).toBe(24));
  });

  it('matriz antiga de 2 níveis (xAxisField + drillFields[0]) continua expandindo igual', () => {
    seed(matrixVisual({ matrixRowFields: undefined, drillFields: ['Cidade'] }));
    render(<LiveCard id="v1" />);
    expect(rowLabels()).toEqual(['0:Norte=2', '0:Sul=3']);
    fireEvent.click(screen.getByRole('button', { name: 'Expandir Norte' }));
    expect(rowLabels()).toEqual(['0:Norte=2', '1:Manaus=2', '0:Sul=3']);
    // o cabeçalho da 1ª coluna continua sendo o campo das linhas
    expect(screen.getByTestId('hierarchical-matrix').querySelector('thead th')?.textContent).toContain('Regiao');
  });

  it('matriz de 1 nível continua usando a matriz antiga', () => {
    seed(matrixVisual({ matrixRowFields: undefined }));
    render(<LiveCard id="v1" />);
    expect(screen.queryByTestId('hierarchical-matrix')).toBeNull();
  });

  it('hover numa linha (seams F↔G): a dica de página filtra pelo caminho INTEIRO da linha', () => {
    vi.useFakeTimers();
    const st = useBiStore.getState();
    st.newProject('Drill UI');
    st.setTables([pedidos, lojas]);
    useBiStore.setState({ measures, filters: [] });
    globalDataEngine.setSchema([pedidos, lojas], [], measures);
    const pageId = useBiStore.getState().activePageId;
    const mainVisual = matrixVisual();
    (mainVisual as any).formatConfig = { showTitle: true, tooltip: { type: 'page', pageId: 'tt' } };
    const tooltipVisual: Visual = {
      id: 'v2',
      type: 'bar',
      title: 'Lojas',
      x: 0,
      y: 0,
      w: 200,
      h: 140,
      dataConfig: { tableId: 'fv', xAxisField: 'Loja', yAxisFields: ['Clientes'], aggregation: 'NONE' },
      formatConfig: { showTitle: true },
    };
    useBiStore.setState(s => ({
      pages: [
        { ...s.pages[0], id: pageId, visuals: [mainVisual] },
        { id: 'tt', name: 'Dica', order: 2, visuals: [tooltipVisual], tooltipPage: { enabled: true, fields: ['Loja'] } },
      ],
    }));
    render(<LiveCard id="v1" />);
    fireEvent.click(screen.getByRole('button', { name: 'Expandir Sul' }));

    const poaRow = Array.from(document.querySelectorAll('[data-testid="hierarchical-matrix"] tbody tr')).find(
      tr => tr.querySelector('td span')?.textContent === 'POA'
    )!;
    act(() => fireEvent.mouseEnter(poaRow.querySelector('td')!));
    act(() => vi.advanceTimersByTime(200));

    const popover = screen.getByTestId('tooltip-page-popover');
    expect(within(popover).getByTestId('echarts')).toBeTruthy();
    // POA (Regiao=Sul, Cidade=POA) só tem as lojas L1 e L2, nunca as 4 da tabela toda.
    expect(cats()).toEqual(['L1', 'L2']);
    vi.useRealTimers();
  });
});

describe('Frente H→G: formatação condicional nos totais/subtotais da matriz hierárquica', () => {
  const valueCell = (label: string) =>
    Array.from(document.querySelectorAll('[data-testid="hierarchical-matrix"] tbody tr')).find(
      tr => tr.querySelector('td span')?.textContent === label
    )!.querySelectorAll('td')[1] as HTMLElement;

  it('sem applyToTotals, a regra colore só os valores folha, não os subtotais nem o total geral', () => {
    seed(
      matrixVisual({
        formatConfig: undefined,
      } as any)
    );
    const v = getVisual();
    v.formatConfig = {
      showTitle: true,
      conditionalFormats: [
        { id: 'f1', target: 'background', applyTo: 'Clientes', mode: 'rules', rules: [{ operator: '>=', value: 2, color: '#ff0000' }] },
      ],
    };
    useBiStore.setState(s => ({ pages: s.pages.map(p => ({ ...p, visuals: p.visuals.map(x => (x.id === 'v1' ? v : x)) })) }));
    render(<LiveCard id="v1" />);
    // "Sul" (subtotal, valor 3) não deve ficar vermelho: applyToTotals é falso por padrão.
    expect(valueCell('Sul').style.backgroundColor).not.toBe('rgb(255, 0, 0)');
    expect(screen.getByTestId('matrix-total-Clientes').style.backgroundColor).not.toBe('rgb(255, 0, 0)');
    fireEvent.click(screen.getByRole('button', { name: 'Expandir Sul' }));
    fireEvent.click(screen.getByRole('button', { name: 'Expandir POA' }));
    // "L1" é folha (valor 2): deve colorir normalmente.
    expect(valueCell('L1').style.backgroundColor).toBe('rgb(255, 0, 0)');
  });

  it('com applyToTotals, subtotais, total de linha, totais de coluna e total geral usam a escala própria', () => {
    seed(matrixVisual());
    const v = getVisual();
    v.formatConfig = {
      showTitle: true,
      conditionalFormats: [
        { id: 'f1', target: 'background', applyTo: 'Clientes', mode: 'rules', applyToTotals: true, rules: [{ operator: '>=', value: 3, color: '#00ff00' }] },
      ],
    };
    useBiStore.setState(s => ({ pages: s.pages.map(p => ({ ...p, visuals: p.visuals.map(x => (x.id === 'v1' ? v : x)) })) }));
    render(<LiveCard id="v1" />);
    // "Sul" (subtotal de nível 0 = 3) entra na regra >=3.
    expect(valueCell('Sul').style.backgroundColor).toBe('rgb(0, 255, 0)');
    // "Norte" (subtotal de nível 0 = 2) não entra.
    expect(valueCell('Norte').style.backgroundColor).not.toBe('rgb(0, 255, 0)');
    // Total geral (4) >= 3.
    expect(screen.getByTestId('matrix-total-Clientes').style.backgroundColor).toBe('rgb(0, 255, 0)');
  });
});

describe('Seams F↔G: a dica de página respeita o nível/caminho de drill (não o xAxisField cru)', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('ponto "Trim 1" (expandido, Ano+Trimestre) filtra a página de dica por ano=2024 e trimestre=1', () => {
    vi.useFakeTimers();
    const st = useBiStore.getState();
    st.newProject('Drill UI');
    st.setTables([pedidos, lojas]);
    useBiStore.setState({ measures, filters: [] });
    globalDataEngine.setSchema([pedidos, lojas], [], measures);
    const pageId = useBiStore.getState().activePageId;
    const mainVisual: Visual = {
      id: 'v1',
      type: 'bar',
      title: 'Vendas por data',
      x: 0,
      y: 0,
      w: 480,
      h: 300,
      dataConfig: { tableId: 'fp', xAxisField: 'Data', yAxisFields: ['Total'], aggregation: 'NONE', drillMode: 'drill' },
      formatConfig: { showTitle: true, tooltip: { type: 'page', pageId: 'tt' } },
    };
    const tooltipVisual: Visual = {
      id: 'v2',
      type: 'column',
      title: 'Por produto',
      x: 0,
      y: 0,
      w: 200,
      h: 140,
      dataConfig: { tableId: 'fp', xAxisField: 'Produto', yAxisFields: ['Total'], aggregation: 'NONE' },
      formatConfig: { showTitle: true },
    };
    useBiStore.setState(s => ({
      pages: [
        { ...s.pages[0], id: pageId, visuals: [mainVisual] },
        { id: 'tt', name: 'Dica', order: 2, visuals: [tooltipVisual], tooltipPage: { enabled: true, fields: ['Data'] } },
      ],
    }));

    render(<LiveCard id="v1" />);
    hover();
    // Expande todos os níveis (rótulo concatenado "2024 Trim 1")
    fireEvent.click(screen.getByRole('button', { name: 'Expandir todos os níveis' }));
    expect(cats()).toContain('2024 Trim 1');

    act(() => chart.onEvents.mouseover({ name: '2024 Trim 1', event: { event: { clientX: 10, clientY: 10 } } }));
    act(() => vi.advanceTimersByTime(200));

    const popover = screen.getByTestId('tooltip-page-popover');
    // Só Jan (a2=5) e Fev (b1=7) de 2024 caem em Q1: filtro certo = produtos a2/b1.
    const prodCats = within(popover).queryAllByTestId('echarts');
    expect(prodCats.length).toBeGreaterThan(0);
    expect(cats()).toEqual(['a2', 'b1']);
  });
});
