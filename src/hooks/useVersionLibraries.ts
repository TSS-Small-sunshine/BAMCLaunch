import { useCallback, useState } from 'react';
import { downloadVersionLibraries, type LibrariesSummary } from '../lib/tauri';

/** 单版本 libraries 下载状态(与 useVersionAssets 同款状态机) */
type LibrariesState =
  | { status: 'idle' }
  | { status: 'downloading' }
  | { status: 'done'; summary: LibrariesSummary }
  | { status: 'error'; message: string };

export function useVersionLibraries(versionId: string) {
  const [state, setState] = useState<LibrariesState>({ status: 'idle' });

  const download = useCallback(async () => {
    setState({ status: 'downloading' });
    try {
      const summary = await downloadVersionLibraries(versionId);
      setState({ status: 'done', summary });
    } catch (err) {
      setState({ status: 'error', message: String(err) });
    }
  }, [versionId]);

  return { state, download };
}
