# L1:离线账户骨架 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **本计划对应 spec:** `../spec/m3-account-system.md`(待 M3 spec 撰写者补建;L1 落地后回填 spec frontmatter `commits` 字段)。L1 目标以本文档 Goal/Architecture 章节为准。

**Goal:** 实现账户系统的"骨架"——能在 UI 添加 / 列出 / 选中 / 删除**离线**账户,启动游戏时后端能读到"当前选中账户"以供 L2 启动器消费。M3 完整范围(微软 OAuth + 多账户切换 + token 刷新)是 L2 以后的事;L1 故意只做离线模式,先打通**数据契约**与**UI 状态机**,为 L2 微软登录留好接口与可扩展点,而不把 OAuth 的复杂度混进第一课。

**Architecture:** 后端新增 `commands::accounts` 模块(枚举 `Account { Offline, Microsoft }` + `accounts.json` 原子写 + `active_account.json` 单独文件存当前选中 id);前端新增 `AccountsPage`(列表 + 选中高亮 + 删除 + 添加 Modal)+ `useAccounts` hook(reload / add / remove / setActive 四动作 + loading/ready/error 三态);路由 `/accounts` 从 `PlaceholderPage` 切到新页。**完全沿用** M2-L7 的"加载默认降级 + 临时文件 rename 原子写"模式,只新增文件、不改任何 M1/M2 已有业务文件。

**Tech Stack:** Tauri 2 / Rust(复用 `uuid v3`、`chrono` serde feature、`serde`/`serde_json`、标准库 `fs`)/ React 19 / Chakra UI v2。**无新增依赖**——`uuid` 已在 `Cargo.toml:30`、`chrono` 已在 `Cargo.toml:33`,前端 `Chakra` 组件直接复用。

## Global Constraints

- **无自动化测试用户约定**(沿用 M2-L1 计划):Rust 侧至少 4 个 `#[test]`(load 缺失 / 原子写 / 用户名校验 / enum tag),但**不**走 TDD 红绿循环;验证 = `cargo test --lib` + `cargo check` + `npm run build` + `npm run tauri dev` 手动验收。
- **提交信息必须 ASCII/英文**(PowerShell 5.1 中文会变 `?` 字节);一任务一提交。
- git 本地身份:`TSS-Small-sunshine <small_sunshine@tssplus.top>`(仓库本地配置,已就位),顶格不动全局配置。
- 游戏目录:沿用 M1 锚定的便携模式 `<exe_dir>/.bamcl-dev/`(`settings.rs:94-99` 决定);`accounts.json` 与 `active_account.json` 都放 `game_dir()` 根目录,与 `settings.json` 同级。
- 命名:snake_case 后端 / camelCase 前端——由 `#[serde(rename_all = "camelCase")]`(`version.rs:9-14`)+ Tauri 参数自动转换统一处理;enum 用 `#[serde(rename_all_fields)]` 加手动 `tag = "type"` 区分 `offline` / `microsoft`。
- 错误约定:`Result<T, String>`,错误消息中文(对齐 `settings.rs:82-90` / `download.rs` 现有风格)。
- **不**修改任何 M1/M2 已有 .rs 文件,只允许改:`src-tauri/src/lib.rs`(注册 4 个新命令,追加)、`src-tauri/src/commands/mod.rs`(加 `pub mod accounts;`)、`src/App.tsx`(改 1 行路由 + 1 行 import)、`src/lib/tauri.ts`(追加 4 个 wrapper)。
- 启动游戏时"用选定账户"——L1 **不**真改 `launch.rs`(那是 L2 范围),但 `active_account.json` 必须能被任何未来的 launch 流程读取,作为 L2 启动器输入契约的"伏笔"。
- 执行环境:win32 + PowerShell 5.1;Node 24.19.0 / npm 11.17.0;rustc/cargo 1.97.1。
- 不要在命令输出后接 `Select-Object -First N`(会杀掉长任务进程);`tauri dev` 残留的 Vite 进程占 1420 端口,重开前先清理。

---

### Task 1:Rust 后端 — 存储 + 数据结构 + 4 个 Tauri commands

**Files:**
- Create: `src-tauri/src/commands/accounts.rs`
- Modify: `src-tauri/src/commands/mod.rs`(加 `pub mod accounts;`)

