//! M3 / L1:账户系统骨架 —— 仅离线账户(微软 OAuth 是 L2 的事)。
//!
//! 存储:
//! - `<game_dir>/accounts.json`        = `Vec<Account>`(完整账户列表)
//! - `<game_dir>/active_account.json`  = `{"id": "<uuid>"}`(当前选中账户)
//!
//! 教学要点:
//! - 缺失文件 → 返默认(同 L7 settings):启动器永远能跑
//! - 原子写 → `<file>.tmp` → rename,避免半写状态
//! - 离线账户 UUID = `Uuid::new_v3(NAMESPACE_OID, "offline:<username>")` ——
//!   同一用户名跨设备同 UUID(参考 HMCL 的「离线模式玩家 UUID 派生」做法)

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::download::game_dir;

/// M3:账户类型(serde tag = "type",外层 `lowercase` 标签)
/// 离线 / 微软两个变体;L1 只创建 Offline 变体,Microsoft 留给 L2。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Account {
    Offline(OfflineAccount),
    Microsoft(MicrosoftAccount),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineAccount {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrosoftAccount {
    pub id: Uuid,
    pub username: String,
    pub uuid: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    /// Xbox User ID —— spec §4.1 列出,launch 注入 `auth_xuid` 需要
    /// L1 旧 JSON 无此字段,`#[serde(default)]` 兜底为空字符串
    #[serde(default)]
    pub xuid: String,
}

/// M3:账户列表容器(serde 容器 struct;空文件 = 空列表)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AccountList {
    pub accounts: Vec<Account>,
}

impl AccountList {
    /// 加载 accounts.json,缺失 → 返默认
    pub fn load() -> Self {
        let path = accounts_file_path();
        if !path.is_file() {
            return AccountList::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AccountList::default(),
        }
    }

    /// 原子写盘(同 L7 settings 的 tmp+rename 套路)
    pub fn save(&self) -> Result<(), String> {
        let path = accounts_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建账户目录失败: {e}"))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化账户列表失败: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| format!("写入临时账户文件失败: {e}"))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| format!("提交账户文件失败: {e}"))?;
        Ok(())
    }

    /// 找一个 UUID 对应的账户 index(供删除 / 切换用)
    pub fn find_index(&self, id: Uuid) -> Option<usize> {
        self.accounts.iter().position(|a| a.id() == id)
    }
}

/// M3:active 账户文件容器 —— 只存 UUID,主体仍在 accounts.json
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ActiveAccount {
    pub id: Uuid,
}

impl ActiveAccount {
    /// 加载 active_account.json,缺失 → 返默认(无激活)
    pub fn load() -> Self {
        let path = active_account_file_path();
        if !path.is_file() {
            return ActiveAccount::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => ActiveAccount::default(),
        }
    }

    /// 写入 active_account.json(原子写)
    pub fn save(&self) -> Result<(), String> {
        let path = active_account_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建 active 目录失败: {e}"))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化 active 失败: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|e| format!("写入临时 active 失败: {e}"))?;
        std::fs::rename(&tmp_path, &path).map_err(|e| format!("提交 active 失败: {e}"))?;
        Ok(())
    }

