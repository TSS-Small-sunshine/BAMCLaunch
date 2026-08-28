//! L6:启动参数拼接 + 进程拉起(离线模式)。
//! 教学主线:把 L1 的说明书 + L2/L3/L4 的物料 + L5 的 Java 拼成 java 命令 + spawn。

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// L6:启动结果(返回给前端)
#[derive(Debug, Serialize)]
pub struct LaunchResult {
    /// spawn 出的 Java 进程 PID(后续用不上,前端只是显示「已启动」)
    pub pid: u32,
    /// 真实用的 java.exe 路径
    pub java_path: String,
}

/// L6:启动器识别的 OS 名(跟 Mojang 说明书 `os.name` 字符串一致)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    Windows,
    Linux,
    Osx,
}

impl OsKind {
    /// Mojang 用的字符串(`arguments.jvm[].rules[].os.name` 用这个值比对)
    pub fn as_mojang(&self) -> &'static str {
        match self {
            OsKind::Windows => "windows",
            OsKind::Linux => "linux",
            OsKind::Osx => "osx",
        }
    }

    /// 当前进程的 OS
    pub fn current() -> Self {
        if cfg!(windows) {
            OsKind::Windows
        } else if cfg!(target_os = "macos") {
            OsKind::Osx
        } else {
            OsKind::Linux
        }
    }

    /// 平台 classpath 分隔符(Windows `;`,其他 `:`)
    pub fn classpath_separator(&self) -> char {
        match self {
            OsKind::Windows => ';',
            _ => ':',
        }
    }
}

/// L6:argument 规则的 os 段(简化:只用 name + arch,26.2 实测无 arch/disallow/features)
#[derive(Debug, Deserialize)]
pub struct ArgRuleOs {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

/// L6:单条 argument 规则
#[derive(Debug, Deserialize)]
pub struct ArgRule {
    pub action: String,
    #[serde(default)]
    pub os: Option<ArgRuleOs>,
}

/// L6:argument 数组里的单项
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ArgItem {
    /// 带 rules 的项:只有当前平台/feature 满足才加入
    Conditional {
        #[serde(default)]
        rules: Vec<ArgRule>,
        value: serde_json::Value,
    },
    /// 纯字符串/字符串数组:永远加入
    Plain(serde_json::Value),
}

/// L6:规则命中判断 —— 简化版(只支持 os.name + os.arch;features 实测 26.2 不用,
/// 教学注释说明要扩展时怎么改)
fn arg_rule_applies(rule: &ArgRule, os: OsKind) -> bool {
    if let Some(os_cond) = &rule.os {
        // name 不匹配 → 不命中
        if let Some(name) = &os_cond.name {
            if name != os.as_mojang() {
                return false;
            }
        }
        // arch 不匹配 → 不命中(暂未用,占位)
        if let Some(arch) = &os_cond.arch {
            // 真实 Rust arch: "x86_64" / "aarch64" / "x86";Mojang: "x86" / "x86_64" / "arm64"
            // 26.2 实测无 arch 维度,这里只做占位匹配(x86 ↔ x86)
            let rust_arch = std::env::consts::ARCH;
            if arch != rust_arch && !(arch == "x86" && rust_arch == "x86") {
                return false;
            }
        }
        // 都匹配(或未指定)→ 命中
        true
    } else {
        // 无 os 条件 → 通用规则,命中
        true
    }
}

/// L6:扩展 `arguments.jvm[]` / `arguments.game[]` 为扁平 args 列表。
/// 规则语义:无 rules → 加入;有 rules → 最后一条匹配的规则定夺(allow → 加入)。
fn expand_arg_array(items: &[ArgItem], os: OsKind) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        match item {
            ArgItem::Plain(v) => {
                push_arg_value(&mut out, v);
            }
            ArgItem::Conditional { rules, value } => {
                if rules.is_empty() {
                    push_arg_value(&mut out, value);
                } else {
                    // 找到最后一条匹配的 rule
                    let last_match = rules
                        .iter()
                        .rev()
                        .find(|r| arg_rule_applies(r, os));
                    if let Some(r) = last_match {
                        if r.action == "allow" {
                            push_arg_value(&mut out, value);
                        }
                        // action == "disallow" → 跳过
                    }
                    // 无任何 rule 匹配 → 跳过(教学:跟 L4 libraries.rules 语义一致)
                }
            }
        }
    }
    out
}

