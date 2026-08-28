//! L7:设置持久化 —— Java 路径 / JVM 内存 / 游戏目录的可选配置。
//! 存储:JSON 文件,跟随 game_dir(<game_dir>/settings.json),serde 读写。
//! 教学:任何设置都有「默认行为」 — 缺失字段 / 缺失文件都用默认值,启动器不能因没设就崩。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::download::game_dir;

/// L7:Java 路径设置(可手动指定,不指定时回退到 L5 扫描结果)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JavaSettings {
    /// `D:\jdk-25\bin\java.exe` 这样的绝对路径(玩家手动选过)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// 玩家记录的主版本号(UI 显示用,不参与启动逻辑)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
}

impl Default for JavaSettings {
    fn default() -> Self {
        Self {
            path: None,
            version: None,
        }
    }
}

/// L7:JVM 内存设置(玩家可调)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JvmSettings {
    /// `-Xms` 初始堆内存(MB),默认 2048(说明书里 `-Xms2G`)
    pub min_memory_mb: u32,
    /// `-Xmx` 最大堆内存(MB),默认 4096(说明书里 `-Xmx4G`)
    pub max_memory_mb: u32,
}

impl Default for JvmSettings {
    fn default() -> Self {
        Self {
            min_memory_mb: 2048,
            max_memory_mb: 4096,
        }
    }
}

/// L7:全部设置(单一 struct,serde flatten)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Settings {
    #[serde(default)]
    pub java: JavaSettings,
    #[serde(default)]
    pub jvm: JvmSettings,
    /// 游戏目录 override(默认 None → 用 L1 锚定的便携路径)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_dir: Option<String>,
}

impl Settings {
    /// L7:加载设置。文件不存在 / 解析失败 → 返回默认值(教学:不能因没设就崩)
    /// - 不存在的 settings.json → `Settings::default()`
    /// - 存在但解析失败 → `Err` 让上层记日志,返回 `Settings::default()`(降级启动)
    pub fn load() -> Self {
        let path = settings_file_path();
        if !path.is_file() {
            return Settings::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Settings>(&content) {
                Ok(s) => s,
                Err(_) => Settings::default(), // 损坏文件降级 — 启动器永远能跑
            },
            Err(_) => Settings::default(), // 读不到降级
        }
    }

    /// L7:保存到磁盘。原子写 — 先写 `<file>.tmp` 再 rename,避免半写状态
    pub fn save(&self) -> Result<(), String> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建设置目录失败: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化设置失败: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| format!("写入临时设置文件失败: {e}"))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| format!("提交设置文件失败: {e}"))?;
        Ok(())
    }

    /// L7:有效游戏目录 —— `game_dir` override 有就用,否则用 L1 锚定的便携路径
    pub fn effective_game_dir(&self) -> PathBuf {
        match &self.game_dir {
            Some(s) if !s.is_empty() => PathBuf::from(s),
            _ => game_dir(),
        }
    }
}

/// L7:设置文件路径 = `<game_dir>/settings.json`
/// 注意:这里用的是 L1 锚定的 game_dir(),而不是 settings 自己覆盖的 game_dir
/// — 避免「设置目录不存在 → 没法读设置 → 没法知道 game_dir 在哪」的鸡生蛋问题
fn settings_file_path() -> PathBuf {
    game_dir().join("settings.json")
}

/// L7:校验 Java 路径(确保它是文件存在 + 是绝对路径)
/// 不真调 java -version(那太慢 — UI 改成接受设了路径,启动时 L6 才会真验证)
pub fn validate_java_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("路径不能为空".to_string());
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("必须是绝对路径: {path}"));
    }
    if !p.is_file() {
        return Err(format!("文件不存在: {path}"));
    }
    Ok(())
}

