import { useCallback, useEffect, useState } from 'react';
import type { VersionManifest } from '../types/version';
import { fetchVersionManifest } from '../lib/tauri';

/** 请求三态:loading / success / error */
type HookState =
  | { status: 'loading' }
  | { status: 'success'; manifest: VersionManifest }
  | { status: 'error'; message: string };

export function useVersionManifest() {
  const [state, setState] = useState<HookState>({ status: 'loading' });

  const reload = useCallback(async () => {
    setState({ status: 'loading' });
    try {
      const manifest = await fetchVersionManifest();
      setState({ status: 'success', manifest });
    } catch (err) {
      setState({ status: 'error', message: String(err) });
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return { ...state, reload };
}