    /// 清掉文件(账户被删除时调用)
    pub fn clear() {
        let path = active_account_file_path();
        if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// 账户文件路径 = `<game_dir>/accounts.json`
fn accounts_file_path() -> PathBuf {
    game_dir().join("accounts.json")
}

/// 当前激活账户文件路径 = `<game_dir>/active_account.json`
fn active_account_file_path() -> PathBuf {
    game_dir().join("active_account.json")
}

/// 校验离线账户名:Mojang 规则 —— 3-16 字符,ASCII [a-zA-Z0-9_]
pub fn validate_offline_username(username: &str) -> Result<(), String> {
    if username.len() < 3 || username.len() > 16 {
        return Err(format!(
            "用户名长度应在 3-16 字符之间(当前 {0} 字符)",
            username.len()
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("用户名只能包含字母、数字和下划线".to_string());
    }
    Ok(())
}

/// 派生离线模式 UUID —— namespace = OID, name = "offline:<username>"
/// 同一用户名跨设备同 UUID,跟正版 Mojang UUID 不会撞
pub fn derive_offline_uuid(username: &str) -> Uuid {
    Uuid::new_v3(
        &Uuid::NAMESPACE_OID,
        format!("offline:{username}").as_bytes(),
    )
}

/// 取出任意 Account 的 UUID(tag = "type" 不影响字段访问)
trait AccountId {
    fn id(&self) -> Uuid;
}
impl AccountId for Account {
    fn id(&self) -> Uuid {
        match self {
            Account::Offline(o) => o.id,
            Account::Microsoft(m) => m.id,
        }
    }
}
impl AccountId for OfflineAccount {
    fn id(&self) -> Uuid {
        self.id
    }
}
impl AccountId for MicrosoftAccount {
    fn id(&self) -> Uuid {
        self.id
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tauri commands
// ────────────────────────────────────────────────────────────────────────────

/// 列出全部账户(空列表 = 首次使用)
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, String> {
    Ok(AccountList::load().accounts)
}

/// 添加离线账户:校验用户名 → 查重 → 生成 UUID → 落盘
#[tauri::command]
pub async fn add_offline_account(username: String) -> Result<Account, String> {
    validate_offline_username(&username)?;

    let mut list = AccountList::load();

    // 重名检查(只看 Offline 变体 —— 微软账户有自己的 username 体系,允许同字符串)
    let dup = list.accounts.iter().any(|a| match a {
        Account::Offline(o) => o.username.eq_ignore_ascii_case(&username),
        Account::Microsoft(_) => false,
    });
    if dup {
        return Err(format!("已存在同名离线账户: {username}"));
    }

    let new_account = Account::Offline(OfflineAccount {
        id: derive_offline_uuid(&username),
        username,
        created_at: Utc::now(),
    });

    list.accounts.push(new_account.clone());
    list.save()?;
    Ok(new_account)
}

/// 移除账户:按 UUID 过滤 → 落盘;若被删的恰好是 active,一并清掉
#[tauri::command]
pub async fn remove_account(account_id: Uuid) -> Result<(), String> {
    let mut list = AccountList::load();
    let before = list.accounts.len();
    list.accounts.retain(|a| a.id() != account_id);
    if list.accounts.len() == before {
        return Err(format!("账户不存在: {account_id}"));
    }
    list.save()?;

    // 若被删的是 active → 清掉 active_account.json
    let active = ActiveAccount::load();
    if active.id == account_id {
        ActiveAccount::clear();
    }
    Ok(())
}

/// 切换当前激活账户:校验存在 → 写 active_account.json
#[tauri::command]
pub async fn set_active_account(account_id: Uuid) -> Result<(), String> {
    let list = AccountList::load();
    if list.find_index(account_id).is_none() {
        return Err(format!("账户不存在: {account_id}"));
    }
    ActiveAccount { id: account_id }.save()
}

/// L2 补 L1 漏的:启动时读 `active_account.json` → 找到列表里对应账户 → 返 `Some(Account)`
/// active id 为 nil / 列表里找不到 → 返 `None`
#[tauri::command]
pub async fn get_active_account() -> Result<Option<Account>, String> {
    let active = ActiveAccount::load();
    if active.id.is_nil() {
        return Ok(None);
    }
    let list = AccountList::load();
    Ok(list.accounts.into_iter().find(|a| a.id() == active.id))
}

/// L2 助手:把 MicrosoftAccount 按 `id` 覆盖写入(已在则更新,不在则追加)
/// 同时设为 active(M3 spec §4.6 「登录成功自动设为 active」)
/// `pub(crate)` 暴露给 `microsoft_auth` 模块复用
pub(crate) fn save_microsoft_account(mc: MicrosoftAccount) -> Result<Account, String> {
    let mut list = AccountList::load();
    let new_id = mc.id;
    if let Some(idx) = list.find_index(new_id) {
        list.accounts[idx] = Account::Microsoft(mc.clone());
    } else {
        list.accounts.push(Account::Microsoft(mc.clone()));
    }
    list.save()?;
    ActiveAccount { id: new_id }.save()?;
    Ok(Account::Microsoft(mc))
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// L1 测试 1:validate_offline_username 合法边界
    #[test]
    fn validate_offline_username_accepts_valid() {
        assert!(validate_offline_username("abc").is_ok());
        assert!(validate_offline_username("Steve").is_ok());
        assert!(validate_offline_username("player_123").is_ok());
        assert!(validate_offline_username("a1b2c3d4e5f6g7h8").is_ok()); // 刚好 16
    }

    /// L1 测试 2:validate_offline_username 拒绝过短 / 过长
    #[test]
    fn validate_offline_username_rejects_length() {
        assert!(validate_offline_username("").is_err());
        assert!(validate_offline_username("ab").is_err()); // < 3
        assert!(validate_offline_username("a2345678901234567").is_err()); // > 16
    }

    /// L1 测试 3:validate_offline_username 拒绝非法字符
    #[test]
    fn validate_offline_username_rejects_invalid_chars() {
        assert!(validate_offline_username("中文玩家").is_err());
        assert!(validate_offline_username("with space").is_err());
        assert!(validate_offline_username("with-dash").is_err());
        assert!(validate_offline_username("with.dot").is_err());
        assert!(validate_offline_username("with/slash").is_err());
    }

    /// L1 测试 4:derive_offline_uuid 确定性 —— 同名同 UUID
    #[test]
    fn derive_offline_uuid_is_deterministic() {
        let a = derive_offline_uuid("Steve");
        let b = derive_offline_uuid("Steve");
        assert_eq!(a, b, "同名应派生同一 UUID");
    }

    /// L1 测试 5:derive_offline_uuid 不同名不同 UUID
    #[test]
    fn derive_offline_uuid_different_names() {
        let a = derive_offline_uuid("Steve");
        let b = derive_offline_uuid("Alex");
        assert_ne!(a, b, "不同名应派生不同 UUID");
    }

    /// L1 测试 6:Account 序列化是扁平 tag 形式(serde tag = "type")
    #[test]
    fn account_serialization_is_flat_tag() {
        let acc = Account::Offline(OfflineAccount {
            id: Uuid::nil(),
            username: "Steve".bamcl_to_string(),
            created_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        });
        let json = serde_json::to_string(&acc).unwrap();
        // 预期:{"type":"offline","id":"...","username":"Steve","created_at":"..."}
        assert!(json.contains("\"type\":\"offline\""), "tag 字段: {json}");
        assert!(json.contains("\"username\":\"Steve\""));
        assert!(!json.contains("\"Offline\""), "变体名应 lowercase: {json}");
    }

    /// L1 测试 7:AccountList 默认 / 加载缺失文件返默认
    #[test]
    fn account_list_default_is_empty() {
        let l = AccountList::default();
        assert!(l.accounts.is_empty());
    }

    /// L1 测试 8:AccountList 序列化 + 反序列化 roundtrip
    #[test]
    fn account_list_roundtrip() {
        let mut l = AccountList::default();
        l.accounts.push(Account::Offline(OfflineAccount {
            id: derive_offline_uuid("Steve"),
            username: "Steve".bamcl_to_string(),
            created_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }));
        let json = serde_json::to_string_pretty(&l).unwrap();
        let loaded: AccountList = serde_json::from_str(&json).unwrap();
        assert_eq!(l, loaded);
    }

    /// L1 测试 9:AccountList 损坏 JSON → load 返默认(serde 层面验证)
    #[test]
    fn account_list_load_corrupted_returns_default() {
        let result: Result<AccountList, _> = serde_json::from_str("{ not valid");
        assert!(result.is_err());
    }

    /// L1 测试 10:ActiveAccount load 缺失 → 默认(id 全零)
    #[test]
    fn active_account_default_is_nil_uuid() {
        let a = ActiveAccount::default();
        assert_eq!(a.id, Uuid::nil());
    }

    /// L1 测试 11:ActiveAccount 序列化 + 反序列化 roundtrip
    #[test]
    fn active_account_roundtrip() {
        let a = ActiveAccount {
            id: derive_offline_uuid("Steve"),
        };
        let json = serde_json::to_string_pretty(&a).unwrap();
        let loaded: ActiveAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(a, loaded);
    }

    /// L1 测试 12:find_index 找到对应账户
    #[test]
    fn find_index_finds_account() {
        let mut l = AccountList::default();
        let acc_id = derive_offline_uuid("Steve");
        l.accounts.push(Account::Offline(OfflineAccount {
            id: acc_id,
            username: "Steve".bamcl_to_string(),
            created_at: Utc::now(),
        }));
        assert_eq!(l.find_index(acc_id), Some(0));
        assert_eq!(l.find_index(Uuid::nil()), None);
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
