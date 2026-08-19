use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::http_client;

/// version JSON 中 downloads.client 的解析结构(只需本课用到的字段)
#[derive(Debug, Deserialize)]
struct ClientDownload {
    sha1: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Downloads {
    client: ClientDownload,
}

/// version JSON 中 assetIndex 的解析结构(只需本课用到的字段)
#[derive(Debug, Deserialize)]
struct AssetIndexReference {
    id: String,
    sha1: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionJson {
    downloads: Downloads,
    /// L3 起使用;下载 client.jar 时不关心,故为 Option
    #[serde(default, rename = "assetIndex")]
    asset_index: Option<AssetIndexReference>,
}

/// asset index JSON 里单个对象的条目(只需本课用到的字段)
#[derive(Debug, Deserialize, Clone)]
struct AssetObject {
    hash: String,
}

#[derive(Debug, Deserialize)]
struct AssetIndex {
    objects: std::collections::HashMap<String, AssetObject>,
}

/// L3 返回给前端的下载统计
#[derive(Debug, serde::Serialize)]
pub struct AssetsSummary {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
}

/// 内容寻址的对象路径:objects/<sha1 前两位>/<完整 sha1>
/// 名字即校验值,天然防损坏、全版本共享同一仓库
fn asset_object_path(objects_dir: &Path, hash: &str) -> PathBuf {
    objects_dir.join(&hash[..2]).join(hash)
}

/// Mojang 资源 CDN 按 hash 取文件:hash 前两位当目录,完整 hash 当文件名
fn asset_download_url(hash: &str) -> String {
    format!("https://resources.download.minecraft.net/{}/{}", &hash[..2], hash)
}

/// 遍历清单:本地已存在的跳过,缺失的返回待下载列表
fn classify_objects(
    objects: &std::collections::HashMap<String, AssetObject>,
    objects_dir: &Path,
) -> (Vec<(String, AssetObject)>, usize) {
    let mut missing = Vec::new();
    let mut skipped = 0;
    for (name, obj) in objects {
        if asset_object_path(objects_dir, &obj.hash).is_file() {
            skipped += 1;
        } else {
            missing.push((name.clone(), obj.clone()));
        }
    }
    (missing, skipped)
}

/// 计算字节流的 SHA-1 十六进制指纹(四十位小写)
fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// 完整性校验:本地指纹与官方公布值比对(大小写不敏感)
fn verify_sha1(bytes: &[u8], expected: &str) -> bool {
    sha1_hex(bytes).eq_ignore_ascii_case(expected)
}

/// 游戏根目录:便携模式——放在可执行文件旁边(PCL/HMCL 便携版同思路),
/// 与当前工作目录无关,打包发布后也能正常工作
fn game_dir() -> PathBuf {
    std::env::current_exe()
        .expect("无法获取可执行文件路径")
        .parent()
        .expect("可执行文件所在目录应存在")
        .join(".bamcl-dev")
}

/// 下载某个版本的 version JSON(说明书)到 .bamcl-dev/versions/<id>/<id>.json,
/// 返回保存路径。前端通过 invoke("download_version_json", { versionId, url }) 调用。
#[tauri::command]
pub async fn download_version_json(version_id: String, url: String) -> Result<String, String> {
    // 系统边界校验:version_id 会拼成文件路径,拒绝路径分隔符与 ..
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err("非法的版本标识".into());
    }

    let client = http_client()?;
    let bytes = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载版本信息失败: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取版本信息失败: {e}"))?;

    let dir = game_dir().join("versions").join(&version_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建目录失败: {e}"))?;

    let path = dir.join(format!("{version_id}.json"));
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(path.to_string_lossy().into_owned())
}

/// 下载并校验该版本的 client.jar。
/// 前置:需先 download_version_json 下载版本信息(本地说明书提供下载地址与官方 sha1)。
/// 前端通过 invoke("download_version_jar", { versionId }) 调用。
#[tauri::command]
pub async fn download_version_jar(version_id: String) -> Result<String, String> {
    // 系统边界校验:version_id 会拼成文件路径,拒绝路径分隔符与 ..
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err("非法的版本标识".into());
    }

    // 1. 读本地说明书:拿 jar 的下载地址与官方指纹
    let info_path = game_dir()
        .join("versions")
        .join(&version_id)
        .join(format!("{version_id}.json"));
    let raw = tokio::fs::read(&info_path)
        .await
        .map_err(|_| "请先下载该版本的版本信息".to_string())?;
    let info: VersionJson =
        serde_json::from_slice(&raw).map_err(|e| format!("解析版本信息失败: {e}"))?;
    let client_dl = info.downloads.client;

    // 2. 下载 jar 字节
    let bytes = http_client()?
        .get(&client_dl.url)
        .send()
        .await
        .map_err(|e| format!("下载客户端失败: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取客户端数据失败: {e}"))?;

