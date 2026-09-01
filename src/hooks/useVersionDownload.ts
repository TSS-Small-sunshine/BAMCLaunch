import { useCallback, useState } from 'react';
import { downloadVersionJson } from '../lib/tauri';

/** 单版本下载状态:和 useVersionManifest 同款三态状态机 */
type DownloadState =
  | { status: 'idle' }
  | { status: 'downloading' }
  | { status: 'done'; path: string }
  | { status: 'error'; message: string };

export function useVersionDownload(versionId: string, url: string) {
  const [state, setState] = useState<DownloadState>({ status: 'idle' });

  const download = useCallback(async () => {
    setState({ status: 'downloading' });
    try {
      const path = await downloadVersionJson(versionId, url);
      setState({ status: 'done', path });
    } catch (err) {
      setState({ status: 'error', message: String(err) });
    }
  }, [versionId, url]);

  return { state, download };
}
