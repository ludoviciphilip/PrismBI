import { describe, it, expect } from 'vitest';
import { DaxEngine } from '../daxEngine';
import { InMemoryModelAccess } from '../model/modelAccess';
import { DAX_CONFORMANCE_SUMMARY } from '../compliance/daxCatalog.generated';
import { getSampleDatasets } from '../../data/sampleDatasets';

describe('v0.7.0 DAX Coverage & Expanded Functions', () => {
  const { tables, relationships, measures } = getSampleDatasets();
  const model = new InMemoryModelAccess(tables, relationships, measures);

  it('reaches >= 95% global DAX conformance while keeping binary segment DMVs honest', () => {
    expect(DAX_CONFORMANCE_SUMMARY.coveragePercent).toBeGreaterThanOrEqual(95);
    expect(DAX_CONFORMANCE_SUMMARY.notSupportedCount).toBe(10);
  });

  it('evaluates CONTAINSROW and TREATAS cleanly', () => {
    const inTable = DaxEngine.evaluateWithContext(
      'CONTAINSROW(VALUES(f_Vendas[Categoria]), "Eletrônicos")',
      model
    );
    expect(typeof inTable).toBe('boolean');

    const treatasRes = DaxEngine.evaluateWithContext(
      'CALCULATE(SUM(f_Vendas[ValorTotal]), TREATAS(VALUES(f_Vendas[Categoria]), d_Produtos[Categoria]))',
      model
    );
    expect(typeof treatasRes).toBe('number');
  });

  it('evaluates scalar type-checking and scope/subtotal helpers', () => {
    expect(DaxEngine.evaluateWithContext('ISBOOLEAN(TRUE())', model)).toBe(true);
    expect(DaxEngine.evaluateWithContext('ISBOOLEAN(123)', model)).toBe(false);
    expect(DaxEngine.evaluateWithContext('ISDOUBLE(3.14)', model)).toBe(true);
    expect(DaxEngine.evaluateWithContext('ISINT64(42)', model)).toBe(true);
    expect(DaxEngine.evaluateWithContext('ISINT64(42.5)', model)).toBe(false);
    expect(DaxEngine.evaluateWithContext('ISNUMERIC(100)', model)).toBe(true);
    expect(DaxEngine.evaluateWithContext('ISSTRING("PrismBI")', model)).toBe(true);
    expect(DaxEngine.evaluateWithContext('ISCURRENCY(19.95)', model)).toBe(true);
    expect(typeof DaxEngine.evaluateWithContext('ISINSCOPE(d_Clientes[Segmento])', model)).toBe('boolean');
    expect(typeof DaxEngine.evaluateWithContext('ISSUBTOTAL(d_Clientes[Segmento])', model)).toBe('boolean');
    expect(typeof DaxEngine.evaluateWithContext('ISATLEVEL(d_Clientes[Segmento])', model)).toBe('boolean');
  });

  it('evaluates ALLSELECTEDAPPLY, ALLSELECTEDREMOVE, ALWAYSAPPLY and NONFILTER modifiers', () => {
    const base = Number(DaxEngine.evaluateWithContext('SUM(f_Vendas[ValorTotal])', model));
    const apply = Number(
      DaxEngine.evaluateWithContext('CALCULATE(SUM(f_Vendas[ValorTotal]), ALLSELECTEDAPPLY())', model)
    );
    const removed = Number(
      DaxEngine.evaluateWithContext('CALCULATE(SUM(f_Vendas[ValorTotal]), ALLSELECTEDREMOVE())', model)
    );
    const always = Number(
      DaxEngine.evaluateWithContext('CALCULATE(SUM(f_Vendas[ValorTotal]), ALWAYSAPPLY(f_Vendas[ValorTotal] > 0))', model)
    );
    expect(apply).toBeCloseTo(base, 2);
    expect(removed).toBeCloseTo(base, 2);
    expect(always).toBeGreaterThan(0);
    expect(DaxEngine.evaluateWithContext('NONFILTER("ok")', model)).toBe('ok');
  });

  it('evaluates newly expanded INFO.* metadata DMVs', () => {
    const dmvNames = [
      'INFO.CULTURES()',
      'INFO.CATALOGS()',
      'INFO.FORMATSTRINGDEFINITIONS()',
      'INFO.EXTENDEDPROPERTIES()',
      'INFO.GROUPBYCOLUMNS()',
      'INFO.PERSPECTIVES()',
      'INFO.PERSPECTIVETABLES()',
      'INFO.PERSPECTIVECOLUMNS()',
      'INFO.PERSPECTIVEMEASURES()',
      'INFO.PERSPECTIVEHIERARCHIES()',
      'INFO.REFRESHPOLICIES()',
      'INFO.RELATEDCOLUMNDETAILS()',
      'INFO.CALENDARS()',
      'INFO.ATTRIBUTEHIERARCHIES()',
      'INFO.CSDLMETADATA()',
      'INFO.STORAGETABLES()',
      'INFO.TABLESTORAGES()',
      'INFO.COLUMNSTORAGES()',
      'INFO.PARTITIONSTORAGES()',
      'INFO.RELATIONSHIPSTORAGES()',
      'INFO.DICTIONARYSTORAGES()',
      'INFO.HIERARCHYSTORAGES()',
      'INFO.SEGMENTSTORAGES()',
      'INFO.STORAGETABLECOLUMNS()',
    ];
    for (const dmv of dmvNames) {
      const res: any = DaxEngine.evaluateWithContext(dmv, model);
      expect(res && res.__isDaxTable).toBe(true);
    }
  });
});
