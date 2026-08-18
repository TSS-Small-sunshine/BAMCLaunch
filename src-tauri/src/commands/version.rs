use serde::{Deserialize, Serialize};

/// Mojang 官方版本清单(version manifest v2)—— 所有启动器的版本列表都来自这里
const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

/// 清单中的单个版本条目
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    /// Mojang 的 JSON 字段名就是 `type`,用 rename 对齐后再以 camelCase 输出到前端
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub compliance_level: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<ManifestVersion>,
}

/// 拉取 Minecraft 版本清单。前端通过 invoke("fetch_version_manifest") 调用。
#[tauri::command]
pub async fn fetch_version_manifest() -> Result<VersionManifest, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("BAMCLaunch/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let manifest: VersionManifest = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("请求版本清单失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析版本清单失败: {e}"))?;

    Ok(manifest)
}