/// L6:`value` 字段可能是单字符串或字符串数组,展开成 args
fn push_arg_value(out: &mut Vec<String>, value: &serde_json::Value) {
    if let Some(s) = value.as_str() {
        out.push(s.to_string());
    } else if let Some(arr) = value.as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                out.push(s.to_string());
            }
        }
    }
}

/// L6:替换 `${name}` 占位符为 vars[name]。未匹配的 `${...}` 保留原样(报错前调试用)。
fn expand_placeholders(text: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // 吃掉 {
            let mut name = String::new();
            let mut closed = false;
            for nc in chars.by_ref() {
                if nc == '}' {
                    closed = true;
                    break;
                }
                name.push(nc);
            }
            if closed {
                if let Some(v) = vars.get(&name) {
                    out.push_str(v);
                } else {
                    // 未匹配 → 保留 `${name}`(教学:让错误显现而非掩盖)
                    out.push_str(&format!("${{{name}}}"));
                }
            } else {
                // 没找到闭合 `}` → 原样保留
                out.push('$');
                out.push('{');
                out.push_str(&name);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// L6:从 `libraries/` 目录拼 classpath(平台分隔符 + 所有 jar + client.jar + 版本 JSON)
fn build_classpath(version_id: &str, libraries_dir: &Path) -> Result<String, String> {
    let sep = OsKind::current().classpath_separator();

    // 1) libraries 下所有 .jar(递归,虽然是平铺)
    let mut jars = Vec::new();
    collect_jars(libraries_dir, &mut jars)?;

    // 2) 拼:先 client.jar + version JSON,后 libraries
    let version_dir = super::download::game_dir().join("versions").join(version_id);
    let client_jar = version_dir.join("client.jar");
    let version_json = version_dir.join(format!("{version_id}.json"));
    if !client_jar.is_file() {
        return Err(format!("client.jar 未找到: {}", client_jar.display()));
    }
    if !version_json.is_file() {
        return Err(format!("版本说明书未找到: {}", version_json.display()));
    }

    let mut parts: Vec<String> = vec![
        client_jar.to_string_lossy().to_string(),
        version_json.to_string_lossy().to_string(),
    ];
    parts.extend(jars);
    Ok(parts.join(&sep.to_string()))
}

/// L6 助手:递归收集目录下所有 .jar 路径
fn collect_jars(dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(()); // 不存在 → 跳过(L4 还没跑就启动 → 报错交给上层)
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(()); // 读不了 → 跳过
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jars(&path, out)?; // 递归
        } else if path.extension().is_some_and(|e| e == "jar") {
            out.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

/// L6:启动顶层命令 —— 前端 [启动] 按钮调用
#[tauri::command]
pub async fn launch_version(
    version_id: String,
    java_path: String,
) -> Result<LaunchResult, String> {
    // id 安全化
    if version_id.is_empty()
        || version_id.contains('/')
        || version_id.contains('\\')
        || version_id.contains("..")
    {
        return Err("非法的版本 id".to_string());
    }

    // L7:加载设置 — 如果玩家指定了 Java 路径,优先用它(否则用调用方传的 java_path)
    let settings = super::settings::Settings::load();
    let effective_java_path = settings
        .java
        .path
        .clone()
        .unwrap_or(java_path);
    let min_mem = settings.jvm.min_memory_mb;
    let max_mem = settings.jvm.max_memory_mb;
    let effective_game_dir = settings.effective_game_dir();

    // 读说明书
    let version_dir = effective_game_dir.join("versions").join(&version_id);
    let version_json_path = version_dir.join(format!("{version_id}.json"));
    let raw = std::fs::read_to_string(&version_json_path)
        .map_err(|e| format!("读取版本说明书失败: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析版本说明书失败: {e}"))?;

    let main_class = v
        .get("mainClass")
        .and_then(|m| m.as_str())
        .ok_or_else(|| "版本说明书缺少 mainClass 字段".to_string())?
        .to_string();

    // 1) classpath
    let libraries_dir = super::download::game_dir().join("libraries");
    let classpath = build_classpath(&version_id, &libraries_dir)?;

    // 2) 占位符 vars(全 owned String,避免临时值 borrow 问题)
    let natives_dir = version_dir.join("natives");
    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let game_dir_str = effective_game_dir.to_string_lossy().to_string();
    let assets_root = effective_game_dir.join("assets");
    let assets_root_str = assets_root.to_string_lossy().to_string();
    let version_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("release").to_string();
    let assets_index_name = v.get("assets").and_then(|a| a.as_str()).unwrap_or("").to_string();
    let uuid_offline = uuid_offline_for("Player");
    vars.insert("natives_directory".into(), natives_dir.to_string_lossy().to_string());
    vars.insert("classpath".into(), classpath);
    vars.insert("version_name".into(), version_id.clone());
    vars.insert("game_directory".into(), game_dir_str);
    vars.insert("assets_root".into(), assets_root_str);
    vars.insert("assets_index_name".into(), assets_index_name);
    vars.insert("auth_player_name".into(), "Player".into());
    vars.insert("auth_uuid".into(), uuid_offline);
    vars.insert("auth_access_token".into(), String::new());
    vars.insert("auth_xuid".into(), String::new());
    vars.insert("clientid".into(), String::new());
    vars.insert("user_type".into(), "legacy".into());
    vars.insert("version_type".into(), version_type);
    vars.insert("launcher_name".into(), "BAMCLaunch".into());
    vars.insert("launcher_version".into(), "0.1.0".into());
    vars.insert("resolution_width".into(), "854".into());
    vars.insert("resolution_height".into(), "480".into());

    // 3) 拼 JVM args + game args
    let jvm_raw = v.get("arguments").and_then(|a| a.get("jvm")).and_then(|j| j.as_array());
    let game_raw = v.get("arguments").and_then(|a| a.get("game")).and_then(|g| g.as_array());
    let os = OsKind::current();

    let jvm_expanded = expand_arg_array_raw(jvm_raw, os, &vars);
    let game_expanded = expand_arg_array_raw(game_raw, os, &vars);

    // 4) L7:覆盖 JVM 内存 -Xms / -Xmx(玩家设置值,前置到所有 args 前)
    let mut jvm_with_mem = vec![
        format!("-Xms{}m", min_mem),
        format!("-Xmx{}m", max_mem),
    ];
    jvm_with_mem.extend(jvm_expanded);

    // 5) spawn
    let pid = spawn_game_process(
        Path::new(&effective_java_path),
        &jvm_with_mem,
        &main_class,
        &game_expanded,
        &effective_game_dir,
    )
    .map_err(|e| format!("spawn java 失败: {e}"))?;

    Ok(LaunchResult {
        pid,
        java_path: effective_java_path,
    })
}

/// L6 助手:从 `serde_json::Value` 数组展开 argument(绕过 ArgItem Deserialize 复杂度)
fn expand_arg_array_raw(
    arr: Option<&Vec<serde_json::Value>>,
    os: OsKind,
    vars: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    let Some(arr) = arr else { return Vec::new() };
    let mut out = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            // 带 rules 的条件项
            let rules_empty = obj.get("rules").map(|r| r.as_array().map(|a| a.is_empty()).unwrap_or(true)).unwrap_or(true);
            if !rules_empty {
                if let Some(rules_arr) = obj.get("rules").and_then(|r| r.as_array()) {
                    // 找最后一条匹配
                    let last_match = rules_arr.iter().rev().find(|r| {
                        if let Some(rule_obj) = r.as_object() {
                            if let Some(os_cond) = rule_obj.get("os").and_then(|o| o.as_object()) {
                                if let Some(name) = os_cond.get("name").and_then(|n| n.as_str()) {
                                    if name != os.as_mojang() {
                                        return false;
                                    }
                                }
                                if let Some(arch) = os_cond.get("arch").and_then(|a| a.as_str()) {
                                    let rust_arch = std::env::consts::ARCH;
                                    if arch != rust_arch && !(arch == "x86" && rust_arch == "x86") {
                                        return false;
                                    }
                                }
                            }
                            true
                        } else {
                            false
                        }
                    });
                    if let Some(r) = last_match {
                        let action = r.get("action").and_then(|a| a.as_str()).unwrap_or("");
                        if action == "allow" {
                            if let Some(v) = obj.get("value") {
                                push_expanded_args(&mut out, v, vars);
                            }
                        }
                    }
                }
            } else {
                // rules 缺失或空 → 永远加入
                if let Some(v) = obj.get("value") {
                    push_expanded_args(&mut out, v, vars);
                }
            }
        } else if let Some(s) = item.as_str() {
            out.push(expand_placeholders(s, vars));
        } else if let Some(arr_inner) = item.as_array() {
            for v in arr_inner {
                if let Some(s) = v.as_str() {
                    out.push(expand_placeholders(s, vars));
                }
            }
        }
    }
    out
}

fn push_expanded_args(
    out: &mut Vec<String>,
    value: &serde_json::Value,
    vars: &std::collections::HashMap<String, String>,
) {
    if let Some(s) = value.as_str() {
        out.push(expand_placeholders(s, vars));
    } else if let Some(arr) = value.as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                out.push(expand_placeholders(s, vars));
            }
        }
    }
}

