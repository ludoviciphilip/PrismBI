/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore } from '../useBiStore';

describe('Visual clipboard: copy/paste/duplicate (v0.11, atalhos de teclado)', () => {
  beforeEach(() => {
    useBiStore.getState().newProject('Visual Clipboard Test');
  });

  it('copySelectedVisuals stores a snapshot of the selected visual in the clipboard', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    const visual = useBiStore.getState().pages[0].visuals[0];
    useBiStore.getState().setSelectedVisualId(visual.id);

    useBiStore.getState().copySelectedVisuals();
    const clip = useBiStore.getState().visualClipboard;
    expect(clip).toHaveLength(1);
    expect(clip![0].id).toBe(visual.id);
    expect(clip![0]).not.toBe(visual); // deep copy, not the same reference
  });

  it('copySelectedVisuals is a no-op without a selection', () => {
    useBiStore.getState().copySelectedVisuals();
    expect(useBiStore.getState().visualClipboard).toBeNull();
  });

  it('pasteVisuals adds a new visual offset from the copied one, selecting it', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    const visual = useBiStore.getState().pages[0].visuals[0];
    useBiStore.getState().setSelectedVisualId(visual.id);
    useBiStore.getState().copySelectedVisuals();

    useBiStore.getState().pasteVisuals();

    const visuals = useBiStore.getState().pages[0].visuals;
    expect(visuals).toHaveLength(2);
    const pasted = visuals[1];
    expect(pasted.id).not.toBe(visual.id);
    expect(pasted.x).toBe(visual.x + 24);
    expect(pasted.y).toBe(visual.y + 24);
    expect(useBiStore.getState().selectedVisualIds).toEqual([pasted.id]);
  });

  it('pasteVisuals is a no-op with an empty clipboard', () => {
    useBiStore.getState().addVisual('card');
    const before = useBiStore.getState().pages[0].visuals.length;
    useBiStore.getState().pasteVisuals();
    expect(useBiStore.getState().pages[0].visuals.length).toBe(before);
  });

  it('pasting twice keeps the original clipboard snapshot (independent copies)', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    const visual = useBiStore.getState().pages[0].visuals[0];
    useBiStore.getState().setSelectedVisualId(visual.id);
    useBiStore.getState().copySelectedVisuals();

    useBiStore.getState().pasteVisuals();
    useBiStore.getState().pasteVisuals();

    const visuals = useBiStore.getState().pages[0].visuals;
    expect(visuals).toHaveLength(3);
    const ids = new Set(visuals.map(v => v.id));
    expect(ids.size).toBe(3);
  });

  it('duplicateSelectedVisuals clones the selected visual directly (no clipboard needed)', () => {
    const store = useBiStore.getState();
    store.addVisual('column');
    const visual = useBiStore.getState().pages[0].visuals[0];
    useBiStore.getState().setSelectedVisualId(visual.id);

    useBiStore.getState().duplicateSelectedVisuals();

    const visuals = useBiStore.getState().pages[0].visuals;
    expect(visuals).toHaveLength(2);
    const dup = visuals[1];
    expect(dup.id).not.toBe(visual.id);
    expect(dup.type).toBe(visual.type);
    expect(dup.x).toBe(visual.x + 24);
    expect(useBiStore.getState().selectedVisualIds).toEqual([dup.id]);
  });

  it('duplicateSelectedVisuals supports multi-selection', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    store.addVisual('card');
    const [v1, v2] = useBiStore.getState().pages[0].visuals;
    useBiStore.getState().setSelectedVisualIds([v1.id, v2.id]);

    useBiStore.getState().duplicateSelectedVisuals();

    expect(useBiStore.getState().pages[0].visuals).toHaveLength(4);
  });

  it('selectAllVisualsOnPage selects every non-hidden visual on the active page', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    store.addVisual('card');
    store.addVisual('card');
    const visuals = useBiStore.getState().pages[0].visuals;
    useBiStore.getState().updateVisual(visuals[2].id, { formatConfig: { ...visuals[2].formatConfig, hidden: true } });

    useBiStore.getState().setSelectedVisualId(null);
    useBiStore.getState().selectAllVisualsOnPage();

    const selected = useBiStore.getState().selectedVisualIds;
    expect(selected).toHaveLength(2);
    expect(selected).toContain(visuals[0].id);
    expect(selected).toContain(visuals[1].id);
    expect(selected).not.toContain(visuals[2].id);
  });

  it('copy/paste/duplicate are undoable', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    const visual = useBiStore.getState().pages[0].visuals[0];
    useBiStore.getState().setSelectedVisualId(visual.id);

    useBiStore.getState().duplicateSelectedVisuals();
    expect(useBiStore.getState().pages[0].visuals).toHaveLength(2);

    useBiStore.getState().undo();
    expect(useBiStore.getState().pages[0].visuals).toHaveLength(1);
  });
});
