//! L8:实例追踪 —— 后端 spawn 出去的 Java 进程持久化 + 列表查询 + 杀进程。
//! 存储:<game_dir>/running.json(JSON + serde + 原子写)。

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::download::game_dir;

/// L8:单个运行中的实例(玩家启动过的 MC 进程)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunningInstance {
    /// Java 进程 PID
    pub pid: u32,
    /// 启动的 MC 版本 id(如 "26.2")
    pub version_id: String,
    /// 用的 java.exe 路径
    pub java_path: String,
    /// 启动时间(UTC ISO 8601)
    pub started_at: DateTime<Utc>,
}

/// L8:全部运行实例集合
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RunningInstances {
    pub instances: Vec<RunningInstance>,
}

impl RunningInstances {
    /// L8:加载 running.json,缺失返默认
    pub fn load() -> Self {
        let path = running_file_path();
        if !path.is_file() {
            return RunningInstances::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => RunningInstances::default(),
        }
    }

    /// L8:原子保存
    pub fn save(&self) -> Result<(), String> {
        let path = running_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建实例目录失败: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化实例列表失败: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| format!("写入临时实例文件失败: {e}"))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| format!("提交实例文件失败: {e}"))?;
        Ok(())
    }

    /// L8:添加一个实例
    pub fn add(&mut self, instance: RunningInstance) {
        // 同 PID 已存在 → 不重复加
        if !self.instances.iter().any(|i| i.pid == instance.pid) {
            self.instances.push(instance);
        }
    }

    /// L8:移除一个 PID(进程已退出 / 已被杀)
    pub fn remove(&mut self, pid: u32) -> bool {
        let before = self.instances.len();
        self.instances.retain(|i| i.pid != pid);
        self.instances.len() != before
    }
}

/// L8:running.json 路径(跟 settings.json 同目录)
fn running_file_path() -> PathBuf {
    game_dir().join("running.json")
}

/// L8:检查 PID 是否还活着(Windows / Unix 走不同 API)
/// 教学点:
/// - Unix: `kill(pid, 0)` 不发信号,只检查 errno(0 = 存在,ESRCH = 不存在)
/// - Windows: `OpenProcess(SYNCHRONIZE, FALSE, pid)`,非零句柄 = 存在
#[cfg(unix)]
pub fn is_pid_alive(pid: u32) -> bool {
    // libc::kill(pid, 0) = 信号 0 = 不发信号,只检测
    // 安全转换 u32 → i32(Unix PID 通常 < 2^31,但保险起见检查)
    if pid > i32::MAX as u32 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        true
    } else {
        let errno = std::io::Error::last_os_error().raw_os_error();
        errno == Some(libc::EPERM) // EPERM = 存在但没权限
    }
}

#[cfg(windows)]
pub fn is_pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        CloseHandle(handle);
        true
    }
}