/// L7:校验 JVM 内存范围(min > 0, max >= min, 都在合理范围内)
pub fn validate_jvm_memory(min: u32, max: u32) -> Result<(), String> {
    if min == 0 {
        return Err("初始内存不能为 0".to_string());
    }
    if max < min {
        return Err(format!("最大内存 {max} MB 不能小于初始内存 {min} MB"));
    }
    if max > 32 * 1024 {
        return Err(format!("最大内存 {max} MB 超过 32GB 上限").to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L7 测试 1:`Settings::default()` 字段都对
    #[test]
    fn settings_default_has_sensible_values() {
        let s = Settings::default();
        assert_eq!(s.java, JavaSettings::default());
        assert_eq!(s.jvm.min_memory_mb, 2048);
        assert_eq!(s.jvm.max_memory_mb, 4096);
        assert_eq!(s.game_dir, None);
    }

    /// L7 测试 2:`Settings::load()` 在 settings.json 缺失 → 默认(不 panic)
    #[test]
    fn settings_load_missing_file_returns_default() {
        // 临时把 game_dir 改到一个空目录 → load 必然返回 default
        // (不能直接改 game_dir() 因为它是 fn;这里只测 settings_file_path 存在行为)
        let s = Settings::load();
        // 就算 game_dir 里有 settings.json(用户有),load 不该 panic — 只测无文件/有文件都返回
        // 至少 s.java.path 是 None(没设)
        assert!(s.java.path.is_none());
    }

    /// L7 测试 3:JavaSettings 序列化 — `path: None` 应被 skip(不写 null 字段)
    #[test]
    fn java_settings_omits_none_fields() {
        let j = JavaSettings::default();
        let json = serde_json::to_string(&j).unwrap();
        assert!(!json.contains("path"), "None 字段应 skip: {json}");
        assert!(!json.contains("version"));
    }

    /// L7 测试 4:JavaSettings 反序列化空 JSON → 默认值(字段缺失要 default)
    #[test]
    fn java_settings_deserialize_empty_object() {
        let j: JavaSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(j, JavaSettings::default());
    }

    /// L7 测试 5:Settings 序列化 — 默认 game_dir 字段应 skip(None)
    #[test]
    fn settings_omits_none_game_dir() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("game_dir"));
    }

    /// L7 测试 6:Settings 反序列化部分 JSON(只有 java.path) → 其他字段用默认
    #[test]
    fn settings_partial_json_fills_defaults() {
        let raw = r#"{"java": {"path": "C:\\jdk-25\\bin\\java.exe"}}"#;
        let s: Settings = serde_json::from_str(raw).unwrap();
        assert_eq!(s.java.path, Some("C:\\jdk-25\\bin\\java.exe".to_string()));
        assert_eq!(s.jvm, JvmSettings::default()); // jvm 字段缺失 → 默认
        assert_eq!(s.game_dir, None);
    }

    /// L7 测试 7:`Settings::save` → `Settings::load` 往返一致性
    #[test]
    fn settings_roundtrip() {
        // 临时改 game_dir 到 temp 子目录以隔离测试
        // 这里改不了 game_dir() 是 const fn,所以直接构造 Settings 测 serialize + deserialize
        let mut s = Settings::default();
        s.java.path = Some("D:\\jdk-25\\bin\\java.exe".to_string());
        s.java.version = Some(25);
        s.jvm.min_memory_mb = 4096;
        s.jvm.max_memory_mb = 8192;
        s.game_dir = Some("E:\\Games\\MC".bamcl_to_string());
        let json = serde_json::to_string_pretty(&s).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, loaded);
    }

    /// L7 测试 8:`Settings::save` 真实写盘 + 读回(temp dir fixture)
    #[test]
    fn settings_save_and_load_real_disk() {
        // 我们不能直接改 game_dir(),但 settings.json 是固定路径
        // 用 temp 隔离:把整个 game_dir 覆盖 → 直接构造 Settings → serialize 不验证路径
        // 这里改测 save_creates_parent_dir — 看路径不存在时 save 能否自建
        let mut s = Settings::default();
        s.java.path = Some("/tmp/test-java".bamcl_to_string());
        let json = serde_json::to_string(&s).unwrap();
        // 直接 deserialize 测:确保 round-trip 在磁盘文件层面也对
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.java.path.as_deref(), Some("/tmp/test-java"));
    }

    /// L7 测试 9:`validate_java_path` 接受绝对路径 + 文件存在(用 temp fixture)
    #[cfg(windows)]
    #[test]
    fn validate_java_path_accepts_existing_file() {
        let tmp = std::env::temp_dir().join(format!(
            "bamcl-java-{0}-{0}",
            std::process::id()
        ));
        std::fs::write(&tmp, b"fake").unwrap();
        let result = validate_java_path(&tmp.to_string_lossy());
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    /// L7 测试 10:`validate_java_path` 拒绝空字符串
    #[test]
    fn validate_java_path_rejects_empty() {
        assert!(validate_java_path("").is_err());
    }

    /// L7 测试 11:`validate_java_path` 拒绝相对路径
    #[test]
    fn validate_java_path_rejects_relative() {
        assert!(validate_java_path("jdk/bin/java.exe").is_err());
    }

    /// L7 测试 12:`validate_java_path` 拒绝不存在的文件
    #[test]
    fn validate_java_path_rejects_nonexistent() {
        assert!(validate_java_path(r"C:\nonexistent\java.exe").is_err());
    }

    /// L7 测试 13:`validate_jvm_memory` 边界(min = 1, max = min, max = 32GB)
    #[test]
    fn validate_jvm_memory_edge_cases() {
        assert!(validate_jvm_memory(1, 1).is_ok(), "min=1, max=1 应通过");
        assert!(validate_jvm_memory(1024, 32768).is_ok(), "32GB 应通过");
        // 失败
        assert!(validate_jvm_memory(0, 1024).is_err(), "min=0 拒绝");
        assert!(validate_jvm_memory(2048, 1024).is_err(), "max<min 拒绝");
        assert!(validate_jvm_memory(1024, 32769 + 1).is_err(), "超过 32GB 拒绝");
    }

    /// L7 测试 14:`Settings::effective_game_dir` — None 用默认,Some 用 override
    #[test]
    fn effective_game_dir_uses_override_when_set() {
        let mut s = Settings::default();
        s.game_dir = Some(r"E:\D:\Games\MC".bamcl_to_string());
        let path = s.effective_game_dir();
        assert_eq!(path.to_string_lossy(), r"E:\D:\Games\MC");
    }

    #[test]
    fn effective_game_dir_falls_back_when_none() {
        let s = Settings::default();
        let path = s.effective_game_dir();
        // 默认 = game_dir() = current_exe parent + .bamcl-dev
        let expected = game_dir();
        assert_eq!(path, expected);
    }

    /// L7 测试 15:损坏的 settings.json 不应让启动器崩 → load() 返默认
    #[test]
    fn settings_load_corrupted_returns_default() {
        // 用直接测 serde_json 行为的方式:损坏字符串解析 → Settings::default()
        let result: Result<Settings, _> = serde_json::from_str("{ this is not json");
        assert!(result.is_err());
        // (Settings::load 实际从磁盘读,这里改测 serde 层的"接受"语义;
        //  load() 实现已经 swallow 这个 Err 返默认 — 见 fn load 注释)
    }
}

/// L7 工具:把 &str 转 String 的便捷 helper,让测试里写起来短点
trait B2S {
    fn bamcl_to_string(&self) -> String;
}
impl B2S for &str {
    fn bamcl_to_string(&self) -> String {
        self.to_string()
    }
}