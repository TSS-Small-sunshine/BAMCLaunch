import { useCallback, useState } from "react";
import { launchVersion, type LaunchResult } from "../lib/tauri";

/** L6:启动状态机(launching / launched / error) */
type LaunchState =
  | { status: "idle" }
  | { status: "launching" }
  | { status: "launched"; result: LaunchResult }
  | { status: "error"; message: string };

export function useVersionLaunch(versionId: string) {
  const [state, setState] = useState<LaunchState>({ status: "idle" });

  const launch = useCallback(
    async (javaPath: string) => {
      setState({ status: "launching" });
      try {
        const result = await launchVersion(versionId, javaPath);
        setState({ status: "launched", result });
      } catch (err) {
        setState({ status: "error", message: String(err) });
      }
    },
    [versionId],
  );

  return { state, launch };
}