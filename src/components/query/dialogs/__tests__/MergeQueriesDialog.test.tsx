// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { MergeQueriesDialog } from '../MergeQueriesDialog';

describe('MergeQueriesDialog', () => {
  it('produces the exact mergeQueries contract payload by clicking key columns', () => {
    const onApply = vi.fn();
    render(
      <MergeQueriesDialog
        leftColumns={['ClienteId', 'Nome']}
        availableTables={[{ id: 't2', name: 'Clientes', columns: ['Id', 'Cidade'] }]}
        onClose={() => {}}
        onApply={onApply}
      />
    );

    fireEvent.click(screen.getByTestId('left-col-ClienteId'));
    fireEvent.click(screen.getByTestId('right-col-Id'));

    fireEvent.change(screen.getByDisplayValue('Externa à esquerda (todas da esquerda)'), { target: { value: 'inner' } });

    const cidadeLabel = within(screen.getByTestId('merge-expand-cols')).getByText('Cidade');
    fireEvent.click(within(cidadeLabel).getByRole('checkbox'));

    fireEvent.click(screen.getByText('Mesclar'));

    expect(onApply).toHaveBeenCalledWith({
      rightQuery: 'Clientes',
      leftKeys: ['ClienteId'],
      rightKeys: ['Id'],
      joinKind: 'inner',
      expand: [{ column: 'Cidade', as: 'Clientes.Cidade' }],
    });
  });
});