/// L8:杀进程(优雅: 先 SIGTERM/CTRL+BREAK, 等 3 秒, 仍存活就 SIGKILL/taskkill /F)
/// Windows 上没有 SIGTERM 概念,优雅方式 = 发送 Ctrl+Break(WM_CLOSE 不灵),L8 直接 taskkill /T
/// Unix 上 SIGTERM → 进程能捕获后退出,3 秒后 SIGKILL 强杀
#[cfg(unix)]
pub async fn kill_instance(pid: u32) -> Result<KillResult, String> {
    if pid > i32::MAX as u32 {
        return Err(format!("PID 越界: {pid}"));
    }
    // 1) SIGTERM
    let sigterm_result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if sigterm_result != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error();
        if errno == Some(libc::ESRCH) {
            return Ok(KillResult::AlreadyGone);
        }
        return Err(format!("SIGTERM 失败: errno={errno:?}"));
    }
    // 2) 等最多 3 秒
    for _ in 0..30 {
        if !is_pid_alive(pid) {
            return Ok(KillResult::TerminatedBySigterm);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // 3) SIGKILL
    let sigkill_result = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    if sigkill_result != 0 {
        return Err("SIGKILL 也失败".to_string());
    }
    // 4) 等一秒确认
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    if !is_pid_alive(pid) {
        Ok(KillResult::ForceKilled)
    } else {
        Err("SIGKILL 后进程仍存活(系统错误?)".to_string())
    }
}

#[cfg(windows)]
pub async fn kill_instance(pid: u32) -> Result<KillResult, String> {
    use tokio::process::Command;
    // Windows 用 taskkill /F /T /PID <pid>
    //   /F = 强杀, /T = 连同子进程一起杀(MC 可能 fork 子进程)
    let output = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output()
        .await
        .map_err(|e| format!("启动 taskkill 失败: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // taskkill 退出码 0 = 成功,128 = 找不到进程
    match output.status.code() {
        Some(0) => Ok(KillResult::ForceKilled),
        Some(128) | Some(1) if stderr.contains("not found") || stdout.contains("not found") => {
            Ok(KillResult::AlreadyGone)
        }
        _ => Err(format!(
            "taskkill 失败: code={:?} stderr={}",
            output.status.code(),
            stderr.trim()
        )),
    }
}

/// L8:杀进程结果分类(Serialize 给前端用)
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillResult {
    /// 进程已不存在(可能早退)
    AlreadyGone,
    /// 优雅退出(SIGTERM)
    TerminatedBySigterm,
    /// 强杀(SIGKILL / taskkill /F)
    ForceKilled,
}

/// L8:列出运行中实例(过滤已死的)— 启动器主入口 / 「实例」页用
#[tauri::command]
pub fn list_instances() -> Vec<RunningInstance> {
    let mut r = RunningInstances::load();
    // 过滤掉已死的进程
    r.instances.retain(|i| is_pid_alive(i.pid));
    // 顺手把过滤结果保存(清理 running.json)
    let _ = r.save();
    r.instances
}

/// L8:杀进程 UI 调用 — 自动从 running.json 移除
#[tauri::command]
pub async fn kill_running_instance(pid: u32) -> Result<KillResult, String> {
    let result = kill_instance(pid).await?;
    if matches!(result, KillResult::ForceKilled | KillResult::TerminatedBySigterm | KillResult::AlreadyGone) {
        let mut r = RunningInstances::load();
        r.remove(pid);
        let _ = r.save();
    }
    Ok(result)
}

/// L8:L6 launch 启动后调用 — 登记实例 + 后台 task 等退出自动 remove
pub fn register_instance(instance: RunningInstance) {
    let mut r = RunningInstances::load();
    r.add(instance);
    let _ = r.save();
}

/// L8:L6 launch 后台 task —— 进程退出后从 running.json 移除
/// 教学:tokio::process::Child 自带 wait(),但我们要 PID 层面的清理(因为我们没保留 Child)
/// 这里 spawn 一个轮询 task:每 2 秒检查 is_pid_alive,死了就 remove
pub fn spawn_exit_watcher(pid: u32) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if !is_pid_alive(pid) {
                let mut r = RunningInstances::load();
                if r.remove(pid) {
                    let _ = r.save();
                }
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L8 测试 1:`RunningInstances::default()` 空列表
    #[test]
    fn default_is_empty() {
        let r = RunningInstances::default();
        assert!(r.instances.is_empty());
    }

    /// L8 测试 2:load 缺失文件 → 空(不 panic)
    #[test]
    fn load_missing_file_returns_default() {
        let r = RunningInstances::load();
        assert!(r.instances.is_empty());
    }

    /// L8 测试 3:add → 不重复
    #[test]
    fn add_dedupes_by_pid() {
        let mut r = RunningInstances::default();
        let inst = RunningInstance {
            pid: 100,
            version_id: "26.2".bamcl_to_string(),
            java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
            started_at: Utc::now(),
        };
        r.add(inst.clone());
        r.add(inst.clone());
        assert_eq!(r.instances.len(), 1, "同 PID 重复添加应被忽略");
    }

    /// L8 测试 4:add 多个不同 PID
    #[test]
    fn add_different_pids() {
        let mut r = RunningInstances::default();
        for pid in [100u32, 200, 300] {
            r.add(RunningInstance {
                pid,
                version_id: "26.2".bamcl_to_string(),
                java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
                started_at: Utc::now(),
            });
        }
        assert_eq!(r.instances.len(), 3);
    }

    /// L8 测试 5:remove 存在的 PID → true 且长度减一
    #[test]
    fn remove_existing_pid() {
        let mut r = RunningInstances::default();
        r.add(RunningInstance {
            pid: 100,
            version_id: "26.2".bamcl_to_string(),
            java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
            started_at: Utc::now(),
        });
        assert!(r.remove(100));
        assert!(r.instances.is_empty());
    }

    /// L8 测试 6:remove 不存在的 PID → false
    #[test]
    fn remove_nonexistent_pid() {
        let mut r = RunningInstances::default();
        assert!(!r.remove(999));
    }

    /// L8 测试 7:save + load 往返
    #[test]
    fn save_load_roundtrip() {
        let mut r = RunningInstances::default();
        r.add(RunningInstance {
            pid: 12345,
            version_id: "26.2".bamcl_to_string(),
            java_path: "D:\\jdk\\java.exe".bamcl_to_string(),
            started_at: Utc::now(),
        });
        let json = serde_json::to_string_pretty(&r).unwrap();
        let loaded: RunningInstances = serde_json::from_str(&json).unwrap();
        assert_eq!(r, loaded);
    }

    /// L8 测试 8:损坏 JSON → load 返默认
    #[test]
    fn load_corrupted_returns_default() {
        let result: Result<RunningInstances, _> = serde_json::from_str("{ not valid");
        assert!(result.is_err());
        // (实际 load() 已 swallow 返回 default,这里验证 serde 行为)
    }

    /// L8 测试 9:`is_pid_alive` —— 当前进程的 PID 应该活着,任意大的假 PID 应不存在
    #[test]
    fn is_pid_alive_for_self_and_fake() {
        let self_pid = std::process::id();
        assert!(is_pid_alive(self_pid), "当前进程应活着");
        // 0xFFFFFFFF(u32::MAX) 不可能存在
        assert!(!is_pid_alive(u32::MAX));
    }

    /// L8 测试 10:`is_pid_alive` 对 0 PID(Windows 上 PID 0 是 idle 进程,Unix 上是 scheduler)
    /// 跳过断言 — 不同 OS 行为不同
    #[cfg(windows)]
    #[test]
    fn is_pid_alive_pid_0() {
        // Windows 上 PID 0 = System Idle Process,OpenProcess 会拒绝访问 → 我们返 false
        // 但严格说它是"存在"的(只是无访问权)
        // 这里不强制断言,只测函数不 panic
        let _ = is_pid_alive(0);
    }

    /// L8 测试 11:杀已不存在的 PID → `AlreadyGone`(不报错)
    #[cfg(unix)]
    #[tokio::test]
    async fn kill_already_gone_pid() {
        // 用一个超大 PID 保证不存在
        let result = kill_instance(u32::MAX).await;
        assert_eq!(result.unwrap(), KillResult::AlreadyGone);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn kill_already_gone_pid() {
        let result = kill_instance(u32::MAX).await;
        // Windows 上 taskkill 返回 128 + "not found" → AlreadyGone
        assert_eq!(result.unwrap(), KillResult::AlreadyGone);
    }

    /// L8 测试 12:`RunningInstance` 序列化包含所有字段
    #[test]
    fn instance_serialization_roundtrip() {
        let inst = RunningInstance {
            pid: 42,
            version_id: "26.2".bamcl_to_string(),
            java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
            started_at: DateTime::parse_from_rfc3339("2026-08-29T16:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&inst).unwrap();
        assert!(json.contains("\"pid\":42"));
        assert!(json.contains("\"version_id\":\"26.2\""));
        assert!(json.contains("\"started_at\""));
        let loaded: RunningInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(inst, loaded);
    }

    /// L8 测试 13:实例过滤 — 清理已退出的进程(供 list 时过滤)
    #[test]
    fn prune_dead_instances() {
        let mut r = RunningInstances::default();
        // 加两个:一个还活着(self pid),一个死了(u32::MAX)
        r.add(RunningInstance {
            pid: std::process::id(),
            version_id: "26.2".bamcl_to_string(),
            java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
            started_at: Utc::now(),
        });
        r.add(RunningInstance {
            pid: u32::MAX,
            version_id: "26.3".bamcl_to_string(),
            java_path: "C:\\jdk\\java.exe".bamcl_to_string(),
            started_at: Utc::now(),
        });
        // 过滤掉死的
        r.instances.retain(|i| is_pid_alive(i.pid));
        assert_eq!(r.instances.len(), 1);
        assert_eq!(r.instances[0].pid, std::process::id());
    }
}

trait B2S {
    fn bamcl_to_string(&self) -> String;
}
impl B2S for &str {
    fn bamcl_to_string(&self) -> String {
        self.to_string()
    }
}