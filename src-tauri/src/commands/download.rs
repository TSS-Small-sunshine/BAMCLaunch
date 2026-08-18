use std::path::PathBuf;

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

#[derive(Debug, Deserialize)]
struct VersionJson {
    downloads: Downloads,
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
    }
}