/** 与 Rust 后端返回的 VersionManifest 对应的类型(serde camelCase 输出) */

export interface LatestVersions {
  release: string;
  snapshot: string;
}

export interface ManifestVersion {
  id: string;
  type: "release" | "snapshot";
  url: string;
  time: string;
  releaseTime: string;
  sha1?: string;
  complianceLevel?: number;
}

export interface VersionManifest {
  latest: LatestVersions;
  versions: ManifestVersion[];
}