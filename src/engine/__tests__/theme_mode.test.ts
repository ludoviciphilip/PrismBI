import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  getStoredThemeMode,
  setStoredThemeMode,
  resolveIsDark,
  applyThemeToDocument,
  initSystemThemeListener,
  getSystemTheme,
  THEME_STORAGE_KEY,
} from '../../services/themeService';

describe('themeService', () => {
  let store: Record<string, string> = {};
  let classListSet = new Set<string>();
  let colorSchemeStyle = '';
  let mediaQueryMatches = false;
  let mediaQueryListeners: ((e: any) => void)[] = [];

  beforeEach(() => {
    store = {};
    classListSet = new Set();
    colorSchemeStyle = '';
    mediaQueryMatches = false;
    mediaQueryListeners = [];

    // Mock localStorage
    const mockLocalStorage = {
      getItem: vi.fn((key: string) => store[key] ?? null),
      setItem: vi.fn((key: string, val: string) => {
        store[key] = val;
      }),
      removeItem: vi.fn((key: string) => {
        delete store[key];
      }),
      clear: vi.fn(() => {
        store = {};
      }),
    };

    // Mock document
    const mockDocument = {
      documentElement: {
        classList: {
          add: vi.fn((cls: string) => classListSet.add(cls)),
          remove: vi.fn((cls: string) => classListSet.delete(cls)),
          contains: vi.fn((cls: string) => classListSet.has(cls)),
        },
        style: {
          get colorScheme() {
            return colorSchemeStyle;
          },
          set colorScheme(val: string) {
            colorSchemeStyle = val;
          },
        },
      },
      body: {
        classList: {
          add: vi.fn((...cls: string[]) => cls.forEach(c => classListSet.add(c))),
          remove: vi.fn((...cls: string[]) => cls.forEach(c => classListSet.delete(c))),
          contains: vi.fn((cls: string) => classListSet.has(cls)),
        },
      },
    };

    // Mock window
    const mockWindow = {
      localStorage: mockLocalStorage,
      matchMedia: vi.fn((query: string) => ({
        matches: query.includes('dark') ? mediaQueryMatches : false,
        media: query,
        addEventListener: vi.fn((event: string, handler: any) => {
          if (event === 'change') mediaQueryListeners.push(handler);
        }),
        removeEventListener: vi.fn((event: string, handler: any) => {
          mediaQueryListeners = mediaQueryListeners.filter(h => h !== handler);
        }),
      })),
    };

    (globalThis as any).window = mockWindow;
    (globalThis as any).document = mockDocument;
    (globalThis as any).localStorage = mockLocalStorage;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('getStoredThemeMode', () => {
    it('returns "light" as default when no value is stored in localStorage', () => {
      expect(getStoredThemeMode()).toBe('light');
    });

    it('returns stored "light" mode', () => {
      store[THEME_STORAGE_KEY] = 'light';
      expect(getStoredThemeMode()).toBe('light');
    });

    it('returns stored "dark" mode', () => {
      store[THEME_STORAGE_KEY] = 'dark';
      expect(getStoredThemeMode()).toBe('dark');
    });

    it('returns stored "system" mode', () => {
      store[THEME_STORAGE_KEY] = 'system';
      expect(getStoredThemeMode()).toBe('system');
    });

    it('falls back to "light" when invalid value is present', () => {
      store[THEME_STORAGE_KEY] = 'invalid_mode_value';
      expect(getStoredThemeMode()).toBe('light');
    });
  });

  describe('setStoredThemeMode', () => {
    it('saves theme mode to localStorage with key "insighthub_theme_mode"', () => {
      setStoredThemeMode('dark');
      expect(store[THEME_STORAGE_KEY]).toBe('dark');

      setStoredThemeMode('light');
      expect(store[THEME_STORAGE_KEY]).toBe('light');

      setStoredThemeMode('system');
      expect(store[THEME_STORAGE_KEY]).toBe('system');
    });
  });

  describe('resolveIsDark and getSystemTheme', () => {
    it('resolves "dark" as true unconditionally', () => {
      expect(resolveIsDark('dark')).toBe(true);
    });

    it('resolves "light" as false unconditionally', () => {
      expect(resolveIsDark('light')).toBe(false);
    });

    it('resolves "system" using matchMedia', () => {
      mediaQueryMatches = true;
      expect(getSystemTheme()).toBe('dark');
      expect(resolveIsDark('system')).toBe(true);

      mediaQueryMatches = false;
      expect(getSystemTheme()).toBe('light');
      expect(resolveIsDark('system')).toBe(false);
    });
  });

  describe('applyThemeToDocument', () => {
    it('adds "dark" class and sets colorScheme="dark" when mode is "dark"', () => {
      const isDark = applyThemeToDocument('dark');
      expect(isDark).toBe(true);
      expect(classListSet.has('dark')).toBe(true);
      expect(colorSchemeStyle).toBe('dark');
    });

    it('removes "dark" class and sets colorScheme="light" when mode is "light"', () => {
      classListSet.add('dark');
      const isDark = applyThemeToDocument('light');
      expect(isDark).toBe(false);
      expect(classListSet.has('dark')).toBe(false);
      expect(colorSchemeStyle).toBe('light');
    });

    it('applies dark when system prefers dark and mode is system', () => {
      mediaQueryMatches = true;
      const isDark = applyThemeToDocument('system');
      expect(isDark).toBe(true);
      expect(classListSet.has('dark')).toBe(true);
      expect(colorSchemeStyle).toBe('dark');
    });
  });

  describe('initSystemThemeListener', () => {
    it('registers media query listener and unregisters correctly on cleanup', () => {
      const callback = vi.fn();
      const cleanup = initSystemThemeListener(callback);

      expect(mediaQueryListeners.length).toBe(1);

      // Simulate media query change
      mediaQueryListeners[0]({ matches: true });
      expect(callback).toHaveBeenCalledWith(true);

      mediaQueryListeners[0]({ matches: false });
      expect(callback).toHaveBeenCalledWith(false);

      cleanup();
      expect(mediaQueryListeners.length).toBe(0);
    });
  });
});