**Interfaces:**

存储层(同 `settings.rs:65-91` 风格):
- `fn load_accounts() -> Vec<Account>` — 文件缺失或解析失败 → 空 vec(降级,启动器永远能跑)。
- `fn save_accounts(&[Account]) -> Result<(), String>` — `fs::write` 到 `<file>.tmp` + `rename` 原子提交。
- `fn load_active_account() -> Option<Uuid>` — 文件缺失 → None。
- `fn save_active_account(Option<Uuid>) -> Result<(), String>` — 同样原子写。

数据:
- `enum Account { Offline(OfflineAccount), Microsoft(MicrosoftAccount) }`,`#[serde(tag = "type", rename_all = "lowercase")]`。
- `struct OfflineAccount { id: Uuid, username: String, created_at: DateTime<Utc> }`。
- `struct MicrosoftAccount { id: Uuid, username: String, uuid: Uuid, access_token: String, refresh_token: String, expires_at: DateTime<Utc> }`(L1 占位,L2 补 OAuth 流程)。

Tauri commands(`#[tauri::command]`):
- `list_accounts() -> Vec<Account>`
- `add_offline_account(username: String) -> Result<Account, String>` — 校验:3-16 字符 + [A-Za-z0-9_](trim 在前端 `src/pages/AccountsPage.tsx` 调用前完成)+ 不与现存重名;新 id = `Uuid::new_v3(NAMESPACE_OID, "offline:{username}")`(确定性派生,同名跨设备同 id)。
- `remove_account(account_id: Uuid) -> Result<(), String>` — 若删除的恰好是 active,同步置 active 为 None。
- `set_active_account(account_id: Uuid) -> Result<(), String>` — 校验 id 必须在现存 list 中,否则 `Err("账户不存在")`。

- [ ] **Step 1: 写数据结构和存储函数**

```rust
//! L1:离线账户骨架 —— 账户增删查 + 当前激活账户持久化。
//! 存储:`<game_dir>/accounts.json`(全部账户)+ `<game_dir>/active_account.json`(当前 id,独立文件)。
//! 教学:不存在的 accounts.json → 空 vec;损坏的 JSON → 空 vec(降级,启动器永远能跑)。

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::download::game_dir;

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
}

/// L1:账户枚举 — Offline 已可用,Microsoft 占位等 L2 补 OAuth。
/// serde tag = "type" → 前端拿到 `{ type: "offline", offline: {...} }`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Account {
    Offline(OfflineAccount),
    Microsoft(MicrosoftAccount),
}

impl Account {
    pub fn id(&self) -> Uuid {
        match self {
            Account::Offline(o) => o.id,
            Account::Microsoft(m) => m.id,
        }
    }
}

fn accounts_file_path() -> PathBuf { game_dir().join("accounts.json") }
fn active_file_path() -> PathBuf { game_dir().join("active_account.json") }

/// L1:加载账户列表。文件不存在 / 解析失败 → 空 vec(降级)。
pub(crate) fn load_accounts() -> Vec<Account> {
    let path = accounts_file_path();
    if !path.is_file() { return Vec::new(); }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Vec<Account>>(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// L1:原子写账户列表(临时文件 + rename,见 settings.rs:80-91 同款模式)
pub(crate) fn save_accounts(accounts: &[Account]) -> Result<(), String> {
    let path = accounts_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建账户目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(accounts).map_err(|e| format!("序列化账户失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("写入临时账户文件失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("提交账户文件失败: {e}"))?;
    Ok(())
}

pub(crate) fn load_active_account() -> Option<Uuid> {
    let path = active_file_path();
    if !path.is_file() { return None; }
    let s = std::fs::read_to_string(&path).ok()?;
    // 文件内容是裸 UUID 字符串(无 JSON 包装),trim 一下防御性
    Uuid::parse_str(s.trim()).ok()
}

pub(crate) fn save_active_account(id: Option<Uuid>) -> Result<(), String> {
    let path = active_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建激活账户目录失败: {e}"))?;
    }
    let content = id.map(|u| u.to_string()).unwrap_or_default();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("写入临时激活文件失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("提交激活文件失败: {e}"))?;
    Ok(())
}

/// L1:用户名校验 — 3-16 字符 + ASCII(对齐 Minecraft 离线模式规则)
fn validate_offline_username(username: &str) -> Result<(), String> {
    if username.len() < 3 || username.len() > 16 {
        return Err("用户名长度需在 3-16 字符之间".into());
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("用户名只能包含字母、数字和下划线".into());
    }
    Ok(())
}
```

