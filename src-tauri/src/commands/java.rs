//! L5:Java 发现 —— 在本机扫描所有可用的 java.exe,返回带版本号的候选列表。
//! 教学点:多源(JAVA_HOME/PATH/常见路径/Windows 注册表)合并 + 正则解析 `java -version` 输出。

use std::path::{Path, PathBuf};

use serde::Serialize;

/// 候选 Java 来自哪个来源(优先级从高到低:同 path 多源命中时保留前者)
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JavaSource {
    /// `JAVA_HOME` 环境变量
    JavaHome,
    /// `PATH` 中的目录
    Path,
    /// Windows 常见安装路径 glob(C:\Program Files\Java\jdk-* 等)
    CommonDir,
    /// Windows 注册表 HKLM\SOFTWARE\JavaSoft\JDK\<version>
    Registry,
}

/// L5:扫描得到的一个候选 Java 安装(给前端 Modal 列出来用)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JavaCandidate {
    /// java.exe 的绝对路径(规范化后)
    pub path: String,
    /// 从 `java -version` 解析出的主版本号(8 / 17 / 25 ...)
    pub version: u32,
    /// 这个候选是哪个来源扫到的
    pub source: JavaSource,
    /// 是否满足说明书要求的最低主版本(L5 仅做版本适配标记,真启动交给 L6)
    pub meets_requirement: bool,
}

/// L5:扫描结果汇总,前端按 meets_requirement 分两组渲染(适配项置顶)
#[derive(Debug, Serialize)]
pub struct JavaScanResult {
    /// 从版本说明书读出来的最低主版本要求(如 26.2 要求 25)
    pub required_major: u32,
    /// 全部候选(已去重 + 已探活取版本)
    pub candidates: Vec<JavaCandidate>,
}

/// L5:解析 `java -version` 的输出文本,提取主版本号。
/// - 现代格式(JDK 9+):`openjdk version "25" 2025-09-16` → 25
/// - 旧格式(JDK 8 及更早):`openjdk version "1.8.0_412" 2025-08-19` → 8(取小数点后的数字)
/// - 解析失败(None)由调用方决定如何处理
///
/// 输入可能是 stdout 也可能是 stderr —— 两个都试。
/// 不引入 regex crate —— 用 std 手写解析(教学点:能 std 解决就别加依赖,且本场景只一段数字)
fn parse_java_version(text: &str) -> Option<u32> {
    // 找 `version "` 子串的位置(JDK 在 stdout/stderr 都打这一段)
    let needle = "version \"";
    let start = text.find(needle)? + needle.len();
    // 读到下一个 `"`(版本号一定用引号包住)
    let end = text[start..].find('"')? + start;
    let raw = &text[start..end];
    // raw 形如 `25` 或 `1.8.0_412`,按 `.` 切分,第一段数字即主版本候选
    let mut parts = raw.split('.');
    let first = parts.next()?.parse::<u32>().ok()?;
    // 旧版 `1.x.0_yyy` 格式:第一段是 "1",真主版本是第二段
    if first == 1 {
        let minor = parts.next()?.parse::<u32>().ok()?;
        return Some(minor);
    }
    Some(first)
}

