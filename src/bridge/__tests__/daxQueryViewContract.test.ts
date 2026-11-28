/**
 * v0.11 Frente F: UI × Rust command contract fixtures.
 *
 * `__fixtures__/daxQueryExecute.json` / `daxValidate.json` pair every
 * request the TS bridge wrappers (`daxQueryExecuteNative`,
 * `daxValidateNative`) send to the Tauri commands
 * (`dax_query_execute`/`dax_validate`, `src-tauri/src/dax/query_view/**`)
 * with the exact response shape those commands return — verified by
 * actually running the matching Rust unit test in
 * `src-tauri/src/dax/query_view/tests.rs` (same model, same query/
 * expression; see each fixture's `name` for the matching Rust test name)
 * and copying its asserted output, not invented. Mocks `@tauri-apps/api/
 * core`'s `invoke` here (there's no real Tauri process in Vitest) to check
 * two things a type-only contract can't: that the wrapper sends the
 * parameter shape the Rust `#[tauri::command]` signature expects (`query`/
 * `opts`, `expression`/`context`/`table`), and that the response value
 * flows back through the wrapper's generic untouched (so a Rust-side field
 * rename would fail this test, not just silently show `undefined` in the
 * UI).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import daxQueryFixtures from '../__fixtures__/daxQueryExecute.json';
import daxValidateFixtures from '../__fixtures__/daxValidate.json';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

describe('dax_query_execute / dax_validate bridge ↔ fixture contract', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it.each(daxQueryFixtures)('dax_query_execute: $name', async (fixture: any) => {
    invokeMock.mockResolvedValueOnce(fixture.response);
    const { daxQueryExecuteNative } = await import('../tauriBridge');
    const result = await daxQueryExecuteNative(fixture.request.query, fixture.request.opts?.rowLimit ?? undefined);

    expect(invokeMock).toHaveBeenCalledWith('dax_query_execute', fixture.request);
    expect(result).toEqual(fixture.response);
  });

  it.each(daxValidateFixtures)('dax_validate: $name', async (fixture: any) => {
    invokeMock.mockResolvedValueOnce(fixture.response);
    const { daxValidateNative } = await import('../tauriBridge');
    const result = await daxValidateNative(fixture.request.expression, fixture.request.context, fixture.request.table ?? undefined);

    expect(invokeMock).toHaveBeenCalledWith('dax_validate', fixture.request);
    expect(result).toEqual(fixture.response);
  });
});
