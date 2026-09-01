import { useCallback, useState } from 'react';
import { downloadVersionAssets, type AssetsSummary } from '../lib/tauri';

/** 单版本 assets 下载状态(与 useVersionDownload 同款状态机) */
type AssetsState =
  | { status: 'idle' }
  | { status: 'downloading' }
  | { status: 'done'; summary: AssetsSummary }
  | { status: 'error'; message: string };

export function useVersionAssets(versionId: string) {
  const [state, setState] = useState<AssetsState>({ status: 'idle' });

  const download = useCallback(async () => {
    setState({ status: 'downloading' });
    try {
      const summary = await downloadVersionAssets(versionId);
      setState({ status: 'done', summary });
    } catch (err) {
      setState({ status: 'error', message: String(err) });
    }
  }, [versionId]);

  return { state, download };
}
