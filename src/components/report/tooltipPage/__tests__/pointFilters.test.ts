import { describe, expect, it } from 'vitest';
import { buildTemporaryFilters, tooltipCacheKey } from '../pointFilters';

describe('buildTemporaryFilters', () => {
  it('categoria + legenda -> dois filtros temporários de nível visual', () => {
    const filters = buildTemporaryFilters('f_Vendas', {
      categoryField: 'Regiao',
      categoryValue: 'Sudeste',
      legendField: 'Categoria',
      legendValue: 'Eletrônicos',
    });
    expect(filters).toHaveLength(2);
    expect(filters[0]).toMatchObject({ level: 'visual', tableId: 'f_Vendas', column: 'Regiao', operator: 'equals', value: 'Sudeste' });
    expect(filters[1]).toMatchObject({ level: 'visual', tableId: 'f_Vendas', column: 'Categoria', operator: 'equals', value: 'Eletrônicos' });
  });

  it('só categoria (sem legenda) -> um único filtro', () => {
    const filters = buildTemporaryFilters('f_Vendas', { categoryField: 'Regiao', categoryValue: 'Sul' });
    expect(filters).toHaveLength(1);
    expect(filters[0].column).toBe('Regiao');
  });

  it('valores vazios/indefinidos não geram filtro', () => {
    expect(buildTemporaryFilters('f_Vendas', { categoryField: 'Regiao', categoryValue: undefined })).toHaveLength(0);
    expect(buildTemporaryFilters('f_Vendas', { categoryField: 'Regiao', categoryValue: '' })).toHaveLength(0);
    expect(buildTemporaryFilters('f_Vendas', {})).toHaveLength(0);
  });

  it('legenda igual à categoria não duplica o filtro', () => {
    const filters = buildTemporaryFilters('f_Vendas', {
      categoryField: 'Regiao',
      categoryValue: 'Sul',
      legendField: 'Regiao',
      legendValue: 'Sul',
    });
    expect(filters).toHaveLength(1);
  });

  it('o valor 0 (número) é um valor usável, não é tratado como vazio', () => {
    const filters = buildTemporaryFilters('f_Vendas', { categoryField: 'Ano', categoryValue: 0 });
    expect(filters).toHaveLength(1);
    expect(filters[0].value).toBe(0);
  });
});

describe('tooltipCacheKey', () => {
  it('é estável para os mesmos filtros em ordem diferente', () => {
    const a = buildTemporaryFilters('f_Vendas', { categoryField: 'Regiao', categoryValue: 'Sul', legendField: 'Categoria', legendValue: 'A' });
    const b = buildTemporaryFilters('f_Vendas', { categoryField: 'Categoria', categoryValue: 'A', legendField: 'Regiao', legendValue: 'Sul' });
    expect(tooltipCacheKey('page_1', a)).toBe(tooltipCacheKey('page_1', [a[1], a[0]]));
  });

  it('muda quando a página ou os filtros mudam', () => {
    const f = buildTemporaryFilters('f_Vendas', { categoryField: 'Regiao', categoryValue: 'Sul' });
    expect(tooltipCacheKey('page_1', f)).not.toBe(tooltipCacheKey('page_2', f));
  });
});
