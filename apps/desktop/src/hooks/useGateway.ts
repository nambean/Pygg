import { useCallback, useEffect, useRef, useState } from 'react';
import { call, errorText, onGatewayChange, type GatewayApi } from '../lib/api';

export type Loaded<T> = {data: T | null; error: string | null; loading: boolean; reload: () => void};

/** The last answer for loads that name a cache key, kept for the app's lifetime. */
const cache = new Map<string, unknown>();

/**
 * Loads something from the gateway API, again whenever another screen reports a change, and optionally
 * on an interval (for live figures). Keeps showing the last data while reloading. With `cacheKey`, a screen opened
 * again starts from the last answer instead of empty, for slow loads such as asking the agent CLIs.
 */
export function useGateway<T>(load: (api: GatewayApi) => Promise<T>, deps: unknown[] = [], everyMs?: number,
  cacheKey?: string): Loaded<T> {
  const [data, setData] = useState<T | null>(() => (cacheKey ? cache.get(cacheKey) as T | undefined : undefined) ?? null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const loadRef = useRef(load);
  loadRef.current = load;
  const generation = useRef(0);
  const cacheKeyRef = useRef(cacheKey);

  const reload = useCallback(() => {
    const mine = ++generation.current;
    setLoading(true);
    call(api => loadRef.current(api)).then(
      value => {
        if (cacheKeyRef.current) cache.set(cacheKeyRef.current, value);
        if (mine === generation.current) { setData(value); setError(null); setLoading(false); }
      },
      e => { if (mine === generation.current) { setError(errorText(e)); setLoading(false); } });
  }, []);

  useEffect(() => { reload(); }, [reload, ...deps]);   // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => onGatewayChange(reload), [reload]);
  useEffect(() => {
    if (!everyMs) return;
    const timer = setInterval(() => { if (document.visibilityState === 'visible') reload(); }, everyMs);
    return () => clearInterval(timer);
  }, [everyMs, reload]);
  return {data, error, loading, reload};
}
