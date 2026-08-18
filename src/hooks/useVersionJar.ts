import { useCallback, useState } from "react";
import { downloadVersionJar } from "../lib/tauri";

/** 单版本 client.jar 下载状态(与 useVersionDownload 同款状态机) */
type JarState =
  | { status: "idle" }
  | { status: "downloading" }
  | { status: "done"; path: string }
  | { status: "error"; message: string };

export function useVersionJar(versionId: string) {
  const [state, setState] = useState<JarState>({ status: "idle" });

  const download = useCallback(async () => {
    setState({ status: "downloading" });
    try {
      const path = await downloadVersionJar(versionId);
      setState({ status: "done", path });
    } catch (err) {
      setState({ status: "error", message: String(err) });
    }
  }, [versionId]);

  return { state, download };
}