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
    /// L4 起使用;下载 client.jar 时不关心,故默认空
    #[serde(default)]
    libraries: Vec<Library>,
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

/// L4:libraries 数组里单个库的 rules 条目(26.2 实测只有 allow + os.name,通用解析)
#[derive(Debug, Deserialize)]
struct LibraryRuleOs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LibraryRule {
    action: String,
    #[serde(default)]
    os: Option<LibraryRuleOs>,
}

/// L4:libraries 数组里单个库的下载信息(只需本课用到的字段)
#[derive(Debug, Deserialize, Clone)]
struct ArtifactDownload {
    path: String,
    sha1: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct LibraryDownloads {
    artifact: ArtifactDownload,
}

/// L4:libraries 数组里单个库(只需本课用到的字段)
#[derive(Debug, Deserialize)]
struct Library {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<LibraryRule>,
}

/// L4 返回给前端的下载统计(natives = 解压的原生库包数)
#[derive(Debug, serde::Serialize)]
pub struct LibrariesSummary {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub natives: usize,
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

/// L4:rules 过滤——该库在当前平台是否允许下载。
/// 语义与官方启动器一致:无 rules 默认允许;有 rules 时最后一条匹配的规则定夺(allow→true)。
/// os 过滤只按名称(26.2 实测 rules 无 arch/disallow 维度)。
fn library_allowed(rules: &[LibraryRule], os_name: &str) -> bool {
    let Some(last_match) = rules
        .iter()
        .filter(|r| r.os.as_ref().is_none_or(|os| os.name == os_name))
        .last()
    else {
        return rules.is_empty();
    };
    last_match.action == "allow"
}

/// L4:native 识别——名字最后一个冒号段以 natives- 开头即原生库包
fn is_native_library(name: &str) -> bool {
    name.rsplit(':').next().is_some_and(|c| c.starts_with("natives-"))
}

/// L4:zip 条目路径的安全化——拒绝绝对路径、反斜杠、空段与 .. 逃逸,保证解压不越出 natives 目录
fn safe_entry_path(natives_dir: &Path, entry: &str) -> Option<PathBuf> {
    if entry.starts_with('/') || entry.contains('\\') {
        return None;
    }
    if entry.split('/').any(|seg| seg.is_empty() || seg == "..") {
        return None;
    }
    let path = natives_dir.join(entry);
    path.starts_with(natives_dir).then_some(path)
}

/// L4:胖 jar 裁剪——LWJGL 3.4 的 natives jar 一个包里同时装 x64/x86/arm64 三套 dll
/// (还有 META-INF 校验文件),只解本机架构那套,其余丢弃。断言规则:
///  - META-INF/ 前缀一律跳过(实测全是 .sha1/.git 元数据,无真 dll)
///  - windows/<arch>/ 目录:仅当 arch 与当前架构一致才放行
///  - 其他(平铺 jar,如 jtracy)原样保留
fn entry_allowed_for_arch(entry: &str, arch: &str) -> bool {
    if entry.starts_with("META-INF/") {
        return false;
    }
    if let Some(rest) = entry.strip_prefix("windows/") {
        let Some(entry_arch) = rest.split('/').next() else {
            return false;
        };
        return entry_arch == arch;
    }
    true
}

/// L4:把 natives jar(zip)里的文件解压到 natives 目录,返回解压出的文件数。
/// 条目逐个安全化,任何逃逸路径即整体失败(zip slip 防护);
/// 胖 jar 只解本机架构(entry_allowed_for_arch)。
/// 同步函数:zip 解压是 CPU 密集工作,调用方用 spawn_blocking 丢进阻塞线程池
/// (ZipFile 实现了非 Send 的 dyn Read,待在 async 任务里会编译期报错,这也是教学点)。
fn extract_natives(
    jar_bytes: &[u8],
    natives_dir: &Path,
    lib_name: &str,
    arch: &str,
) -> Result<usize, String> {
    use std::io::Read;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(jar_bytes)).map_err(|e| format!("打开原生库 {lib_name} 失败: {e}"))?;
    let mut count = 0;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取原生库 {lib_name} 失败: {e}"))?;
        let entry = file.name().to_string();
        if file.is_dir() || !entry_allowed_for_arch(&entry, arch) {
            continue;
        }
        let target = safe_entry_path(natives_dir, &entry)
            .ok_or_else(|| format!("原生库 {lib_name} 含非法路径: {entry}"))?;
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf)
            .map_err(|e| format!("解压 {entry} 失败: {e}"))?;
        std::fs::create_dir_all(target.parent().ok_or_else(|| "解压目标缺少父目录".to_string())?)
            .map_err(|e| format!("创建目录失败: {e}"))?;
        std::fs::write(&target, &buf).map_err(|e| format!("写入 {entry} 失败: {e}"))?;
        count += 1;
    }
    Ok(count)
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
pub(crate) fn game_dir() -> PathBuf {
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

/// Rust 架构名 → LWJGL fat-jar 目录名(x86_64→x64,aarch64→arm64,x86 不变)
fn lwjgl_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    }
}

