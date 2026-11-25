// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { StepsPanel } from '../StepsPanel';
import { EtlStep } from '../../../types/etl';

const steps: EtlStep[] = [
  { id: 's1', name: 'Origem Alterada', op: 'promoteHeaders' },
  { id: 's2', name: 'Linhas Filtradas', op: 'removeBlankRows' },
  { id: 's3', name: 'Tipo Alterado', op: 'changeType', columns: [{ column: 'A', type: 'text' }] },
];

describe('StepsPanel', () => {
  it('renders every step name', () => {
    render(
      <StepsPanel
        steps={steps}
        selectedStepId="s2"
        onSelect={() => {}}
        onRename={() => {}}
        onDelete={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onEdit={() => {}}
      />
    );
    expect(screen.getByText('Origem Alterada')).toBeTruthy();
    expect(screen.getByText('Linhas Filtradas')).toBeTruthy();
    expect(screen.getByText('Tipo Alterado')).toBeTruthy();
  });

  it('shows an error badge on the failed step', () => {
    render(
      <StepsPanel
        steps={steps}
        selectedStepId="s2"
        failedStep={{ index: 2, message: 'erro' }}
        onSelect={() => {}}
        onRename={() => {}}
        onDelete={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onEdit={() => {}}
      />
    );
    expect(screen.getByTestId('step-error-s3')).toBeTruthy();
  });

  it('calls onSelect when a step row is clicked', () => {
    const onSelect = vi.fn();
    render(
      <StepsPanel
        steps={steps}
        onSelect={onSelect}
        onRename={() => {}}
        onDelete={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onEdit={() => {}}
      />
    );
    fireEvent.click(screen.getByText('Linhas Filtradas'));
    expect(onSelect).toHaveBeenCalledWith('s2');
  });

  it('commits a rename on double click + enter', () => {
    const onRename = vi.fn();
    render(
      <StepsPanel
        steps={steps}
        onSelect={() => {}}
        onRename={onRename}
        onDelete={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onEdit={() => {}}
      />
    );
    fireEvent.doubleClick(screen.getByText('Origem Alterada'));
    const input = screen.getByDisplayValue('Origem Alterada');
    fireEvent.change(input, { target: { value: 'Novo Nome' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onRename).toHaveBeenCalledWith('s1', 'Novo Nome');
  });

  it('calls onDelete for the removed step', () => {
    const onDelete = vi.fn();
    render(
      <StepsPanel
        steps={steps}
        onSelect={() => {}}
        onRename={() => {}}
        onDelete={onDelete}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onEdit={() => {}}
      />
    );
    fireEvent.click(screen.getAllByLabelText('Remover etapa')[0]);
    expect(onDelete).toHaveBeenCalledWith('s1');
  });
});
