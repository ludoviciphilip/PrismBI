// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import React from 'react';
import { SplitColumnDialog } from '../SplitColumnDialog';

describe('SplitColumnDialog', () => {
  it('Bug 2: editing an existing splitColumn step opens prefilled', () => {
    const onApply = vi.fn();
    render(
      <SplitColumnDialog
        column="Cod"
        initial={{ mode: 'delimiterEach', delimiter: '-', into: ['Cod.1', 'Cod.2'] }}
        onClose={() => {}}
        onApply={onApply}
      />,
    );
    expect((screen.getByDisplayValue('-') as HTMLInputElement).value).toBe('-');
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ mode: 'delimiterEach', delimiter: '-', into: ['Cod.1', 'Cod.2'] });
  });

  it('Bug 2: prefills fixed-length mode with its length', () => {
    const onApply = vi.fn();
    render(
      <SplitColumnDialog column="Cod" initial={{ mode: 'fixedLength', length: 3, into: ['Cod.1', 'Cod.2'] }} onClose={() => {}} onApply={onApply} />,
    );
    expect((screen.getByDisplayValue('3') as HTMLInputElement).value).toBe('3');
    fireEvent.click(screen.getByText('OK'));
    expect(onApply).toHaveBeenCalledWith({ mode: 'fixedLength', length: 3, into: ['Cod.1', 'Cod.2'] });
  });
});