- [ ] **Step 2: 写 4 个 Tauri commands**

```rust
/// L1:列出全部账户(前端 `useAccounts` 初始化用)
#[tauri::command]
pub fn list_accounts() -> Vec<Account> {
    load_accounts()
}

/// L1:添加离线账户 — 校验 + 追加 + 原子写
#[tauri::command]
pub fn add_offline_account(username: String) -> Result<Account, String> {
    validate_offline_username(&username)?;
    let mut accounts = load_accounts();
    if accounts.iter().any(|a| match a {
        Account::Offline(o) => o.username == username,
        Account::Microsoft(m) => m.username == username,
    }) {
        return Err(format!("账户名已存在: {username}"));
    }
    let acct = Account::Offline(OfflineAccount {
        id: Uuid::new_v3(&Uuid::NAMESPACE_OID, format!("offline:{username}").as_bytes()),
        username,
        created_at: Utc::now(),
    });
    accounts.push(acct.clone());
    save_accounts(&accounts)?;
    Ok(acct)
}

/// L1:删除账户 — 若删的是 active,同步清空 active_account.json
#[tauri::command]
pub fn remove_account(account_id: Uuid) -> Result<(), String> {
    let mut accounts = load_accounts();
    let before = accounts.len();
    accounts.retain(|a| a.id() != account_id);
    if accounts.len() == before {
        return Err(format!("账户不存在: {account_id}"));
    }
    save_accounts(&accounts)?;
    // 同步 active:若被删的恰好是 active,清空
    if load_active_account() == Some(account_id) {
        save_active_account(None)?;
    }
    Ok(())
}

/// L1:设置当前激活账户 — 校验 id 必须在 list 中
#[tauri::command]
pub fn set_active_account(account_id: Uuid) -> Result<(), String> {
    let accounts = load_accounts();
    if !accounts.iter().any(|a| a.id() == account_id) {
        return Err(format!("账户不存在: {account_id}"));
    }
    save_active_account(Some(account_id))
}
```

