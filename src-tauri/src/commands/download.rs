use std::path::PathBuf;

use super::http_client;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}