/// L6 助手:实际 spawn java 进程
fn spawn_game_process(
    java_path: &Path,
    jvm_args: &[String],
    main_class: &str,
    game_args: &[String],
    cwd: &Path,
) -> Result<u32, String> {
    if !java_path.is_file() {
        return Err(format!("java.exe 不存在: {}", java_path.display()));
    }
    let mut cmd = Command::new(java_path);
    cmd.args(jvm_args)
        .arg(main_class)
        .args(game_args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn java 失败: {e}"))?;
    Ok(child.id().unwrap_or(0))
}

/// L6 助手:用 Java 标准算法生成离线 UUID
/// Java `UUID.nameUUIDFromBytes(bytes)` 实际是 MD5(bytes) alone —— 不加 namespace
/// (跟 RFC 4122 §4.3 不一致,但 JDK 一直这么实现,所有 Mojang 客户端都用这个 quirk)
/// 教学点:必须实测对照,不能光看文档/规范 — 2026-08-29 实测对 "OfflinePlayer:Player" 得 a01e3843-e521-3998-958a-f459800e4d11
fn uuid_offline_for(player_name: &str) -> String {
    let input = format!("OfflinePlayer:{player_name}");
    // uuid crate 的 new_v3 会加 namespace,这里手动算 MD5 alone
    let digest = md5_like_jdk_nameUUID(&input);
    format_uuid_v3(&digest)
}

/// L6 助手:对输入字节做 MD5(用 RustCrypto 的 md-5 crate)
fn md5_like_jdk_nameUUID(input: &str) -> [u8; 16] {
    use md5::{Md5, Digest};
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&result);
    out
}

