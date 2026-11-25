import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('../../bridge/tauriBridge', () => ({
  isTauriRuntime: vi.fn(() => false),
  credentialsSet: vi.fn(async () => {}),
  credentialsGet: vi.fn(async () => null),
}));

import {
  clearAiApiKey,
  getAiApiKey,
  saveAiSettings,
  migrateGeminiKeyToKeyring,
  saveAiApiKeySecurely,
  loadAiApiKeySecurely,
  GEMINI_KEYRING_KEY,
} from '../aiConfig';
import { isTauriRuntime, credentialsSet, credentialsGet } from '../../bridge/tauriBridge';

describe('aiConfig keyring migration (v0.8.0)', () => {
  beforeEach(() => {
    clearAiApiKey();
    vi.mocked(isTauriRuntime).mockReturnValue(false);
    vi.mocked(credentialsSet).mockClear();
    vi.mocked(credentialsGet).mockReset().mockResolvedValue(null);
  });

  it('migrateGeminiKeyToKeyring is a no-op outside Tauri', async () => {
    saveAiSettings({ apiKey: 'AIza-browser-key' });
    const migrated = await migrateGeminiKeyToKeyring();
    expect(migrated).toBe(false);
    expect(credentialsSet).not.toHaveBeenCalled();
    expect(getAiApiKey()).toBe('AIza-browser-key');
  });

  it('migrateGeminiKeyToKeyring is a no-op when there is no existing localStorage key', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(true);
    const migrated = await migrateGeminiKeyToKeyring();
    expect(migrated).toBe(false);
    expect(credentialsSet).not.toHaveBeenCalled();
  });

  it('migrateGeminiKeyToKeyring moves an existing key into the keyring and clears localStorage', async () => {
    saveAiSettings({ apiKey: 'AIza-legacy-key' });
    vi.mocked(isTauriRuntime).mockReturnValue(true);

    const migrated = await migrateGeminiKeyToKeyring();

    expect(migrated).toBe(true);
    expect(credentialsSet).toHaveBeenCalledWith(GEMINI_KEYRING_KEY, 'AIza-legacy-key');
    expect(getAiApiKey()).toBe(''); // removed from localStorage
  });

  it('saveAiApiKeySecurely writes to the keyring under Tauri and never leaves the key in localStorage', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(true);
    await saveAiApiKeySecurely('AIza-new-key');
    expect(credentialsSet).toHaveBeenCalledWith(GEMINI_KEYRING_KEY, 'AIza-new-key');
    expect(getAiApiKey()).toBe('');
  });

  it('saveAiApiKeySecurely falls back to localStorage outside Tauri', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(false);
    await saveAiApiKeySecurely('AIza-browser-only');
    expect(credentialsSet).not.toHaveBeenCalled();
    expect(getAiApiKey()).toBe('AIza-browser-only');
  });

  it('loadAiApiKeySecurely prefers the keyring value under Tauri', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(true);
    vi.mocked(credentialsGet).mockResolvedValue('AIza-from-keyring');
    saveAiSettings({ apiKey: 'AIza-stale-local-copy' });

    const key = await loadAiApiKeySecurely();
    expect(key).toBe('AIza-from-keyring');
  });

  it('loadAiApiKeySecurely falls back to localStorage when the keyring has nothing yet', async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(true);
    vi.mocked(credentialsGet).mockResolvedValue(null);
    saveAiSettings({ apiKey: 'AIza-transition-period' });

    const key = await loadAiApiKeySecurely();
    expect(key).toBe('AIza-transition-period');
  });
});