/// 下载该版本在**当前平台**需要的全部 libraries(第三方运行库)。
/// 流程:读本地说明书 → rules 按当前 OS 过滤(windows)→ 遍历:已存在跳过,缺失的并发 8 下载
///   → 每 jar sha1 校验,不一致不写盘 → natives 条目(zip)额外解压到 versions/<id>/natives/(路径穿越防护)。
/// 前置:需先 download_version_json 下载版本信息。
/// 前端通过 invoke("download_version_libraries", { versionId }) 调用。
#[tauri::command]
pub async fn download_version_libraries(version_id: String) -> Result<LibrariesSummary, String> {
    // 系统边界校验:version_id 会拼成文件路径,拒绝路径分隔符与 ..
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err("非法的版本标识".into());
    }

    // 1. 读本地说明书:拿 libraries 清单
    let info_path = game_dir()
        .join("versions")
        .join(&version_id)
        .join(format!("{version_id}.json"));
    let raw = tokio::fs::read(&info_path)
        .await
        .map_err(|_| "请先下载该版本的版本信息".to_string())?;
    let info: VersionJson =
        serde_json::from_slice(&raw).map_err(|e| format!("解析版本信息失败: {e}"))?;

    // 2. rules 过滤:只下当前平台(Windows)需要的库
    let os_name = std::env::consts::OS;
    let required: Vec<&Library> = info
        .libraries
        .iter()
        .filter(|lib| library_allowed(&lib.rules, os_name))
        .collect();

    // 3. 遍历清单:已存在的跳过,缺失的进入待下载列表;同时收集 natives 包
    let libs_dir = game_dir().join("libraries");
    let natives_dir = game_dir().join("versions").join(&version_id).join("natives");
    let mut missing: Vec<&Library> = Vec::new();
    let mut skipped = 0usize;
    for lib in &required {
        let target = safe_entry_path(&libs_dir, &lib.downloads.artifact.path)
            .ok_or_else(|| format!("库路径非法: {}", lib.downloads.artifact.path))?;
        if target.is_file() {
            skipped += 1;
        } else {
            missing.push(lib);
        }
    }
    let total = required.len();

    // 4. 并发 8 下载缺失库:每 jar sha1 校验,失败即整体报错;natives 解压
    let client = http_client()?;
    let arch = lwjgl_arch();
    let mut downloaded = 0usize;
    let mut natives = 0usize;
    for chunk in missing.chunks(8) {
        let mut handles = tokio::task::JoinSet::new();
        for lib in chunk {
            let client = client.clone();
            let libs_dir = libs_dir.clone();
            let natives_dir = natives_dir.clone();
            let name = lib.name.clone();
            let artifact = lib.downloads.artifact.clone();
            let is_native = is_native_library(&name);
            handles.spawn(async move {
                let bytes = client
                    .get(&artifact.url)
                    .send()
                    .await
                    .map_err(|e| format!("下载库 {name} 失败: {e}"))?
                    .bytes()
                    .await
                    .map_err(|e| format!("读取库 {name} 失败: {e}"))?;
                if !verify_sha1(&bytes, &artifact.sha1) {
                    return Err(format!(
                        "库 {name} 校验失败(期望 sha1 {},实际 {})",
                        artifact.sha1,
                        sha1_hex(&bytes)
                    ));
                }
                let target = safe_entry_path(&libs_dir, &artifact.path)
                    .ok_or_else(|| format!("库路径非法: {}", artifact.path))?;
                tokio::fs::create_dir_all(
                    target.parent().ok_or_else(|| "库路径缺少父目录".to_string())?,
                )
                .await
                .map_err(|e| format!("创建目录失败: {e}"))?;
                tokio::fs::write(&target, &bytes)
                    .await
                    .map_err(|e| format!("写入库 {name} 失败: {e}"))?;
                let mut native_count = 0;
                if is_native {
                    // zip 解压是 CPU 密集的同步工作 + ZipFile 非 Send → spawn_blocking 线程池
                    let jar_bytes = bytes.clone();
                    let arch = arch.to_string();
                    native_count = tokio::task::spawn_blocking(move || {
                        extract_natives(&jar_bytes, &natives_dir, &name, &arch)
                    })
                    .await
                    .map_err(|e| format!("解压任务失败: {e}"))??;
                }
                Ok::<usize, String>(native_count)
            });
        }
        while let Some(result) = handles.join_next().await {
            let native_count = result
                .map_err(|e| format!("下载任务失败: {e}"))?
                .map_err(|e| e)?;
            downloaded += 1;
            natives += native_count;
        }
    }

    Ok(LibrariesSummary {
        total,
        downloaded,
        skipped,
        natives,
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

    /// L4:rules 过滤——没有 rules 的库默认总是要下(跨平台通用库)
    #[test]
    fn library_without_rules_is_allowed() {
        assert!(library_allowed(&[], "windows"));
    }

    /// L4:rules 过滤——os 匹配则允许,不匹配则拒绝
    #[test]
    fn rule_allow_only_matching_os() {
        let rules = vec![LibraryRule {
            action: "allow".into(),
            os: Some(LibraryRuleOs { name: "windows".into() }),
        }];
        assert!(library_allowed(&rules, "windows"));
        assert!(!library_allowed(&rules, "linux"));
    }

    /// L4:rules 语义——最后一条匹配的规则定夺(与官方启动器一致)
    #[test]
    fn last_matching_rule_decides() {
        let rules = vec![
            LibraryRule {
                action: "allow".into(),
                os: Some(LibraryRuleOs { name: "windows".into() }),
            },
            LibraryRule {
                action: "disallow".into(),
                os: Some(LibraryRuleOs { name: "windows".into() }),
            },
        ];
        assert!(!library_allowed(&rules, "windows"));
    }

    /// L4:native 识别——名字含 natives-windows 标记(26.2 实测形状)
    #[test]
    fn recognizes_native_library() {
        assert!(is_native_library("org.lwjgl:lwjgl-glfw:3.4.1:natives-windows"));
        assert!(!is_native_library("org.lwjgl:lwjgl-glfw:3.4.1"));
    }

    /// L4:zip 路径穿越防护——拒绝 .. 与绝对路径,放行正常条目
    #[test]
    fn safe_entry_path_rejects_escape() {
        let target_dir = PathBuf::from("/game/.bamcl-dev/versions/26.2/natives");
        assert_eq!(
            safe_entry_path(&target_dir, "lwjgl.dll"),
            Some(target_dir.join("lwjgl.dll"))
        );
        assert_eq!(
            safe_entry_path(&target_dir, "natives/glfw.dll"),
            Some(target_dir.join("natives/glfw.dll"))
        );
        assert!(safe_entry_path(&target_dir, "../evil.dll").is_none());
        assert!(safe_entry_path(&target_dir, "/absolute.dll").is_none());
    }

    /// L4:胖 jar 裁剪——META-INF 元数据永远跳过
    #[test]
    fn entry_filter_skips_meta_inf() {
        assert!(!entry_allowed_for_arch(
            "META-INF/windows/x64/org/lwjgl/lwjgl.dll.sha1",
            "x64"
        ));
    }

    /// L4:胖 jar 裁剪——只解本机架构的 dll,其他架构丢弃(LWJGL 3.4 fat-jar 实测 3 套并存)
    #[test]
    fn entry_filter_keeps_only_current_arch() {
        let entry = "windows/x64/org/lwjgl/lwjgl.dll";
        assert!(entry_allowed_for_arch(entry, "x64"));
        assert!(!entry_allowed_for_arch(entry, "arm64"));
        assert!(!entry_allowed_for_arch("windows/arm64/org/lwjgl/lwjgl.dll", "x64"));
    }

    /// L4:胖 jar 裁剪——平铺 jar(如 jtracy,无架构目录)原样保留
    #[test]
    fn entry_filter_keeps_flat_entries() {
        assert!(entry_allowed_for_arch("jtracy-jni-windows.dll", "x64"));
    }
}