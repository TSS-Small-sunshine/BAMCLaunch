//! L5:Java 发现 —— 在本机扫描所有可用的 java.exe,返回带版本号的候选列表。
//! 教学点:多源(JAVA_HOME/PATH/常见路径/Windows 注册表)合并 + 正则解析 `java -version` 输出。

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::http_client;

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
    todo!("TDD: 实现见后续测试")
}

/// L5:从 JAVA_HOME + PATH 环境变量收集候选 java.exe 路径(全平台)。
fn discover_from_env() -> Vec<PathBuf> {
    todo!("TDD")
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
fn dedupe_candidates(candidates: Vec<JavaCandidate>) -> Vec<JavaCandidate> {
    todo!("TDD")
}

/// L5:候选 version 与 required_major 比较。
fn meets_requirement(candidate_version: u32, required_major: u32) -> bool {
    todo!("TDD")
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
}