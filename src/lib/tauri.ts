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

/** 下载并校验该版本的 client.jar,返回保存路径 */
export function downloadVersionJar(versionId: string): Promise<string> {
  return invoke<string>("download_version_jar", { versionId });
}

/** 下载该版本全部 assets(素材库),返回下载统计 */
export function downloadVersionAssets(versionId: string): Promise<AssetsSummary> {
  return invoke<AssetsSummary>("download_version_assets", { versionId });
}

/** download_version_assets 命令的返回统计 */
export interface AssetsSummary {
  total: number;
  downloaded: number;
  skipped: number;
}