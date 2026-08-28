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

/** 下载当前平台需要的全部 libraries(运行库),返回下载统计 */
export function downloadVersionLibraries(versionId: string): Promise<LibrariesSummary> {
  return invoke<LibrariesSummary>("download_version_libraries", { versionId });
}

/** download_version_assets 命令的返回统计 */
export interface AssetsSummary {
  total: number;
  downloaded: number;
  skipped: number;
}

/** download_version_libraries 命令的返回统计 */
export interface LibrariesSummary {
  total: number;
  downloaded: number;
  skipped: number;
  natives: number;
}

/** L5:扫描本机 Java 安装 —— 候选来源(优先级从高到低) */
export type JavaSource = "java_home" | "path" | "common_dir" | "registry";

/** L5:扫描得到的一个候选 Java 安装 */
export interface JavaCandidate {
  /** java.exe 绝对路径 */
  path: string;
  /** 从 `java -version` 解析出的主版本号 */
  version: number;
  /** 这个候选是哪个来源扫到的 */
  source: JavaSource;
  /** 是否满足版本说明书要求的最低主版本 */
  meets_requirement: boolean;
}

/** L5:扫描结果汇总,前端按 meets_requirement 分组渲染 */
export interface JavaScanResult {
  /** 从版本说明书读出来的最低主版本要求(如 26.2 要求 25) */
  required_major: number;
  /** 全部候选(已去重 + 已探活取版本) */
  candidates: JavaCandidate[];
}

/** L5:扫描本机 Java 安装并取真实版本号(调 scan_java_installations 命令) */
export function scanJavaInstallations(versionId: string): Promise<JavaScanResult> {
  return invoke<JavaScanResult>("scan_java_installations", { versionId });
}