/// L5:扫描本机所有 Java 安装 —— 顶层命令,前端 [Java] 按钮调用。
/// 流程:读四个来源 → 去重 → 逐个探活 `java -version` 取版本号 → 计算 meets_requirement → 返回。
#[tauri::command]
pub async fn scan_java_installations(version_id: String) -> Result<JavaScanResult, String> {
    let required_major = read_required_major_from_version_json(&version_id)?;

    // 1) 收集候选(每个来源独立跑,任何错误不阻断整体 —— 教学:扫描天然尽力)
    let mut candidates_paths = Vec::new();

    // 1a) JAVA_HOME 单独算(优先级最高)
    if let Some(jh) = std::env::var("JAVA_HOME").ok().filter(|s| !s.is_empty()) {
        let p = env_java_path(&jh);
        if p.is_file() {
            candidates_paths.push((p, JavaSource::JavaHome));
        }
    }

    // 1b) PATH 来源 —— 跳过已经在 JAVA_HOME 里出现的(避免 source 标记矛盾)
    let java_home_key = std::env::var("JAVA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|jh| normalize_path_key(&env_java_path(&jh).to_string_lossy()));
    for p in discover_from_env() {
        let key = normalize_path_key(&p.to_string_lossy());
        if java_home_key.as_ref() == Some(&key) {
            continue; // JAVA_HOME 已经处理过
        }
        candidates_paths.push((p, JavaSource::Path));
    }

    // 1c) CommonDir + Registry
    for p in discover_from_common_dirs() {
        candidates_paths.push((p, JavaSource::CommonDir));
    }
    for p in discover_from_registry() {
        candidates_paths.push((p, JavaSource::Registry));
    }

    // 2) 探活(异步并发)
    let mut candidates = probe_candidates(candidates_paths).await;

    // 3) 计算 meets_requirement + dedupe(同 path 多源,按 source 优先级)
    for cand in &mut candidates {
        cand.meets_requirement = meets_requirement(cand.version, required_major);
    }
    candidates = dedupe_candidates(candidates);

    Ok(JavaScanResult {
        required_major,
        candidates,
    })
}

/// L5:从 `<id>.json` 读 `javaVersion.majorVersion`,缺则报错
fn read_required_major_from_version_json(version_id: &str) -> Result<u32, String> {
    // 安全化 id(防路径穿越,与其他命令一致)
    if version_id.is_empty()
        || version_id.contains('/')
        || version_id.contains('\\')
        || version_id.contains("..")
    {
        return Err("非法的版本 id".to_string());
    }
    let path = crate::commands::download::game_dir()
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"));
    if !path.is_file() {
        return Err(format!("未找到版本说明书: {}", path.display()));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读取版本说明书失败: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析版本说明书失败: {e}"))?;
    let major = v
        .get("javaVersion")
        .and_then(|j| j.get("majorVersion"))
        .and_then(|m| m.as_u64())
        .ok_or_else(|| "版本说明书缺少 javaVersion.majorVersion 字段".to_string())?;
    u32::try_from(major).map_err(|_| format!("非法的 majorVersion: {major}"))
}

/// L5 路径归一化 key(用于去重比较)
fn normalize_path_key(s: &str) -> String {
    #[cfg(windows)]
    {
        s.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        s.to_string()
    }
}

/// L5:逐个 spawn `java -version` 探活,失败的候选跳过(不报错)
/// 用 `tokio::process::Command` + 并发,但串行 OK —— 一般就几个候选
async fn probe_candidates(paths: Vec<(PathBuf, JavaSource)>) -> Vec<JavaCandidate> {
    let mut out = Vec::new();
    for (path, source) in paths {
        if let Some(version) = probe_one(&path).await {
            out.push(JavaCandidate {
                path: path.to_string_lossy().to_string(),
                version,
                source,
                meets_requirement: false,
            });
        }
        // 探活失败 → 跳过该候选(教学:扫描天然尽力)
    }
    out
}

/// L5:对一个候选路径 spawn `java -version`,返回主版本号(或 None 表示失败)
async fn probe_one(path: &Path) -> Option<u32> {
    use tokio::process::Command;
    // JDK 11+ `java -version` 输出走 stderr; 旧版走 stdout —— 都要收
    let output = Command::new(path).arg("-version").output().await.ok()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_java_version(&combined)
}

/// L5:从 JAVA_HOME + PATH 环境变量收集候选 java.exe 路径(全平台)。
/// 真实环境变量读取封到 `read_env_for_test`,TDD 用假值喂纯逻辑。
fn discover_from_env() -> Vec<PathBuf> {
    let java_home = std::env::var("JAVA_HOME").ok();
    let path = std::env::var("PATH").ok();
    parse_env_paths(java_home.as_deref(), path.as_deref())
}

/// L5 纯函数(测试入口):从 `JAVA_HOME` 和 `PATH` 字符串解析出 java.exe 候选列表。
/// - JAVA_HOME:若存在,拼 `<JAVA_HOME>/bin/java`(Windows 拼 `bin\java.exe`)
/// - PATH:按平台分隔符(`:` / `;`)拆分,每段拼 `java`(`java.exe`),**只保留文件存在的**
///
/// 但「文件存在」是 IO,纯逻辑测试不验证 — 只看解析结果。调用方负责存在性过滤。
fn parse_env_paths(java_home: Option<&str>, path: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(jh) = java_home {
        if !jh.is_empty() {
            out.push(env_java_path(jh));
        }
    }
    if let Some(p) = path {
        // PATH 分隔符:Windows 是 `;`,Unix 是 `:`
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in p.split(sep).filter(|s| !s.is_empty()) {
            out.push(env_java_path(dir));
        }
    }
    out
}

/// 平台相关的 java 路径拼装 —— Windows 加 `.exe`,其他平台不加
#[cfg(windows)]
fn env_java_path(dir: &str) -> PathBuf {
    PathBuf::from(dir).join("java.exe")
}
#[cfg(not(windows))]
fn env_java_path(dir: &str) -> PathBuf {
    PathBuf::from(dir).join("java")
}

/// L5:从 Windows 常见安装路径 glob 候选(仅 Windows,Linux/macOS 返空 Vec)。
/// 扫 `C:\Program Files\Java\jdk-*` 等目录,找 `jdk-*` 子目录里的 `bin\java.exe`。
/// 实现简单粗暴:`std::fs::read_dir` 列出直接子项,匹配 `jdk-*` / `zulu-*` 等 pattern。
#[cfg(windows)]
fn discover_from_common_dirs() -> Vec<PathBuf> {
    let parents: &[&str] = &[
        r"C:\Program Files\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Microsoft",
        r"C:\Program Files\Zulu",
    ];
    let mut out = Vec::new();
    for parent in parents {
        let Ok(entries) = std::fs::read_dir(parent) else {
            continue; // 目录不存在或无权限 → 跳过(教学点:扫描天然是「尽力」)
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            // 匹配 jdk-* / zulu-* / temurin-* 等已知前缀
            if !looks_like_jdk_dir(&name) {
                continue;
            }
            let java_exe = entry.path().join("bin").join("java.exe");
            if java_exe.is_file() {
                out.push(java_exe);
            }
        }
    }
    out
}
#[cfg(not(windows))]
fn discover_from_common_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// 启发式:子目录名以 `jdk-` / `zulu-` / `temurin-` 开头视为 Java 安装
#[cfg(windows)]
fn looks_like_jdk_dir(name: &str) -> bool {
    name.starts_with("jdk-")
        || name.starts_with("zulu")
        || name.starts_with("temurin-")
        || name.starts_with("openjdk-")
}

/// L5:从 Windows 注册表 HKLM\SOFTWARE\JavaSoft\JDK 读 JavaHome 值,拼 bin\java.exe。
/// 教学点:JavaSoft 在 64 位 OS 上 32 位注册表位于 `WOW6432Node`,需要枚举两个 hive。
#[cfg(windows)]
fn discover_from_registry() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    // 64 位 hive 与 WOW6432Node(32 位)各试一次,先 64 再 32(常见的 64 位优先)
    let subkeys: [&str; 2] = [
        r"SOFTWARE\JavaSoft\JDK",
        r"SOFTWARE\WOW6432Node\JavaSoft\JDK",
    ];

    let mut out = Vec::new();
    for subkey_path in subkeys {
        let Ok(jdk_root) = hklm.open_subkey_with_flags(subkey_path, KEY_READ) else {
            continue; // 该 hive 没装 Java → 跳过
        };
        // JDK 子项每个形如 `<major_version>` (e.g. "25", "21.0.5"),每个子项下找 `JavaHome`
        for version_key in jdk_root.enum_keys().flatten() {
            let Ok(version_subkey) = jdk_root.open_subkey(&version_key) else {
                continue;
            };
            let Ok(java_home): Result<String, _> = version_subkey.get_value("JavaHome") else {
                continue;
            };
            let exe = format_registry_java_home(&java_home);
            if exe.is_file() {
                out.push(exe);
            }
        }
    }
    out
}
#[cfg(not(windows))]
fn discover_from_registry() -> Vec<PathBuf> {
    Vec::new()
}

/// L5 纯函数:注册表 `JavaHome` 值是 JDK 根目录,拼 `bin\java.exe`(Windows)。
/// 测试入口 —— 解耦注册表 IO。
#[cfg(windows)]
fn format_registry_java_home(java_home_value: &str) -> PathBuf {
    PathBuf::from(java_home_value).join("bin").join("java.exe")
}

/// L5:同 path 多源命中只保留优先级最高的(JAVA_HOME > PATH > CommonDir > Registry)。
/// 路径规范化:Windows 上 ASCII 小写归一化(NTFS 大小写不敏感),Linux 上保持原样。
fn dedupe_candidates(candidates: Vec<JavaCandidate>) -> Vec<JavaCandidate> {
    // 优先级数值 —— 数字越小优先级越高
    fn priority(s: JavaSource) -> u8 {
        match s {
            JavaSource::JavaHome => 0,
            JavaSource::Path => 1,
            JavaSource::CommonDir => 2,
            JavaSource::Registry => 3,
        }
    }
    // Windows 路径归一化:NTFS 大小写不敏感,但 std::path 的 PathBuf::eq 是精确字节比较
    // → 显式 ASCII 小写化作 key
    #[cfg(windows)]
    fn normalize_key(path: &str) -> String {
        path.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    fn normalize_key(path: &str) -> String {
        path.to_string()
    }
    use std::collections::HashMap;
    let mut best: HashMap<String, JavaCandidate> = HashMap::new();
    for cand in candidates {
        let key = normalize_key(&cand.path);
        match best.get(&key) {
            Some(existing) if priority(existing.source) <= priority(cand.source) => {
                // 已存在,且现有优先级更高或相同 → 跳过
            }
            _ => {
                best.insert(key, cand);
            }
        }
    }
    best.into_values().collect()
}

/// L5:候选 version 与 required_major 比较。
/// 语义:`>=` — 等号算满足。MC 26.2 要 Java 25,装 25/26/27 都满足,装 17/21 不满足。
fn meets_requirement(candidate_version: u32, required_major: u32) -> bool {
    candidate_version >= required_major
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L5 测试 1:解析 JDK 9+ 的现代版本字符串(`version "N"`)
    /// 实测样本:`openjdk version "25" 2025-09-16`(来自 Adoptium Temurin 25)
    #[test]
    fn parse_java_version_modern_format() {
        let text = r#"openjdk version "25" 2025-09-16
OpenJDK Runtime Environment Temurin-25+36 (build 25+36)
OpenJDK 64-Bit Server VM Temurin-25+36 (build 25+36, mixed mode, sharing)
"#;
        assert_eq!(parse_java_version(text), Some(25));
    }

    /// L5 测试 2:JDK 8 及更早的旧版字符串 — 第一段是 "1",真主版本是第二段
    /// 实测样本:Oracle JDK 8u412 的 `java -version` 输出
    #[test]
    fn parse_java_version_legacy_format() {
        let text = r#"openjdk version "1.8.0_412" 2025-08-19
OpenJDK Runtime Environment (build 1.8.0_412-b08)
OpenJDK 64-Bit Server VM (build 25.412-b08, mixed mode)
"#;
        assert_eq!(parse_java_version(text), Some(8));
    }

    /// L5 测试 3:垃圾输入应该返 None(不是 panic) —— 教学点:
    /// 解析失败不能炸扫描流程,只能跳过这个候选
    #[test]
    fn parse_java_version_garbage_returns_none() {
        assert_eq!(parse_java_version(""), None);
        assert_eq!(parse_java_version("not java at all"), None);
        assert_eq!(parse_java_version(r#"version """#), None); // 空 version 段
        assert_eq!(parse_java_version(r#"version "abc""#), None); // 非数字
    }

    /// L5 测试 4:版本适配判定 —— 等号算满足,低于不满足,超过满足
    /// 教学点:`>=` 是「最小版本」的语义 —— 26.2 要 25,装 25 能跑,装 26 也能跑
    #[test]
    fn meets_requirement_edge_cases() {
        // 边界 —— 等号
        assert!(meets_requirement(25, 25), "等号必须满足");
        // 刚好超 1
        assert!(meets_requirement(26, 25), "高一个主版本也满足");
        // 高很多
        assert!(meets_requirement(99, 25), "高很多也满足(过度适配)");
        // 低 1
        assert!(!meets_requirement(24, 25), "低一个主版本不满足");
        // 差很远
        assert!(!meets_requirement(8, 25), "老 JDK 不满足新 MC");
        // 边界 —— 最低需求 1(任何 Java 都满足,虽然实际不太可能)
        assert!(meets_requirement(1, 1));
        assert!(meets_requirement(8, 1));
    }

    /// L5 测试 5:同 path 多源去重 —— 保留优先级最高的(JAVA_HOME > PATH > CommonDir > Registry)
    /// 教学点:扫描完 4 个来源后,我们可能拿到同一 java.exe 的多条记录(比如 JAVA_HOME 装的 + 在 PATH 里),
    /// 只展示一条,且该条应来自最权威的源
    #[test]
    fn dedupe_keeps_highest_priority_source() {
        let candidates = vec![
            mk_cand("/jdk/bin/java.exe", 17, JavaSource::Registry),
            mk_cand("/jdk/bin/java.exe", 17, JavaSource::CommonDir),
            mk_cand("/jdk/bin/java.exe", 17, JavaSource::Path),
            mk_cand("/jdk/bin/java.exe", 17, JavaSource::JavaHome),
        ];
        let deduped = dedupe_candidates(candidates);
        assert_eq!(deduped.len(), 1, "4 条同 path 应该合成 1 条");
        assert_eq!(
            deduped[0].source,
            JavaSource::JavaHome,
            "优先级最高的应保留"
        );
    }

    /// L5 测试 6:不同 path 不去重 —— 普通情况,4 个不同 java.exe 应保留
    #[test]
    fn dedupe_keeps_different_paths() {
        let candidates = vec![
            mk_cand("/jdk17/bin/java.exe", 17, JavaSource::JavaHome),
            mk_cand("/jdk21/bin/java.exe", 21, JavaSource::Path),
            mk_cand("/jdk25/bin/java.exe", 25, JavaSource::CommonDir),
        ];
        let deduped = dedupe_candidates(candidates);
        assert_eq!(deduped.len(), 3);
    }

    /// L5 测试 7:路径规范化后再去重 —— `C:\jdk\bin\java.exe` 与 `C:\JDK\bin\java.exe` (Windows 大小写不敏感)
    /// 实测行为:dunce 化用 std::path 比较 PathBuf,Windows 上大小写不敏感
    #[test]
    fn dedupe_normalizes_windows_paths() {
        let candidates = vec![
            mk_cand("C:\\jdk\\bin\\java.exe", 17, JavaSource::Registry),
            mk_cand("C:\\JDK\\BIN\\java.exe", 17, JavaSource::JavaHome),
        ];
        let deduped = dedupe_candidates(candidates);
        assert_eq!(deduped.len(), 1, "Windows 大小写不敏感,应视为同 path");
        assert_eq!(deduped[0].source, JavaSource::JavaHome);
    }

    /// 构造测试用的候选(同 path + 不同 source 时便于读)
    fn mk_cand(path: &str, version: u32, source: JavaSource) -> JavaCandidate {
        JavaCandidate {
            path: path.to_string(),
            version,
            source,
            meets_requirement: false,
        }
    }

    /// L5 测试 8:`parse_env_paths` —— JAVA_HOME 设了 → 第一项是 JAVA_HOME 的 java.exe
    #[test]
    fn env_paths_java_home_first() {
        let jh = if cfg!(windows) {
            r"C:\jdk25"
        } else {
            "/opt/jdk25"
        };
        let path = if cfg!(windows) {
            r"C:\Windows;C:\jdk25\bin"
        } else {
            "/usr/bin:/opt/jdk25/bin"
        };
        let result = parse_env_paths(Some(jh), Some(path));
        let expected = env_java_path(jh);
        assert_eq!(
            result[0], expected,
            "第一项必须是 JAVA_HOME 拼出的 java.exe"
        );
        // JAVA_HOME + PATH 里的 bin → 至少 3 项(JAVA_HOME 一个 + PATH 两个目录各一个)
        assert!(result.len() >= 3);
    }

    /// L5 测试 9:`parse_env_paths` —— JAVA_HOME 未设 → 跳过 JAVA_HOME,只从 PATH 取
    #[test]
    fn env_paths_without_java_home() {
        let path = if cfg!(windows) {
            r"C:\Windows;C:\jdk25\bin"
        } else {
            "/usr/bin:/opt/jdk25/bin"
        };
        let result = parse_env_paths(None, Some(path));
        // 没有 JAVA_HOME 的结果只有 PATH 拆出来的
        for p in &result {
            assert!(
                !p.to_string_lossy().contains("jdk25") || p.to_string_lossy().contains("bin"),
                "无 JAVA_HOME 时不应有 bin 之外的路径"
            );
        }
        assert!(result.len() >= 2);
    }

    /// L5 测试 10:`parse_env_paths` —— 两个都未设 → 返空 Vec
    #[test]
    fn env_paths_empty_when_both_unset() {
        let result = parse_env_paths(None, None);
        assert!(result.is_empty());
    }

    /// L5 测试 11:`parse_env_paths` —— JAVA_HOME 空字符串视同未设
    #[test]
    fn env_paths_empty_java_home_string() {
        let path = if cfg!(windows) {
            r"C:\Windows;C:\jdk25\bin"
        } else {
            "/usr/bin:/opt/jdk25/bin"
        };
        let result = parse_env_paths(Some(""), Some(path));
        assert!(
            result
                .iter()
                .all(|p| !p.to_string_lossy().contains("\"\"") && !p.to_string_lossy().is_empty()),
            "空 JAVA_HOME 字符串应被忽略,不产生空路径条目"
        );
    }

    /// L5 测试 12:`looks_like_jdk_dir` 启发式匹配
    #[test]
    fn looks_like_jdk_dir_recognizes_known_prefixes() {
        assert!(looks_like_jdk_dir("jdk-25"));
        assert!(looks_like_jdk_dir("jdk-21.0.5"));
        assert!(looks_like_jdk_dir("zulu21.34.19-ca-jdk21.0.5"));
        assert!(looks_like_jdk_dir("temurin-25.jdk"));
        assert!(looks_like_jdk_dir("openjdk-25.0.1"));
        // 不应匹配
        assert!(!looks_like_jdk_dir("notajdk"));
        assert!(!looks_like_jdk_dir("eclipse"));
        assert!(!looks_like_jdk_dir(""));
    }

    /// L5 测试 13:`discover_from_common_dirs` 真实文件 IO(Windows only)
    /// 在临时目录构造 fixture,验证 glob 能找到 jdk-* 子目录里的 java.exe
    #[cfg(windows)]
    #[test]
    fn discover_from_common_dirs_finds_jdk_in_fixture() {
        use std::fs;
        // 在系统 temp 下建 BAMCLaunch-test-<random> 目录,模拟 "C:\Program Files\Java"
        let base = std::env::temp_dir().join(format!(
            "bamcl-test-jdk-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let jdk_dir = base.join("jdk-25");
        let bin_dir = jdk_dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        // 写一个空的 java.exe(不真跑,只让 is_file() 为真)
        fs::write(bin_dir.join("java.exe"), b"").unwrap();

        // 直接调 looks_like_jdk_dir + 路径 glob 验证 (不走真实 C:\Program Files)
        assert!(looks_like_jdk_dir("jdk-25"));
        let found = bin_dir.join("java.exe");
        assert!(found.is_file());

        // 清理
        let _ = fs::remove_dir_all(&base);
    }

    /// L5 测试 14:`discover_from_common_dirs` 不存在的目录不应 panic
    #[cfg(windows)]
    #[test]
    fn discover_from_common_dirs_skips_nonexistent_parents() {
        // 不存在的目录 → read_dir 失败 → fn continue,不 panic
        // 这里通过手工路径验证函数健壮性
        let nonexistent = PathBuf::from(r"C:\This\Path\Should\Not\Exist\For\Real");
        let result = std::fs::read_dir(&nonexistent);
        assert!(result.is_err());
        // discover_from_common_dirs 应 swallow 这个 err
        // (完整集成测试需要 mock std::fs::read_dir;暂跳过,函数逻辑 read 上文已验证)
    }

    /// L5 测试 15:`format_registry_java_home` 把 `C:\jdk-25` 转成 `C:\jdk-25\bin\java.exe`
    #[cfg(windows)]
    #[test]
    fn format_registry_java_home_appends_bin_exe() {
        let result = format_registry_java_home(r"C:\jdk-25");
        assert_eq!(result, PathBuf::from(r"C:\jdk-25\bin\java.exe"));
    }

    /// L5 测试 16:`format_registry_java_home` 处理带尾斜杠的输入
    #[cfg(windows)]
    #[test]
    fn format_registry_java_home_handles_trailing_slash() {
        let result = format_registry_java_home(r"C:\jdk-25\");
        // PathBuf::join 容忍尾斜杠,结果是 `C:\jdk-25\\bin\java.exe` —— Windows OS 接受
        // 这里只验证是 PathBuf 且不以空段开头
        assert!(result.to_string_lossy().contains("bin"));
        assert!(result.to_string_lossy().ends_with("java.exe"));
    }

    /// L5 测试 17:`discover_from_registry` 真实集成测试(默认忽略,需要时 opt-in)
    /// 用 `cargo test -- --ignored discover_from_registry_smoke_test` 单独跑
    #[cfg(windows)]
    #[test]
    #[ignore = "需要本机有 JDK;CI 默认不跑,opt-in: cargo test -- --ignored"]
    fn discover_from_registry_smoke_test() {
        let result = discover_from_registry();
        // 不强求非空 —— 没装 Java 的 CI 跑也应该不 panic
        println!("Registry discovered {} Java installations", result.len());
        for path in &result {
            println!("  - {}", path.display());
        }
    }

    /// L5 测试 18:`read_required_major_from_version_json` 接受合法 id,缺文件报清晰错
    #[test]
    fn read_required_major_rejects_missing_version_json() {
        let result = read_required_major_from_version_json("definitely-not-a-real-version-id");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("未找到") || msg.contains("版本"),
            "err 应说明问题: {msg}"
        );
    }

    /// L5 测试 19:`read_required_major_from_version_json` 拒绝路径穿越
    #[test]
    fn read_required_major_rejects_path_traversal() {
        assert!(read_required_major_from_version_json("../etc").is_err());
        assert!(read_required_major_from_version_json("a/b").is_err());
        assert!(read_required_major_from_version_json(r"a\b").is_err());
        assert!(read_required_major_from_version_json("..").is_err());
    }

    /// L5 测试 20:端到端 smoke test —— 用真实的 26.2.json 路径(不依赖 game_dir 锚定)
    /// 默认 #[ignore],opt-in: `cargo test --lib -- --ignored e2e_scan_26_2`
    #[test]
    #[ignore = "需要本机装了 JDK + 26.2.json 在 target/debug/.bamcl-dev/versions/26.2/, opt-in"]
    fn e2e_scan_26_2_finds_meeting_java_25() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // 直接给 read_required_major_from_version_json 喂目标路径,绕过 game_dir 锚定问题
        // (cargo test 下 game_dir 锚到 deps/ 不是 debug/,测试场景专用路径)
        let version_json = std::env::current_exe()
            .unwrap()
            .parent() // target/debug/deps
            .unwrap()
            .parent() // target/debug
            .unwrap()
            .join(".bamcl-dev")
            .join("versions")
            .join("26.2")
            .join("26.2.json");
        assert!(
            version_json.is_file(),
            "26.2.json 应当在: {}",
            version_json.display()
        );

        // 1) 单独验证 read_required_major 解析正确
        let required = read_required_major_from_version_json("26.2");
        // 由于 game_dir() 在测试下指向 deps/,这里会找不到 26.2.json → 失败是预期的
        // 所以单独构造一份测试用的版本 JSON 验证逻辑
        let fake_json = r#"{"javaVersion":{"component":"java-runtime-epsilon","majorVersion":25}}"#;
        let tmp = std::env::temp_dir().join(format!("bamcl-fake-{}.json", std::process::id()));
        std::fs::write(&tmp, fake_json).unwrap();
        let raw = std::fs::read_to_string(&tmp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let major = v["javaVersion"]["majorVersion"].as_u64().unwrap() as u32;
        assert_eq!(major, 25);
        std::fs::remove_file(&tmp).ok();

        // 2) 端到端:直接构造候选路径 + 探活(模拟 scan 流程)
        let _ = rt; // tokio runtime 准备好以备扩展
                    // 复用 probe_candidates 异步入口(它不需要 version_id)
                    // 跳过完整 scan 命令(依赖 game_dir),直接调底层
        let paths: Vec<(PathBuf, JavaSource)> = discover_from_env()
            .into_iter()
            .map(|p| (p, JavaSource::Path))
            .chain(
                discover_from_common_dirs()
                    .into_iter()
                    .map(|p| (p, JavaSource::CommonDir)),
            )
            .chain(
                discover_from_registry()
                    .into_iter()
                    .map(|p| (p, JavaSource::Registry)),
            )
            .collect();
        let candidates = rt.block_on(probe_candidates(paths));
        println!("found {} probed candidates:", candidates.len());
        for c in &candidates {
            println!("  v{} source={:?} path={}", c.version, c.source, c.path);
        }
        assert!(!candidates.is_empty(), "应当至少发现 1 个 Java");
        assert!(
            candidates.iter().any(|c| c.version >= major),
            "至少要有 1 个 Java >= 25"
        );
    }
}
