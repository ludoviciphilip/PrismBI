// @vitest-environment jsdom
/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import { useBiStore } from '../../../store/useBiStore';
import { useGlobalShortcuts } from '../useGlobalShortcuts';

function Harness() {
  useGlobalShortcuts();
  return (
    <div>
      <input data-testid="text-input" />
      <textarea data-testid="text-area" />
    </div>
  );
}

function fireKeyDown(target: EventTarget, init: Partial<KeyboardEventInit> & { key: string }) {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}

describe('useGlobalShortcuts (v0.11)', () => {
  beforeEach(() => {
    useBiStore.getState().newProject('Shortcuts Hook Test');
  });

  afterEach(() => {
    cleanup();
  });

  it('Ctrl+S on the window calls saveProject()', async () => {
    render(<Harness />);
    const saveSpy = vi.spyOn(useBiStore.getState(), 'saveProject').mockResolvedValue();

    fireKeyDown(window, { key: 's', ctrlKey: true });
    // the action is async (await saveProject()); flush microtasks
    await Promise.resolve();
    await Promise.resolve();

    expect(saveSpy).toHaveBeenCalledTimes(1);
    saveSpy.mockRestore();
  });

  it('ignores a plain-letter canvas shortcut (Ctrl+A select-all) while focus is in a text input', () => {
    useBiStore.getState().setActiveView('report');
    const { getByTestId } = render(<Harness />);
    const input = getByTestId('text-input') as HTMLInputElement;
    input.focus();

    useBiStore.getState().addVisual('card');
    useBiStore.getState().setSelectedVisualId(null);

    fireKeyDown(input, { key: 'a', ctrlKey: true });

    // Must NOT have selected every visual on the page — editable focus blocks it.
    expect(useBiStore.getState().selectedVisualIds).toEqual([]);
  });

  it('still allows Ctrl+S while focus is in a text input (allowInEditable)', async () => {
    const { getByTestId } = render(<Harness />);
    const input = getByTestId('text-input') as HTMLInputElement;
    input.focus();
    const saveSpy = vi.spyOn(useBiStore.getState(), 'saveProject').mockResolvedValue();

    fireKeyDown(input, { key: 's', ctrlKey: true });
    await Promise.resolve();
    await Promise.resolve();

    expect(saveSpy).toHaveBeenCalledTimes(1);
    saveSpy.mockRestore();
  });

  it('scopes canvas shortcuts to the Report view only', () => {
    render(<Harness />);
    useBiStore.getState().addVisual('card');
    const visualId = useBiStore.getState().pages[0].visuals[0].id;
    useBiStore.getState().setSelectedVisualId(visualId);

    // Outside the Report view scope, Ctrl+C must not populate the clipboard.
    useBiStore.getState().setActiveView('model');
    fireKeyDown(window, { key: 'c', ctrlKey: true });
    expect(useBiStore.getState().visualClipboard).toBeNull();

    // Inside the Report view scope, it works.
    useBiStore.getState().setActiveView('report');
    fireKeyDown(window, { key: 'c', ctrlKey: true });
    expect(useBiStore.getState().visualClipboard).not.toBeNull();
    expect(useBiStore.getState().visualClipboard?.[0].id).toBe(visualId);
  });

  it('F5 always preventDefault()s, even with no handler matched for the scope', () => {
    useBiStore.getState().setActiveView('governance' as any);
    render(<Harness />);
    const event = fireKeyDown(window, { key: 'F5' });
    expect(event.defaultPrevented).toBe(true);
  });
});
