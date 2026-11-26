import { useEffect, useMemo, useState } from 'react';

const nativeCache = new Map<string, unknown>();

/**
 * Executa uma consulta de drill/matriz: no Tauri pelo motor nativo (assíncrono,
 * com cache por chave), mostrando o resultado do fallback TS enquanto a resposta
 * nativa não chega (mesmo padrão do VisualCard); fora do Tauri, só o TS.
 */
export function useEngineQuery<T>(
  key: string,
  enabled: boolean,
  native: boolean,
  runTs: () => T,
  runNative: () => Promise<T>
): { data: T | null; error: string | null } {
  const ts = useMemo(() => {
    if (!enabled) return { data: null as T | null, error: null as string | null };
    try {
      return { data: runTs(), error: null };
    } catch (err: unknown) {
      return { data: null, error: err instanceof Error ? err.message : String(err) };
    }
  }, [enabled, key]);

  const [nativeData, setNativeData] = useState<{ key: string; data: T } | null>(() =>
    enabled && native && nativeCache.has(key) ? { key, data: nativeCache.get(key) as T } : null
  );

  useEffect(() => {
    if (!enabled || !native) return;
    if (nativeCache.has(key)) {
      setNativeData({ key, data: nativeCache.get(key) as T });
      return;
    }
    let cancelled = false;
    runNative()
      .then(data => {
        if (cancelled) return;
        if (nativeCache.size >= 250) nativeCache.clear();
        nativeCache.set(key, data);
        setNativeData({ key, data });
      })
      .catch(() => {
        // fallback silencioso para o resultado TS, como o VisualCard
      });
    return () => {
      cancelled = true;
    };
  }, [enabled, native, key]);

  if (enabled && native && nativeData && nativeData.key === key) return { data: nativeData.data, error: null };
  return ts;
}
