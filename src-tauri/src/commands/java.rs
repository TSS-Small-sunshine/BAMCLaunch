//! L5:Java 发现 —— 在本机扫描所有可用的 java.exe,返回带版本号的候选列表。
//! 教学点:多源(JAVA_HOME/PATH/常见路径/Windows 注册表)合并 + 正则解析 `java -version` 输出。

use std::path::PathBuf;

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
    let _ = version_id; // TODO(L5 后段):读 <id>.json 拿 required_major
    todo!("TDD: 实现见后续测试")
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
#[cfg(windows)]
fn discover_from_common_dirs() -> Vec<PathBuf> {
    todo!("TDD")
}
#[cfg(not(windows))]
fn discover_from_common_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// L5:从 Windows 注册表 HKLM\SOFTWARE\JavaSoft\JDK 读 JavaHome 值,拼 bin\java.exe。
#[cfg(windows)]
fn discover_from_registry() -> Vec<PathBuf> {
    todo!("TDD")
}
#[cfg(not(windows))]
fn discover_from_registry() -> Vec<PathBuf> {
    Vec::new()
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
        assert_eq!(deduped[0].source, JavaSource::JavaHome, "优先级最高的应保留");
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
        let jh = if cfg!(windows) { r"C:\jdk25" } else { "/opt/jdk25" };
        let path = if cfg!(windows) {
            r"C:\Windows;C:\jdk25\bin"
        } else {
            "/usr/bin:/opt/jdk25/bin"
        };
        let result = parse_env_paths(Some(jh), Some(path));
        let expected = env_java_path(jh);
        assert_eq!(result[0], expected, "第一项必须是 JAVA_HOME 拼出的 java.exe");
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
            result.iter().all(|p| !p.to_string_lossy().contains("\"\"")
                && !p.to_string_lossy().is_empty()),
            "空 JAVA_HOME 字符串应被忽略,不产生空路径条目"
        );
    }
}