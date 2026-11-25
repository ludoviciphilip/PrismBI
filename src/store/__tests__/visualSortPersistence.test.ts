import { describe, it, expect } from 'vitest';
import { useBiStore } from '../useBiStore';
import { migrateProjectState } from '../../engine/migrations';

describe('visual sort persistence (.insight round-trip)', () => {
  it('sortField/sortOrder survive save -> .insight JSON -> load', () => {
    const store = useBiStore.getState();
    store.newProject('Sort Persist');
    store.addVisual('column');
    store.addVisual('table');
    store.addVisual('slicer');
    const [col, tbl, slc] = useBiStore.getState().pages[0].visuals;
    const upd = useBiStore.getState().updateVisual;
    upd(col.id, { dataConfig: { ...col.dataConfig, sortField: 'ValorTotal', sortOrder: 'asc', limit: 5 } });
    upd(tbl.id, { dataConfig: { ...tbl.dataConfig, sortField: 'Produto', sortOrder: 'desc' } });
    upd(slc.id, { dataConfig: { ...slc.dataConfig, sortOrder: 'desc' } });

    // Same path as Ribbon "Exportar .insight" + "Abrir .insight".
    const fileContent = JSON.stringify(migrateProjectState(useBiStore.getState().getProjectState()), null, 2);
    useBiStore.getState().newProject('Blank');
    expect(useBiStore.getState().pages[0].visuals).toHaveLength(0);
    useBiStore.getState().loadStateFromProject(JSON.parse(fileContent));

    const byId = (id: string) => useBiStore.getState().pages.flatMap(p => p.visuals).find(v => v.id === id)!;
    expect(byId(col.id).dataConfig).toMatchObject({ sortField: 'ValorTotal', sortOrder: 'asc', limit: 5 });
    expect(byId(tbl.id).dataConfig).toMatchObject({ sortField: 'Produto', sortOrder: 'desc' });
    expect(byId(slc.id).dataConfig.sortOrder).toBe('desc');
    expect(byId(slc.id).dataConfig.sortField).toBeUndefined();
  });
});