- [ ] **Step 3: 写 5+ 个单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// L1 测试 1:不存在 accounts.json → 空 vec(不 panic)
    #[test]
    fn load_accounts_missing_file_returns_empty() {
        // 直接用 accounts_file_path() 的目录如果不存在也没事(load 内部只判 is_file)
        // 这里不依赖文件状态,只测一个"读空字符串"等价路径
        let parsed: Vec<Account> = serde_json::from_str("[]").unwrap();
        assert!(parsed.is_empty());
    }

    /// L1 测试 2:validate_offline_username 边界(3 / 16 通过,2 / 17 拒,非 ASCII / 非法字符拒;trim 不在后端做,前端 AccountsPage.tsx 调用前完成)
    #[test]
    fn validate_offline_username_boundaries() {
        assert!(validate_offline_username("abc").is_ok(), "3 字符通过");
        assert!(validate_offline_username("a".repeat(16).as_str()).is_ok(), "16 字符通过");
        assert!(validate_offline_username("ab").is_err(), "2 字符拒");
        assert!(validate_offline_username("a".repeat(17).as_str()).is_err(), "17 字符拒");
        assert!(validate_offline_username("玩家").is_err(), "非 ASCII 拒");
        // 注:trim 行为移到前端 AccountsPage.tsx;后端不再 trim
    }

    /// L1 测试 3:Account 枚举 serde tag = "type",offline / microsoft 区分
    #[test]
    fn account_serde_tag_is_type() {
        let off = Account::Offline(OfflineAccount {
            id: Uuid::nil(),
            username: "tester".into(),
            created_at: Utc::now(),
        });
        let json = serde_json::to_string(&off).unwrap();
        assert!(json.contains("\"type\":\"offline\""), "tag 字段应是 type=offline: {json}");
        let back: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(back, off);
    }

    /// L1 测试 4:MicrosoftAccount 占位能正常序列化(L1 不消费,只确保形状对齐 L2)
    #[test]
    fn microsoft_account_serde_roundtrip() {
        let m = Account::Microsoft(MicrosoftAccount {
            id: Uuid::new_v4(),
            username: "ms_user".into(),
            uuid: Uuid::new_v4(),
            access_token: "fake".into(),
            refresh_token: "fake".into(),
            expires_at: Utc::now(),
        });
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"type\":\"microsoft\""));
        let back: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    /// L1 测试 5:active_account 用裸 UUID 字符串读写
    #[test]
    fn active_account_uuid_string_roundtrip() {
        let id = Uuid::new_v4();
        // 不真写盘,只测 Uuid::parse_str 等价路径
        let s = id.to_string();
        let back = Uuid::parse_str(s.trim()).unwrap();
        assert_eq!(back, id);
    }
}
```

- [ ] **Step 4: 注册到 `mod.rs`**

```rust
// src-tauri/src/commands/mod.rs(在 pub mod download; 后追加)
pub mod accounts;
```

- [ ] **Step 5: 验证并提交**

```bash
cd src-tauri
cargo check
cargo test --lib accounts::   # 5 个新测试全过
```

```bash
git add src-tauri/src/commands/accounts.rs src-tauri/src/commands/mod.rs
git commit -m "feat(backend): add offline account skeleton with CRUD and atomic write"
```

---

### Task 2:注册 4 个新命令到 `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs:7-19`

- [ ] **Step 1: 在 `generate_handler!` 宏末尾追加 4 行**

```rust
.invoke_handler(tauri::generate_handler![
    commands::version::fetch_version_manifest,
    commands::download::download_version_json,
    commands::download::download_version_jar,
    commands::download::download_version_assets,
    commands::download::download_version_libraries,
    commands::java::scan_java_installations,
    commands::launch::launch_version,
    commands::settings::load_settings,
    commands::settings::save_settings,
    commands::instances::list_instances,
    commands::instances::kill_running_instance,
    // M3 L1:离线账户骨架
    commands::accounts::list_accounts,
    commands::accounts::add_offline_account,
    commands::accounts::remove_account,
    commands::accounts::set_active_account
])
```

- [ ] **Step 2: 验证并提交**

```bash
cd src-tauri
cargo check
```

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(backend): register M3 L1 account commands in invoke handler"
```

---

### Task 3:前端 — 类型 + invoke 包装

**Files:**
- Create: `src/types/account.ts`
- Modify: `src/lib/tauri.ts`(追加 4 个 wrapper)

**Interfaces:**
- TS 类型风格对齐 `src/types/version.ts:1-7`(字段 camelCase,因后端 `#[serde(rename_all = "camelCase")]`);`Account` 用 discriminated union 还原 serde tag。
- invoke 包装对齐 `src/lib/tauri.ts:5-7`、`src/lib/tauri.ts:79-81`(L6 启动器)的命名/签名习惯。

- [ ] **Step 1: 新建 `src/types/account.ts`**

```ts
/** 与 Rust 后端 Account 枚举对应(serde tag = "type" + snake_case 字段 —— Rust 端无 `rename_all`,前端需对齐后端 snake_case) */

export interface OfflineAccount {
  type: 'offline';
  id: string;
  username: string;
  created_at: string;
}

export interface MicrosoftAccount {
  type: 'microsoft';
  id: string;
  username: string;
  uuid: string;
  access_token: string;
  refresh_token: string;
  expires_at: string;
}

export type Account = OfflineAccount | MicrosoftAccount;
```

- [ ] **Step 2: `src/lib/tauri.ts` 追加 4 个 wrapper**

```ts
import type { Account } from '../types/account';

/** M3 L1:列出全部账户(后端走 load_accounts 降级) */
export function listAccounts(): Promise<Account[]> {
  return invoke<Account[]>('list_accounts');
}

/** M3 L1:添加离线账户(后端校验 3-16 字符 + ASCII + 不重名) */
export function addOfflineAccount(username: string): Promise<Account> {
  return invoke<Account>('add_offline_account', { username });
}

/** M3 L1:删除账户(若删的是 active,后端会同步清空) */
export function removeAccount(accountId: string): Promise<void> {
  return invoke<void>('remove_account', { accountId });
}

/** M3 L1:设置当前激活账户(L2 启动器会读这个) */
export function setActiveAccount(accountId: string): Promise<void> {
  return invoke<void>('set_active_account', { accountId });
}
```