/// L6 助手:把 MD5 digest 按 UUIDv3 规范转为 UUID 字符串
/// 教学:Java 名UUIDFromBytes 的最后两步 ——
///   - 清除 byte[6] 的高 4 位,置为 `0x3` (version 3)
///   - 清除 byte[8] 的高 2 位,置为 `0b10` (RFC 4122 variant)
fn format_uuid_v3(hash: &[u8; 16]) -> String {
    let mut bytes = *hash;
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10
    // UUID 字符串:`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// L6 测试 1:os.name "windows" 跟 rule.os.name "windows" 匹配
    #[test]
    fn arg_rule_applies_simple_os_match() {
        let raw = serde_json::json!({"action": "allow", "os": {"name": "windows"}});
        let rule: ArgRule = serde_json::from_value(raw).unwrap();
        assert!(arg_rule_applies(&rule, OsKind::Windows));
    }

    /// L6 测试 2:os.name "windows" 跟 rule.os.name "linux" 不匹配
    #[test]
    fn arg_rule_applies_no_match() {
        let raw = serde_json::json!({"action": "allow", "os": {"name": "linux"}});
        let rule: ArgRule = serde_json::from_value(raw).unwrap();
        assert!(!arg_rule_applies(&rule, OsKind::Windows));
    }

    /// L6 测试 3:无 os 条件的 rule → 通用规则,任何 os 都命中
    #[test]
    fn arg_rule_applies_no_os_means_universal() {
        let raw = serde_json::json!({"action": "allow"});
        let rule: ArgRule = serde_json::from_value(raw).unwrap();
        assert!(arg_rule_applies(&rule, OsKind::Windows));
        assert!(arg_rule_applies(&rule, OsKind::Linux));
        assert!(arg_rule_applies(&rule, OsKind::Osx));
    }

    /// L6 测试 4:os.name 不指定,但 arch 不匹配 → 不命中
    #[test]
    fn arg_rule_applies_arch_mismatch() {
        // 我们是 x86_64,rule 要 arm64 → 不命中
        if std::env::consts::ARCH == "x86_64" {
            let raw = serde_json::json!({"action": "allow", "os": {"arch": "arm64"}});
            let rule: ArgRule = serde_json::from_value(raw).unwrap();
            assert!(!arg_rule_applies(&rule, OsKind::Windows));
        }
    }

    /// L6 测试 5:`expand_placeholders` 基本替换
    #[test]
    fn expand_placeholders_basic() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "Steve".into());
        assert_eq!(expand_placeholders("Hello ${name}!", &vars), "Hello Steve!");
    }

    /// L6 测试 6:未匹配的占位符保留 `${...}`(教学:不要掩盖错误)
    #[test]
    fn expand_placeholders_no_match_leaves_intact() {
        let vars = HashMap::new();
        assert_eq!(
            expand_placeholders("Hello ${unknown}!", &vars),
            "Hello ${unknown}!"
        );
    }

    /// L6 测试 7:多个占位符同字符串
    #[test]
    fn expand_placeholders_multiple() {
        let mut vars = HashMap::new();
        vars.insert("a".into(), "X".into());
        vars.insert("b".into(), "Y".into());
        assert_eq!(
            expand_placeholders("${a} and ${b} together", &vars),
            "X and Y together"
        );
    }

    /// L6 测试 8:`${` 后没闭合 `}` → 原样保留
    #[test]
    fn expand_placeholders_unclosed_kept_intact() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "Steve".into());
        assert_eq!(expand_placeholders("Hello ${name", &vars), "Hello ${name");
    }

    /// L6 测试 9:占位符紧贴其他字符(不要误吞)
    #[test]
    fn expand_placeholders_adjacent_text() {
        let mut vars = HashMap::new();
        vars.insert("cp".into(), "/path/to/cp".into());
        assert_eq!(
            expand_placeholders("-cp${cp}extra", &vars),
            "-cp/path/to/cpextra"
        );
    }

    /// L6 测试 10:`expand_arg_array_raw` 真实 26.2 jvm 段(简版)
    #[test]
    fn expand_jvm_windows_filter() {
        // 26.2 jvm 段简化版:
        let arr = serde_json::json!([
            {"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
            {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=..."},
            "--sun-misc-unsafe-memory-access=allow",
            "-cp", "${classpath}"
        ]);
        let arr_v = arr.as_array().unwrap();
        let mut vars = HashMap::new();
        vars.insert("classpath".into(), "C:\\fake\\cp".into());
        let args = expand_arg_array_raw(Some(arr_v), OsKind::Windows, &vars);
        // osx 那条被过滤;windows 那条留下;两个 plain 也留下
        assert_eq!(args.len(), 4);
        assert!(args.contains(&"-XX:HeapDumpPath=...".to_string()));
        assert!(!args.iter().any(|a| a.contains("XstartOnFirstThread")));
        assert!(args.contains(&"--sun-misc-unsafe-memory-access=allow".to_string()));
        assert!(args.contains(&"C:\\fake\\cp".to_string()));
    }

    /// L6 测试 11:`expand_arg_array_raw` Linux 上 windows 项被过滤
    #[test]
    fn expand_jvm_linux_filters_windows() {
        let arr = serde_json::json!([
            {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=..."},
            {"rules": [{"action": "allow", "os": {"name": "linux"}}], "value": "-Xss1M"},
            "--common"
        ]);
        let arr_v = arr.as_array().unwrap();
        let mut vars = HashMap::new();
        let args = expand_arg_array_raw(Some(arr_v), OsKind::Linux, &vars);
        assert_eq!(args.len(), 2);
        assert!(args.contains(&"-Xss1M".to_string()));
        assert!(args.contains(&"--common".to_string()));
        assert!(!args.iter().any(|a| a.contains("HeapDumpPath")));
    }

    /// L6 测试 12:`build_classpath` 用临时 fixture 验证平台分隔符
    #[test]
    fn build_classpath_joins_with_platform_separator() {
        // 在 temp 创建 fake libraries/ 子目录,放空 .jar 文件
        let tmp = std::env::temp_dir().join(format!(
            "bamcl-test-cp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let lib_dir = tmp.join("libraries");
        std::fs::create_dir_all(&lib_dir).unwrap();
        std::fs::write(lib_dir.join("a.jar"), b"").unwrap();
        std::fs::write(lib_dir.join("b.jar"), b"").unwrap();
        std::fs::write(lib_dir.join("c.txt"), b"not a jar").unwrap(); // 应被忽略

        // 创建 versions/<id>/{client.jar, <id>.json} fixture
        // 注意 game_dir() 锚到 current_exe() parent,在测试里是 target/debug/deps/
        // 我们不依赖 game_dir,改测 collect_jars helper
        let mut jars = Vec::new();
        collect_jars(&lib_dir, &mut jars).unwrap();
        let sep = OsKind::current().classpath_separator();
        let joined = jars.join(&sep.to_string());
        let expected_sep = if cfg!(windows) { ';' } else { ':' };
        assert_eq!(sep, expected_sep);
        assert_eq!(jars.len(), 2); // 只数 .jar,.txt 跳过
        // 顺序不固定(read_dir 不保证),但两条都应在
        assert!(joined.contains("a.jar"));
        assert!(joined.contains("b.jar"));
        assert!(!joined.contains("c.txt"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// L6 测试 13:`uuid_offline_for` 跟 Java 标准 nameUUIDFromBytes 一致
    /// Java: `UUID.nameUUIDFromBytes("OfflinePlayer:Player".getBytes("UTF-8"))`
    /// 教学:这是 Mojang 客户端协议规定的算法,玩家离线登录时服务端拿这个 UUID 当身份
    /// 实测验证 2026-08-29: java 25 跑出来的 UUID 是 a01e3843-e521-3998-958a-f459800e4d11
    #[test]
    fn uuid_offline_for_known_vector() {
        let uuid = uuid_offline_for("Player");
        assert_eq!(uuid, "a01e3843-e521-3998-958a-f459800e4d11");
    }

    /// L6 测试 14:`OsKind::as_mojang` 返回正确字符串
    #[test]
    fn os_kind_as_mojang_strings() {
        assert_eq!(OsKind::Windows.as_mojang(), "windows");
        assert_eq!(OsKind::Linux.as_mojang(), "linux");
        assert_eq!(OsKind::Osx.as_mojang(), "osx");
    }

    /// L6 测试 15:`OsKind::classpath_separator` 平台正确
    #[test]
    fn os_kind_classpath_separator() {
        if cfg!(windows) {
            assert_eq!(OsKind::Windows.classpath_separator(), ';');
        } else {
            assert_eq!(OsKind::Windows.classpath_separator(), ';'); // Windows 配置总是 ;
            assert_ne!(OsKind::Linux.classpath_separator(), ';');
        }
    }
}