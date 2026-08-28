import { useCallback, useState } from "react";
import { scanJavaInstallations, type JavaScanResult } from "../lib/tauri";

/** L5:单版本 Java 扫描状态(与 useVersionDownload 等同款状态机,但无持久结果) */
type JavaState =
  | { status: "idle" }
  | { status: "scanning" }
  | { status: "done"; result: JavaScanResult }
  | { status: "error"; message: string };

export function useVersionJava(versionId: string) {
  const [state, setState] = useState<JavaState>({ status: "idle" });

  const scan = useCallback(async () => {
    setState({ status: "scanning" });
    try {
      const result = await scanJavaInstallations(versionId);
      setState({ status: "done", result });
    } catch (err) {
      setState({ status: "error", message: String(err) });
    }
  }, [versionId]);

  const reset = useCallback(() => {
    setState({ status: "idle" });
  }, []);

  return { state, scan, reset };
}