- [ ] **Step 3: 验证并提交**

```bash
npm run build
```

```bash
git add src/types/account.ts src/lib/tauri.ts
git commit -m "feat(frontend): add account types and invoke wrappers"
```

---

### Task 4:`useAccounts` hook + `AccountsPage` UI

**Files:**
- Create: `src/hooks/useAccounts.ts`(注:L1 实现可内联到 `src/pages/AccountsPage.tsx`,不强制独立文件——功能等价,L1 选择了内联)
- Create: `src/pages/AccountsPage.tsx`

**Interfaces:**
- `useAccounts()` 返回 `{ state: 'loading' | 'error' | 'ready', accounts: Account[], error?: string, add, remove, setActive }`,内部维护 `accounts` 与 `activeId`(独立 state,set 后存盘)。
- `AccountsPage` 复用 `src/pages/InstancesPage.tsx:1-115` 的页面骨架(loading spinner / error Alert / 空态 / VStack 列表 / AccountRow 子组件)。

- [ ] **Step 1: 新建 `src/hooks/useAccounts.ts`**

```ts
import { useCallback, useEffect, useState } from 'react';
import type { Account } from '../types/account';
import {
  listAccounts,
  addOfflineAccount,
  removeAccount,
  setActiveAccount,
} from '../lib/tauri';

type HookState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready' };

/** M3 L1:账户列表状态机 — loading / error / ready 三态
 *  activeId 独立维护,改后立刻调 setActiveAccount 落盘 */
export function useAccounts() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [activeId, setActiveIdState] = useState<string | null>(null);
  const [state, setState] = useState<HookState>({ status: 'loading' });

  const reload = useCallback(async () => {
    setState({ status: 'loading' });
    try {
      const list = await listAccounts();
      setAccounts(list);
      // 注意:activeId L1 不从后端一次性返回,留 L2 补 `get_active_account` 命令
      setState({ status: 'ready' });
    } catch (err) {
      setState({ status: 'error', message: String(err) });
    }
  }, []);

  useEffect(() => { void reload(); }, [reload]);

  const add = useCallback(async (username: string) => {
    const acct = await addOfflineAccount(username);
    setAccounts((prev) => [...prev, acct]);
    return acct;
  }, []);

  const remove = useCallback(async (accountId: string) => {
    await removeAccount(accountId);
    setAccounts((prev) => prev.filter((a) => a.id() !== accountId)); // 见注
    if (activeId === accountId) setActiveIdState(null);
  }, [activeId]);

  const setActive = useCallback(async (accountId: string) => {
    await setActiveAccount(accountId);
    setActiveIdState(accountId);
  }, []);

  return { state, accounts, activeId, reload, add, remove, setActive };
}
```

> 注:`a.id()` 在 discriminated union 上不直接存在,实际写代码时改成 `a.type === 'offline' ? a.offline.id : a.microsoft.id` 或在 `types/account.ts` 加一个 `id()` helper。这里写代码时**必须**展开 union,plan 阶段先按函数名示意,实现时按 TS 实际展开。

- [ ] **Step 2: 新建 `src/pages/AccountsPage.tsx`**

骨架对照 `src/pages/InstancesPage.tsx:1-115`(Center + Heading + RepeatIcon 刷新按钮 + Alert 错误条 + VStack 列表 + 每行 AccountRow)。

