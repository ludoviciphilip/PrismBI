import { describe, it, expect } from 'vitest';
import { drillConfigFromPbiMeta } from '../pbiImport';

describe('drillConfigFromPbiMeta (importação PBIR)', () => {
  it('gráfico com várias projeções em Category vira drillFields', () => {
    expect(
      drillConfigFromPbiMeta({ categoryField: 'dProduto.Categoria', drillFields: ['dProduto.Subcategoria', 'dProduto.Produto'] }, 'column')
    ).toEqual({ drillFields: ['dProduto.Subcategoria', 'dProduto.Produto'] });
    expect(drillConfigFromPbiMeta({ categoryField: 'A', drillFields: ['B'] }, 'donut')).toEqual({ drillFields: ['B'] });
  });

  it('matriz com várias projeções em Rows vira matrixRowFields (N níveis)', () => {
    expect(drillConfigFromPbiMeta({ categoryField: 'dLoja.Regiao', drillFields: ['dLoja.Cidade', 'dLoja.Loja'] }, 'matrix')).toEqual({
      matrixRowFields: ['dLoja.Regiao', 'dLoja.Cidade', 'dLoja.Loja'],
    });
  });

  it('uma projeção só, visual sem hierarquia ou payload antigo: nada muda', () => {
    expect(drillConfigFromPbiMeta({ categoryField: 'A', drillFields: [] }, 'column')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: 'A' }, 'matrix')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: 'A', drillFields: null }, 'bar')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: 'A', drillFields: ['B'] }, 'card')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: null, drillFields: ['B'] }, 'matrix')).toEqual({});
  });

  it('não liga a hierarquia de data automática (no PBIR uma data "crua" aparece crua)', () => {
    expect(drillConfigFromPbiMeta({ categoryField: 'dCal.Data', drillFields: [] }, 'line')).not.toHaveProperty('drillMode');
  });

  it('v0.11 Rodada 2 (Frente I): matriz com várias projeções em Columns vira matrixColumnFields (N níveis)', () => {
    expect(
      drillConfigFromPbiMeta({ categoryField: 'dLoja.Regiao', drillFields: ['dLoja.Cidade'], legendField: 'dCal.Ano', legendDrillFields: ['dCal.Trimestre'] }, 'matrix')
    ).toEqual({
      matrixRowFields: ['dLoja.Regiao', 'dLoja.Cidade'],
      matrixColumnFields: ['dCal.Ano', 'dCal.Trimestre'],
    });
  });

  it('colunas: 1 projeção só (legenda simples) ou gráfico (não matriz) não ganham matrixColumnFields', () => {
    expect(drillConfigFromPbiMeta({ categoryField: 'A', legendField: 'B', legendDrillFields: [] }, 'matrix')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: 'A', legendField: 'B', legendDrillFields: ['C'] }, 'column')).toEqual({});
    expect(drillConfigFromPbiMeta({ categoryField: 'A', legendField: null, legendDrillFields: ['C'] }, 'matrix')).toEqual({});
  });
});
