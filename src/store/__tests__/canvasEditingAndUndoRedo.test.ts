import { describe, it, expect, beforeEach } from 'vitest';
import { useBiStore } from '../useBiStore';

describe('Canvas editing, multi-select, align/distribute & undo/redo (v0.6.0)', () => {
  beforeEach(() => {
    useBiStore.getState().newProject('Canvas Test');
  });

  it('records visual additions, moves and deletions in undo/redo history', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    store.addVisual('column');

    let state = useBiStore.getState();
    let visuals = state.pages[0].visuals;
    expect(visuals).toHaveLength(2);
    expect(state.canUndo).toBe(true);
    expect(state.canRedo).toBe(false);

    const firstId = visuals[0].id;
    const origX = visuals[0].x;
    store.updateVisual(firstId, { x: origX + 100 });
    expect(useBiStore.getState().pages[0].visuals[0].x).toBe(origX + 100);

    // Undo move
    useBiStore.getState().undo();
    expect(useBiStore.getState().pages[0].visuals[0].x).toBe(origX);
    expect(useBiStore.getState().canRedo).toBe(true);

    // Redo move
    useBiStore.getState().redo();
    expect(useBiStore.getState().pages[0].visuals[0].x).toBe(origX + 100);

    // Undo twice removes the second visual
    useBiStore.getState().undo();
    useBiStore.getState().undo();
    expect(useBiStore.getState().pages[0].visuals).toHaveLength(1);
  });

  it('supports multi-selection, group delta move and group delete', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    store.addVisual('card');
    store.addVisual('card');

    const visuals = useBiStore.getState().pages[0].visuals;
    const [v1, v2, v3] = visuals;

    useBiStore.getState().setSelectedVisualId(v1.id);
    useBiStore.getState().toggleVisualSelection(v2.id);
    expect(useBiStore.getState().selectedVisualIds).toEqual([v1.id, v2.id]);

    const v1x = v1.x;
    const v2x = v2.x;
    const v3x = v3.x;

    useBiStore.getState().moveVisualsDelta([v1.id, v2.id], 25, 10);
    const moved = useBiStore.getState().pages[0].visuals;
    expect(moved[0].x).toBe(v1x + 25);
    expect(moved[1].x).toBe(v2x + 25);
    expect(moved[2].x).toBe(v3x);

    useBiStore.getState().deleteSelectedVisuals();
    expect(useBiStore.getState().pages[0].visuals.map(v => v.id)).toEqual([v3.id]);
    expect(useBiStore.getState().selectedVisualIds).toEqual([]);

    // Undo restores both deleted visuals
    useBiStore.getState().undo();
    expect(useBiStore.getState().pages[0].visuals).toHaveLength(3);
  });

  it('aligns and distributes selected visuals accurately', () => {
    const store = useBiStore.getState();
    store.addVisual('card');
    store.addVisual('card');
    store.addVisual('card');

    const visuals = useBiStore.getState().pages[0].visuals;
    const [v1, v2, v3] = visuals;
    useBiStore.getState().updateVisual(v1.id, { x: 100, y: 50, w: 100, h: 80 });
    useBiStore.getState().updateVisual(v2.id, { x: 250, y: 120, w: 100, h: 80 });
    useBiStore.getState().updateVisual(v3.id, { x: 500, y: 200, w: 100, h: 80 });

    useBiStore.getState().setSelectedVisualIds([v1.id, v2.id, v3.id]);

    // Distribute horizontally evenly between 100 and 500+100=600 -> gap = 100 -> x: 100, 300, 500
    useBiStore.getState().alignSelectedVisuals('distribute-h');
    let aligned = useBiStore.getState().pages[0].visuals;
    expect(aligned.map(v => v.x)).toEqual([100, 300, 500]);

    // Align top -> all y = 50
    useBiStore.getState().alignSelectedVisuals('top');
    aligned = useBiStore.getState().pages[0].visuals;
    expect(aligned.map(v => v.y)).toEqual([50, 50, 50]);

    // Align left -> all x = 100
    useBiStore.getState().alignSelectedVisuals('left');
    aligned = useBiStore.getState().pages[0].visuals;
    expect(aligned.map(v => v.x)).toEqual([100, 100, 100]);
  });
});