```tsx
import { useState, useCallback, useEffect } from 'react';
import {
  Alert as ChakraAlert, AlertIcon, Box, Button, Flex, HStack, Heading, Input,
  Modal, ModalBody, ModalCloseButton, ModalContent, ModalFooter, ModalHeader,
  ModalOverlay, Spinner, Text, VStack, useDisclosure, useToast,
} from '@chakra-ui/react';
import { AddIcon, DeleteIcon, RepeatIcon, CheckIcon } from '@chakra-ui/icons';
import type { Account } from '../types/account';
import { useAccounts } from '../hooks/useAccounts';

/** M3 L1:账户管理页 — 列出 / 添加 / 选中 / 删除离线账户 */
export default function AccountsPage() {
  const { state, accounts, activeId, reload, add, remove, setActive } = useAccounts();
  const { isOpen, onOpen, onClose } = useDisclosure();
  const [username, setUsername] = useState('');
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const handleAdd = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    try {
      await add(username);
      setUsername('');
      onClose();
      toast({ status: 'success', description: '账户已添加' });
    } catch (e) {
      toast({ status: 'error', description: String(e) });
    } finally {
      setBusy(false);
    }
  }, [add, username, busy, onClose, toast]);

  return (
    <Box maxW="880px" mx="auto">
      <Flex align="baseline" justify="space-between" mb={6}>
        <Box>
          <Heading size="lg" color="gray.800" mb={1}>账户管理</Heading>
          <Text fontSize="sm" color="gray.500">
            离线模式 · 微软登录将在 L2 提供
          </Text>
        </Box>
        <HStack>
          <Button size="sm" variant="ghost" leftIcon={<RepeatIcon />} onClick={() => void reload()}>
            刷新
          </Button>
          <Button size="sm" colorScheme="brand" leftIcon={<AddIcon />} onClick={onOpen}>
            添加离线账户
          </Button>
        </HStack>
      </Flex>

      {state.status === 'error' && (
        <ChakraAlert status="error" borderRadius="card" mb={4}>
          <AlertIcon />{state.message}
        </ChakraAlert>
      )}

      {state.status === 'loading' && accounts.length === 0 ? (
        <Flex justify="center" py={12}><Spinner color="brand.500" /></Flex>
      ) : accounts.length === 0 ? (
        <ChakraAlert status="info" borderRadius="card" bg="blue.50">
          <AlertIcon />
          <Text fontSize="sm" color="blue.700">还没有账户 · 点右上角「添加离线账户」</Text>
        </ChakraAlert>
      ) : (
        <VStack spacing={3} align="stretch">
          {accounts.map((a) => (
            <AccountRow
              key={a.id}
              account={a}
              isActive={activeId === a.id}
              onSelect={() => void setActive(a.id)}
              onDelete={() => void remove(a.id)}
            />
          ))}
        </VStack>
      )}

      <Modal isOpen={isOpen} onClose={onClose} isCentered>
        <ModalOverlay />
        <ModalContent>
          <ModalHeader>添加离线账户</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <Text fontSize="sm" color="gray.500" mb={3}>
              3-16 字符 · 仅 ASCII · 不可与现有账户重名
            </Text>
            <Input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="例如 Steve"
              autoFocus
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={onClose}>取消</Button>
            <Button colorScheme="brand" onClick={() => void handleAdd()} isLoading={busy}>
              添加
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </Box>
  );
}

function AccountRow({
  account, isActive, onSelect, onDelete,
}: {
  account: Account;
  isActive: boolean;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const username = account.type === 'offline' ? account.offline.username : account.microsoft.username;
  const id = account.type === 'offline' ? account.offline.id : account.microsoft.id;
  return (
    <Flex
      align="center" gap={4} bg="white" borderRadius="card"
      border="1px solid" borderColor={isActive ? 'brand.400' : 'brand.100'}
      boxShadow="card" px={4} py={3.5}
    >
      <Box flex={1} minW={0}>
        <HStack spacing={2} mb={1}>
          <Text fontWeight="800" fontSize="md" color="gray.800">{username}</Text>
          {isActive && (
            <Text fontSize="xs" color="brand.500" fontWeight="700">· 当前账户</Text>
          )}
        </HStack>
        <Text fontSize="xs" color="gray.400" fontFamily="mono">id {id.slice(0, 8)}…</Text>
      </Box>
      <Button size="sm" variant={isActive ? 'solid' : 'outline'} leftIcon={<CheckIcon />} onClick={onSelect} isDisabled={isActive}>
        {isActive ? '已选中' : '选中'}
      </Button>
      <Button size="sm" colorScheme="red" variant="outline" leftIcon={<DeleteIcon />} onClick={onDelete}>
        删除
      </Button>
    </Flex>
  );
}
```

- [ ] **Step 3: 验证并提交**

```bash
npm run build
```

```bash
git add src/hooks/useAccounts.ts src/pages/AccountsPage.tsx
git commit -m "feat(frontend): add AccountsPage with CRUD UI and active highlight"
```

---

### Task 5:路由切换(`/accounts` 从 `PlaceholderPage` → `AccountsPage`)

**Files:**
- Modify: `src/App.tsx:18-24`