    // 3. sha1 完整性校验:指纹不一致说明文件损坏/被篡改,不写盘
    if !verify_sha1(&bytes, &client_dl.sha1) {
        return Err(format!(
            "校验失败: 文件可能已损坏(期望 sha1 {},实际 {})",
            client_dl.sha1,
            sha1_hex(&bytes)
        ));
    }

    // 4. 落盘
    let jar_path = game_dir()
        .join("versions")
        .join(&version_id)
        .join("client.jar");
    tokio::fs::write(&jar_path, &bytes)
        .await
        .map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(jar_path.to_string_lossy().into_owned())
}

/// 下载该版本的全部 assets(素材库:音效、语言、字体…)。
/// 流程:读本地说明书 → 取 assetIndex → 下载 index(sha1 校验,复用 verify_sha1)
///   → 解析 objects 清单 → 遍历:已存在跳过,缺失的并发 8 下载 → 每文件 sha1 校验,不一致不写盘。
/// 前置:需先 download_version_json 下载版本信息。
/// 前端通过 invoke("download_version_assets", { versionId }) 调用。
#[tauri::command]
pub async fn download_version_assets(version_id: String) -> Result<AssetsSummary, String> {
    // 系统边界校验:version_id 会拼成文件路径,拒绝路径分隔符与 ..
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err("非法的版本标识".into());
    }

    // 1. 读本地说明书:拿 assetIndex 的下载地址与官方指纹
    let info_path = game_dir()
        .join("versions")
        .join(&version_id)
        .join(format!("{version_id}.json"));
    let raw = tokio::fs::read(&info_path)
        .await
        .map_err(|_| "请先下载该版本的版本信息".to_string())?;
    let info: VersionJson =
        serde_json::from_slice(&raw).map_err(|e| format!("解析版本信息失败: {e}"))?;
    let asset_index = info
        .asset_index
        .ok_or_else(|| "版本信息中缺少 assetIndex 字段".to_string())?;

    // 2. 下载物料清单(asset index),sha1 校验通过才写盘 — 清单错了,后面全错
    let client = http_client()?;
    let index_bytes = client
        .get(&asset_index.url)
        .send()
        .await
        .map_err(|e| format!("下载资源清单失败: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取资源清单失败: {e}"))?;
    if !verify_sha1(&index_bytes, &asset_index.sha1) {
        return Err(format!(
            "资源清单校验失败(期望 sha1 {},实际 {})",
            asset_index.sha1,
            sha1_hex(&index_bytes)
        ));
    }

    let assets_dir = game_dir().join("assets");
    let indexes_dir = assets_dir.join("indexes");
    tokio::fs::create_dir_all(&indexes_dir)
        .await
        .map_err(|e| format!("创建目录失败: {e}"))?;
    tokio::fs::write(indexes_dir.join(format!("{}.json", asset_index.id)), &index_bytes)
        .await
        .map_err(|e| format!("写入资源清单失败: {e}"))?;

    // 3. 解析清单,区分"已有"和"缺失"
    let index: AssetIndex =
        serde_json::from_slice(&index_bytes).map_err(|e| format!("解析资源清单失败: {e}"))?;
    let objects_dir = assets_dir.join("objects");
    tokio::fs::create_dir_all(&objects_dir)
        .await
        .map_err(|e| format!("创建目录失败: {e}"))?;
    let (missing, skipped) = classify_objects(&index.objects, &objects_dir);
    let total = index.objects.len();

    // 4. 并发 8 下载缺失对象:每文件 sha1 校验,失败即整体报错(脏数据不落地)
    let mut downloaded = 0usize;
    for chunk in missing.chunks(8) {
        let mut handles = tokio::task::JoinSet::new();
        for (name, obj) in chunk {
            let client = client.clone();
            let objects_dir = objects_dir.clone();
            let name = name.clone();
            let hash = obj.hash.clone();
            handles.spawn(async move {
                let bytes = client
                    .get(asset_download_url(&hash))
                    .send()
                    .await
                    .map_err(|e| format!("下载资源 {name} 失败: {e}"))?
                    .bytes()
                    .await
                    .map_err(|e| format!("读取资源 {name} 失败: {e}"))?;
                if !verify_sha1(&bytes, &hash) {
                    return Err(format!(
                        "资源 {name} 校验失败(期望 sha1 {hash},实际 {})",
                        sha1_hex(&bytes)
                    ));
                }
                let path = asset_object_path(&objects_dir, &hash);
                tokio::fs::create_dir_all(
                    path.parent().ok_or_else(|| "对象路径缺少父目录".to_string())?,
                )
                .await
                .map_err(|e| format!("创建目录失败: {e}"))?;
                tokio::fs::write(&path, &bytes)
                    .await
                    .map_err(|e| format!("写入资源 {name} 失败: {e}"))?;
                Ok::<(), String>(())
            });
        }
        while let Some(result) = handles.join_next().await {
            result
                .map_err(|e| format!("下载任务失败: {e}"))?
                .map_err(|e| e)?;
            downloaded += 1;
        }
    }

