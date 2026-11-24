/**
 * @license
 * The Mutualist License v1.3
 * Theme Service for InsightHub (Light / Dark / System mode)
 */

export type ThemeMode = 'light' | 'dark' | 'system';

export const THEME_STORAGE_KEY = 'insighthub_theme_mode';

/**
 * Checks system preference for dark mode.
 */
export function getSystemTheme(): 'light' | 'dark' {
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'light';
}

/**
 * Reads user's stored theme preference from localStorage.
 * Defaults to 'light' if not explicitly configured.
 */
export function getStoredThemeMode(): ThemeMode {
  if (typeof window === 'undefined') return 'light';
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      return stored;
    }
  } catch (err) {
    console.warn('Unable to access localStorage for theme:', err);
  }
  return 'light';
}

/**
 * Persists theme preference to localStorage.
 */
export function setStoredThemeMode(mode: ThemeMode): void {
  if (typeof window === 'undefined') return;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, mode);
  } catch (err) {
    console.warn('Unable to save theme mode to localStorage:', err);
  }
}

/**
 * Resolves whether dark mode is currently active given a mode.
 */
export function resolveIsDark(mode: ThemeMode): boolean {
  if (mode === 'dark') return true;
  if (mode === 'light') return false;
  return getSystemTheme() === 'dark';
}

/**
 * Applies theme classes and color-scheme to document root and body.
 */
export function applyThemeToDocument(mode: ThemeMode): boolean {
  if (typeof document === 'undefined') return false;

  const isDark = resolveIsDark(mode);
  const root = document.documentElement;
  const body = document.body;

  if (isDark) {
    root.classList.add('dark');
    if (body) {
      body.classList.add('dark');
      body.classList.remove('bg-[#f0f2f5]', 'text-slate-900');
      body.classList.add('bg-slate-950', 'text-slate-100');
    }
    root.style.colorScheme = 'dark';
  } else {
    root.classList.remove('dark');
    if (body) {
      body.classList.remove('dark', 'bg-slate-950', 'text-slate-100');
      body.classList.add('bg-[#f0f2f5]', 'text-slate-900');
    }
    root.style.colorScheme = 'light';
  }

  return isDark;
}

/**
 * Subscribes to system dark mode changes when mode is 'system'.
 */
export function initSystemThemeListener(onSystemChange: (isDark: boolean) => void): () => void {
  if (typeof window === 'undefined' || !window.matchMedia) {
    return () => {};
  }

  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  const listener = (e: MediaQueryListEvent) => {
    onSystemChange(e.matches);
  };

  if (mediaQuery.addEventListener) {
    mediaQuery.addEventListener('change', listener);
    return () => mediaQuery.removeEventListener('change', listener);
  } else if ('addListener' in mediaQuery) {
    // Legacy support
    (mediaQuery as any).addListener(listener);
    return () => (mediaQuery as any).removeListener(listener);
  }

  return () => {};
}