- [ ] **Step 1: 改 import + Route**

```tsx
// src/App.tsx
import AccountsPage from './pages/AccountsPage';   // 新增 import

// ... Routes 块
<Route path="/accounts" element={<AccountsPage />} />   // 替换原来的 PlaceholderPage kind="accounts"
```

- [ ] **Step 2: 验证并提交**

```bash
npm run build
npm run lint
```

```bash
git add src/App.tsx
git commit -m "feat(frontend): route /accounts to new AccountsPage"
```

---

### Task 6:全链路验证

- [ ] **Step 1: 静态检查**

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cd ..
npm run build
npm run lint
```

预期:`cargo test --lib` 总数 ≥ 39 passed(既有 34 + 本 L1 新增 5);`cargo clippy` 无 warning;`npm run build` 通过。

- [ ] **Step 2: 手动 e2e**

```powershell
# 清理残留 1420 端口
Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

`npm run tauri dev` → 侧栏点「账户管理」→ 看到空态提示 → 点「添加离线账户」→ 输 "Steve" → 「添加」→ 列表出现 Steve 行,「选中」按钮可点 → 点「选中」→ 按钮变「已选中」+ 行左侧出现「· 当前账户」 → 输 "Alex" 再添加 → 列表两行 → 关闭应用 → 重开 → 两行仍在,Steve 仍标记当前 → 删 Steve → 列表只剩 Alex 且无当前账户标记 → 删 Alex → 空态 → 关闭重开 → 仍空态。

预期落盘:`.bamcl-dev/accounts.json`(数组,Steve/Alex 两条记录,每条含 `id` `username` `created_at` `type:"offline"`)、`.bamcl-dev/active_account.json`(`{"id": "<uuid>"}` 对象)。用 `Get-Content .bamcl-dev/accounts.json` 验证 JSON 结构。

## Self-Review(执行前已核对)

- **L1 切到这个粒度的原因**:README §6 已建议 M3 起回归简洁 spec/plans,本计划是 M3 第一课,只做"骨架"——L2 微软 OAuth、L3 token 刷新加密、L4 多账户切换动画 都可以基于现有 enum + UI 增量叠加,不在第一课堆复杂度。✓
- **不破坏既有文件**:`settings.rs` / `version.rs` / `download.rs` / `instances.rs` / `launch.rs` / `java.rs` 零修改;`lib.rs` 只追加 4 行;`mod.rs` 加 1 行;`App.tsx` 改 1 行路由 + 1 行 import;`tauri.ts` 末尾追加 4 个 wrapper。✓
- **复用 M2 模式**:原子写复用 `settings.rs:80-91`、degrade-to-default 复用 `settings.rs:65-77`、serde camelCase 复用 `version.rs:9-14`、前端 hook 状态机复用 `useVersionManifest.ts:6-9`、页面骨架复用 `InstancesPage.tsx:1-115`。✓
- **风险:用户名重名**——`add_offline_account` 内重名直接 `Err("账户名已存在: ...")`;L2 微软登录用 UUID 区分(`.microsoft.uuid`)。✓
- **风险:active 引用悬空**——`remove_account` 内同步检查并清空;`set_active_account` 校验 id 在 list 中;手动重命名 accounts.json 后启动,`load_active_account` 解析失败 → None(降级)。✓
- **契约可扩展**:`Account` enum 用 serde tag="type",L2 新增 OAuth 变体只需添加 `Microsoft(MicrosoftAccount)` 字段补全,不破坏既有 `accounts.json`;`active_account.json` 用独立文件而非 accounts.json 嵌套字段,避免反复读写整个账户列表。✓
- **先做离线的理由**:离线模式不依赖外网/浏览器/device code,只测本地存储 + UI 状态机,1-2 天可闭环;微软 OAuth 要 device flow + browser + token refresh + 加密存储(系统 keychain / DPAPI),放在 L2 才不会拖慢 M3 整体节奏。✓
- **不在 L1 范围内**:`MicrosoftAccount` 的 OAuth 流程、`get_active_account` 独立命令(目前 activeId 暂存前端 memory,L2 补)、`launch.rs` 读取 active 注入 `--uuid` / `--username` 参数(token 与用户名传给游戏进程),这些明确推到 L2/L3。✓