    Ok(AssetsSummary {
        total,
        downloaded,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 回归测试(2026-08-18 bug):数据目录必须锚定在 exe 旁边(绝对路径),
    /// 不能依赖"当前工作目录"——tauri dev 下 cwd=src-tauri,打包后更不可控
    #[test]
    fn game_dir_is_anchored_to_executable() {
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert!(game_dir().is_absolute());
        assert!(game_dir().starts_with(exe_dir));
    }

    /// SHA1 官方测试向量:SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
    #[test]
    fn sha1_hex_matches_known_vector() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn verify_sha1_accepts_match_and_rejects_mismatch() {
        let bytes = b"abc";
        // 大小写不敏感也能匹配
        assert!(verify_sha1(bytes, "A9993E364706816ABA3E25717850C26C9CD0D89D"));
        assert!(!verify_sha1(bytes, "0000000000000000000000000000000000000000"));
    }

    /// 模拟真实 26.2.json 里 downloads.client 的形状
    #[test]
    fn parses_client_download_from_version_json() {
        let raw = json!({
            "id": "26.2",
            "downloads": {
                "client": {
                    "sha1": "2dc72797acbc1b63fc16a11c4ac393605f453754",
                    "size": 39193383,
                    "url": "https://piston-data.mojang.com/v1/objects/2dc72797acbc1b63fc16a11c4ac393605f453754/client.jar"
                },
                "server": {}
            }
        })
        .to_string();
        let parsed: VersionJson = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed.downloads.client.sha1,
            "2dc72797acbc1b63fc16a11c4ac393605f453754"
        );
        assert!(parsed.downloads.client.url.ends_with("client.jar"));
        // 没有 assetIndex 字段也能解析(缺省为 None)
        assert!(parsed.asset_index.is_none());
    }

    /// 真实 26.2.json 里 assetIndex 的形状(L3 用)
    #[test]
    fn parses_asset_index_reference_from_version_json() {
        let raw = json!({
            "id": "26.2",
            "downloads": {
                "client": {
                    "sha1": "2dc72797acbc1b63fc16a11c4ac393605f453754",
                    "url": "https://piston-data.mojang.com/v1/objects/2dc72797acbc1b63fc16a11c4ac393605f453754/client.jar"
                }
            },
            "assetIndex": {
                "id": "32",
                "sha1": "773791767c043b4f9493b50c54257619cecb08a4",
                "size": 586366,
                "totalSize": 479185985,
                "url": "https://piston-meta.mojang.com/v1/packages/773791767c043b4f9493b50c54257619cecb08a4/32.json"
            }
        })
        .to_string();
        let parsed: VersionJson = serde_json::from_str(&raw).unwrap();
        let ai = parsed.asset_index.unwrap();
        assert_eq!(ai.id, "32");
        assert_eq!(ai.sha1, "773791767c043b4f9493b50c54257619cecb08a4");
    }

    /// 内容寻址路径:objects/<前两位>/<完整hash> — 名字即校验值
    #[test]
    fn asset_object_path_uses_content_addressing() {
        let dir = PathBuf::from("/game/.bamcl-dev/assets/objects");
        let hash = "773791767c043b4f9493b50c54257619cecb08a4";
        assert_eq!(
            asset_object_path(&dir, hash),
            PathBuf::from("/game/.bamcl-dev/assets/objects/77/773791767c043b4f9493b50c54257619cecb08a4")
        );
    }

    /// 资源 CDN 按 hash 取文件:hash 前两位当目录
    #[test]
    fn asset_download_url_embeds_hash() {
        let hash = "773791767c043b4f9493b50c54257619cecb08a4";
        assert_eq!(
            asset_download_url(hash),
            "https://resources.download.minecraft.net/77/773791767c043b4f9493b50c54257619cecb08a4"
        );
    }

    /// 清单遍历:已存在的对象跳过,缺失的进入待下载列表
    #[test]
    fn classify_objects_skips_existing_files() {
        use std::collections::HashMap;
        use std::fs;

        let temp_dir = std::env::temp_dir().join("bamcl-test-assets");
        let objects_dir = temp_dir.join("objects");
        let hash_existing = "1111111111111111111111111111111111111111";
        let hash_missing = "2222222222222222222222222222222222222222";
        let existing_path = asset_object_path(&objects_dir, hash_existing);
        fs::create_dir_all(existing_path.parent().unwrap()).unwrap();
        fs::write(&existing_path, b"x").unwrap();

        let mut objects = HashMap::new();
        objects.insert(
            "icons/icon_a.png".to_string(),
            AssetObject { hash: hash_existing.to_string() },
        );
        objects.insert(
            "icons/icon_b.png".to_string(),
            AssetObject { hash: hash_missing.to_string() },
        );

        let (missing, skipped) = classify_objects(&objects, &objects_dir);
        assert_eq!(skipped, 1);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].0, "icons/icon_b.png");

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}