// @vitest-environment jsdom
// v0.11 Rodada 2 Frente F: repro do bug relatado pelo Luis — a grade da DAX
// Query View mostrava a coluna de medida ("T") vazia e o ano formatado como
// "2.026" (separador de milhar indevido num `dCalendario[Ano]` inteiro).
// Fixture abaixo é EXATAMENTE a forma que `dax_query_execute` (Rust) devolve
// para:
//   DEFINE MEASURE fLançamentos[Teste] = [Faturamento] * 2
//   EVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], "T", [Teste])
//   ORDER BY dCalendario[Ano] DESC
// (1 linha: Ano=2026, T=1986738.2), incluindo o `formatString` "0" que o
// TMDL do dCalendario[Ano] carrega (query_view::qexec::DaxQueryColumn).
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../../bridge/tauriBridge', async () => {
  const actual = await vi.importActual<any>('../../../bridge/tauriBridge');
  return {
    ...actual,
    isTauriRuntime: vi.fn(() => true),
    daxQueryExecuteNative: vi.fn(async () => ({
      resultSets: [
        {
          columns: [
            { name: 'dCalendario[Ano]', daxType: 'integer', formatString: '0' },
            { name: 'T', daxType: 'currency' },
          ],
          rows: [[2026, 1986738.2]],
          displayRows: [['2026', '1986738.2']],
          rowCount: 1,
          truncated: false,
        },
      ],
      elapsedMs: 0.7,
      error: null,
    })),
  };
});

import { useBiStore } from '../../../store/useBiStore';
import { DaxQueryView } from '../DaxQueryView';

describe('<DaxQueryView /> grid (Rhino bug repro)', () => {
  beforeEach(() => {
    useBiStore.setState({
      daxQueries: [
        {
          id: 'query_1',
          title: 'Consulta 1',
          query:
            'DEFINE MEASURE fLançamentos[Teste] = [Faturamento] * 2\nEVALUATE SUMMARIZECOLUMNS(dCalendario[Ano], "T", [Teste])\nORDER BY dCalendario[Ano] DESC',
        },
      ],
      activeDaxQueryTabId: 'query_1',
    } as any);
  });

  afterEach(() => cleanup());

  it('shows 2026 (no thousands separator) under dCalendario[Ano] and the T value under T, not blank', async () => {
    render(<DaxQueryView />);
    fireEvent.click(screen.getByText('Executar'));

    // Header: both columns must be present.
    await waitFor(() => expect(screen.getByText('dCalendario[Ano]')).toBeTruthy());
    expect(screen.getByText('T')).toBeTruthy();

    const row = screen.getByText('dCalendario[Ano]').closest('table')!.querySelector('tbody tr')!;
    const cells = Array.from(row.querySelectorAll('td')).map(td => td.textContent);
    // cells[0] is the row-number column ("#"); [1] is dCalendario[Ano]; [2] is T.
    expect(cells[1]).toBe('2026'); // not "2.026": the model's formatString ("0") must win over generic grouping.
    expect(cells[2]).not.toBe(''); // T must not render blank: the bug mapped/formatted it away.
    expect(cells[2]).not.toBe('(Em branco)');
    expect(cells[2]).toContain('1.986.738'); // pt-BR grouping for a currency value with no explicit model format.
  });
});
