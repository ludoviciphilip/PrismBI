// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { ReplaceValuesDialog } from '../ReplaceValuesDialog';

describe('ReplaceValuesDialog', () => {
  it('Bug 2: editing an existing replaceValues step opens prefilled', () => {
    const onApply = vi.fn();
    render(
      <ReplaceValuesDialog
        column="Cidade"
        initial={{ find: 'SP', replace: 'São Paulo', matchEntireCell: false }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect((screen.getByDisplayValue('SP') as HTMLInputElement).value).toBe('SP');
    expect((screen.getByDisplayValue('São Paulo') as HTMLInputElement).value).toBe('São Paulo');
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ find: 'SP', replace: 'São Paulo', matchEntireCell: false });
  });

  it('opens empty with matchEntireCell=true by default for a new step', () => {
    const onApply = vi.fn();
    render(<ReplaceValuesDialog column="Cidade" onClose={() => {}} onApply={onApply} />);
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ find: '', replace: '', matchEntireCell: true });
  });
});
