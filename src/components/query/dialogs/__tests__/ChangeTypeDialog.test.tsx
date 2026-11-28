// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { ChangeTypeDialog } from '../ChangeTypeDialog';

describe('ChangeTypeDialog', () => {
  it('Bug 2: editing an existing changeType step opens with the saved type selected', () => {
    const onApply = vi.fn();
    render(<ChangeTypeDialog column="Ano" currentType="integer" onClose={() => {}} onApply={onApply} />);
    expect((screen.getByTestId('change-type-select') as HTMLSelectElement).value).toBe('integer');
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith('integer');
  });

  it('defaults to text for a brand-new changeType step', () => {
    const onApply = vi.fn();
    render(<ChangeTypeDialog column="Nome" onClose={() => {}} onApply={onApply} />);
    expect((screen.getByTestId('change-type-select') as HTMLSelectElement).value).toBe('text');
  });
});
