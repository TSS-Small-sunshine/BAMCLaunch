import { invoke } from "@tauri-apps/api/core";
import type { VersionManifest } from "../types/version";

/** 调用 Rust 后端的 fetch_version_manifest 命令 */
export function fetchVersionManifest(): Promise<VersionManifest> {
  return invoke<VersionManifest>("fetch_version_manifest");
}

/** 把指定版本的 version JSON 下载到本地游戏目录,返回保存路径 */
export function downloadVersionJson(versionId: string, url: string): Promise<string> {
  return invoke<string>("download_version_json", { versionId, url });
}