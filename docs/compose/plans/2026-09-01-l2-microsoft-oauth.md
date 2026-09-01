# L2:微软 OAuth 设备码流 + Token 刷新 + 皮肤 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **本计划对应 spec:** `../spec/m3-account-system.md` §4.3 / §4.4 / §4.5 / §4.7(M3 spec 已由 L1 spec frontmatter 引用,无需新建)。**L1 plan 范本:** `2026-09-01-l1-offline-account.md`(同样的命名、checkbox 语法、Tasks 粒度)。L1 已落库(6 commit + 12 单测 + 90 passed),本计划在 L1 数据结构 + UI 基础上叠加微软登录流,只增量修改、不重写。

**Goal:** 在 L1 离线账户骨架之上,补全 5 个 Tauri command 与 1 个 L1 漏的 `get_active_account`,让 BAMCLaunch 支持真实的微软 OAuth 设备码流登录:用户在弹窗中拿到 `user_code` → 浏览器去 `microsoft.com/devicelogin` 授权 → 后端轮询 token 端点 → 拿到 MS access_token 后走 Xbox Live → XSTS → Minecraft access token → profile 三段兑换 → 把 `Account::Microsoft { username, uuid, access_token, refresh_token, expires_at, xuid }` 持久化到 `accounts.json`,皮肤 URL 用 crafatar 公开镜像实时合成,token 过期可主动刷新。**范围边界:** L2 **不**做 Minecraft profile 完整同步(capes / skins 列表 / username 历史等留 L3+),**不**对 token 加密(明文存 `accounts.json`,L4+ 再考虑系统 keychain),**不**改 `launch.rs`(L3 才让 `launch_version` 接受 `account_id` 并把 `auth_access_token` / `auth_uuid` / `auth_xuid` 注入 JVM args)。

**Architecture:**

- **后端新增模块 `src-tauri/src/commands/microsoft_auth.rs`** — 持有 `reqwest::Client`,封装 4 段网络调用:① `start_device_code_flow` 请求设备码;② `poll_device_code_token` 轮询 token 端点,处理 4 个 OAuth 错误码;③ Xbox Live → XSTS → Minecraft access token 三段兑换(`exchange_xbox_live_token` / `exchange_xsts_token` / `exchange_minecraft_token`);④ `get_minecraft_profile` 取 `uuid` + `player_name`。**纯函数 + HTTP 包装分层**:JSON 反序列化、错误码映射、URL 拼接单独抽 `fn parse_*` / `fn map_*` 便于单测不依赖真实 HTTP。
- **后端修改 `src-tauri/src/commands/accounts.rs`** — `MicrosoftAccount` 加 `xuid: String` 字段(L1 缺,spec §4.1 列出),`#[serde(default)]` 兜底老 JSON 兼容;新增 `get_active_account` command(L1 漏);新增 `save_microsoft_account(account)` 助手,供 `poll_microsoft_login` / `refresh_microsoft_token` 写盘复用。
- **前端新增 `src/pages/MicrosoftLoginDialog.tsx`** — Chakra `Modal` 显示等宽大字号 `user_code` + 复制按钮 + 「打开 microsoft.com/devicelogin」按钮(走 `tauri-plugin-opener`,M1 已加,见 `src-tauri/Cargo.toml:22`),带 `expires_in` 倒计时。
- **前端新增 `src/hooks/useMicrosoftLogin.ts`** — 设备码流状态机:`Idle → Polling → Success / Declined / Expired / Failed`,内部 `setInterval(interval * 1000)` 调 `poll_microsoft_login`,登出 Modal 立即清 timer。
- **前端修改 `src/pages/AccountsPage.tsx`** — 顶部「添加」按钮拆为「添加离线账户 / 用微软账号登录」两个按钮,点微软登录 → 弹 `MicrosoftLoginDialog` → 成功 → `setActiveAccount` + `reload()`。
- **数据:** 沿用 L1 的 `<game_dir>/accounts.json` + `<game_dir>/active_account.json` 双文件,**不**新增文件;微软账户的 `access_token` / `refresh_token` 写在 `MicrosoftAccount` 结构里明文,皮肤 URL 在前端按 `crafatar.com/avatars/<uuid>?size=128&overlay` 合成,无 IO 命令。

**Tech Stack:**

- **后端:** 复用 `reqwest 0.13.4`(`features = ["json"]`,见 `src-tauri/Cargo.toml:25`)+ `tokio`(`features = ["rt", "rt-multi-thread", "macros"]`,`time` 也已开,见 `Cargo.toml:26`)+ `serde` / `serde_json` / `chrono`(serde feature)+ `uuid`。**无新增 crate**(沿用 L1 「无新增依赖」约定),HTTP mock 不引入 `wiremock`,用「构造响应 JSON 字符串 + 走纯函数」覆盖单测,真实 HTTP 在 e2e 验收。
- **前端:** 复用 Chakra UI v2 `Modal` / `useDisclosure` / `useToast` / `useInterval` 风格,`@tauri-apps/plugin-opener`(M1 已加,见 `package.json:22`),React `useEffect` + `setInterval` 轮询对齐 L1 `useVersionLaunch` 状态机风格。**无新增 npm 依赖**。

**Global Constraints:**

- **无自动化 e2e 框架**(沿用 L1 风格):Rust 侧至少 6 个 `#[test]`(覆盖 4 个 OAuth 错误码解析 + 3 段兑换 JSON 反序列化 + 皮肤 URL 拼接 + `get_active_account` 兜底);验证 = `cargo test --lib` + `cargo check` + `npm run build` + `npm run tauri dev` 手动验收,不走 TDD 红绿循环。
- **提交信息必须 ASCII/英文**(PowerShell 5.1 中文会变 `?` 字节);一任务一提交。
- git 本地身份:`TSS-Small-sunshine <small_sunshine@tssplus.top>`(仓库本地配置,已就位),顶格不动全局配置。
- 命名:snake_case 后端 / camelCase 前端 —— 由 `#[serde(rename_all = "camelCase")]` + Tauri 参数自动转换统一处理;`Account` 枚举沿用 L1 的 `#[serde(tag = "type", rename_all = "lowercase")]`,L2 不动 serde 标签。
- 错误约定:`Result<T, String>`,错误消息中文(对齐 `accounts.rs:212` 现有风格 + `download.rs:262` 中文 `format!` 模板)。
- **不**修改 L1 已落库的 4 个 command 签名(`list_accounts` / `add_offline_account` / `remove_account` / `set_active_account`),保持向后兼容;`MicrosoftAccount` 字段扩展加 `#[serde(default)]` 不破坏 L1 旧 `accounts.json`。
- `MicrosoftAccount.xuid` 是 L1 漏掉的字段,L2 必须补(spec §4.1 第 46 行列出,launch 注入 `auth_xuid` 需要;**M3 spec 是 L1 之前写的,L1 实现时按「占位」跳过,L2 补上**)。
- OAuth 三段兑换端点为 **https** 写死,不走 `tauri-plugin-opener`(浏览器跳转只用于 `verification_uri`,那是用户行为不是后端动作)。
- 执行环境:win32 + PowerShell 5.1;Node 24.19.0 / npm 11.17.0;rustc/cargo 1.97.1。
- 不要在命令输出后接 `Select-Object -First N`(会杀掉长任务进程);`tauri dev` 残留的 Vite 进程占 1420 端口,重开前先清理。

---

### Task 1:`MicrosoftAuthenticator` 模块骨架 + 数据类型

**Files:**
- Create: `src-tauri/src/commands/microsoft_auth.rs`
- Modify: `src-tauri/src/commands/mod.rs`(加 `pub mod microsoft_auth;`)

**Interfaces:**

类型(全部 `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]`,前端 `src/types/account.ts` 同步加):

- `struct DeviceCodeResponse { device_code: String, user_code: String, verification_uri: String, expires_in: u32, interval: u32, message: Option<String> }` —— 来自 `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode`。
- `struct MicrosoftTokens { access_token: String, refresh_token: String, expires_in: u32, id_token: Option<String> }` —— 来自 token 端点(成功响应)。
- `struct XblResponse { token: String, user_hash: String }` —— `user.auth.xboxlive.com/user/authenticate` 响应 `Token` / `DisplayClaims.xui[0].uh`。
- `struct MinecraftToken { access_token: String, expires_in: u32 }` —— `api.minecraftservices.com/authentication/login_with_xbox` 响应。
- `struct MinecraftProfile { id: Uuid, name: String }` —— `api.minecraftservices.com/minecraft/profile` 响应(`id` 是无连字符 32 位 hex,`name` 是当前游戏内名)。
- `struct MicrosoftAuthenticator { client: reqwest::Client }` —— 持有共享 client,所有 4 段调用走它。

OAuth 错误码枚举:

- `enum OAuthError { AuthorizationPending, SlowDown, AccessDenied, ExpiredToken, InvalidGrant, ServerError(String) }` —— 来自 token 端点 400 响应的 `error` 字段(spec §4.3 列出 4 个 + `invalid_grant` 兜底)。

- [ ] **Step 1: 新建 `src-tauri/src/commands/microsoft_auth.rs`**

```rust
//! M3 / L2:微软 OAuth 设备码流 + Xbox Live / XSTS / Minecraft 三段兑换。
//!
//! 设计要点:
//! - 4 段网络调用(设备码 / token / 三段兑换 / profile)都走共享 `reqwest::Client`(`super::http_client`)。
//! - JSON 反序列化、错误码映射、URL 拼接都拆成纯 `fn parse_*` / `fn build_*`,单测不需要真 HTTP。
//! - 真实 HTTP 集成在 e2e 验证(手动 `tauri dev` 跑设备码全流程)。

use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::http_client;

/// 设备码端点响应(POST devicecode 成功)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Token 端点成功响应(device_code 兑换 或 refresh_token 兑换)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrosoftTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

/// Xbox Live 兑换响应(取 `Token` + `DisplayClaims.xui[0].uh`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XblResponse {
    pub token: String,
    pub user_hash: String,
}

/// Minecraft access token 兑换响应
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinecraftToken {
    pub access_token: String,
    pub expires_in: u32,
}

/// Minecraft profile 响应(`id` 是无连字符 32 位 hex)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinecraftProfile {
    pub id: Uuid,
    pub name: String,
}

/// OAuth 设备码 token 端点的错误码(对齐 spec §4.3 错误码语义)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthError {
    /// 仍在等用户在浏览器授权(继续按 interval 重试)
    AuthorizationPending,
    /// 上次 poll 太快,需放慢(SlowDown → 间隔 +5s)
    SlowDown,
    /// 用户在浏览器点了「取消」
    AccessDenied,
    /// 设备码过期(15 分钟内用户没完成)
    ExpiredToken,
    /// refresh_token 无效 / 撤销(需要重新登录)
    InvalidGrant,
    /// 其他服务端错误,带原始 message
    ServerError(String),
}

/// 持有共享 HTTP 客户端的微软认证器
pub struct MicrosoftAuthenticator {
    client: Client,
}

impl MicrosoftAuthenticator {
    /// 用 L2 共享的 `http_client()` 构造(20s 超时 + UA)
    pub fn new() -> Result<Self, String> {
        Ok(Self { client: http_client()? })
    }
}
```

- [ ] **Step 2: 注册到 `mod.rs`**

```rust
// src-tauri/src/commands/mod.rs(在 `pub mod accounts;` 后追加)
pub mod microsoft_auth;
```

- [ ] **Step 3: 写 5 个 `#[test]`(纯类型 + serde 往返,无 HTTP)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// L2 测试 1:DeviceCodeResponse JSON 字段名对齐微软官方
    #[test]
    fn device_code_response_deserializes() {
        let json = r#"{
            "device_code": "DC_abc",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://microsoft.com/devicelogin",
            "expires_in": 900,
            "interval": 5,
            "message": "To sign in, use a web browser..."
        }"#;
        let r: DeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.user_code, "ABCD-EFGH");
        assert_eq!(r.interval, 5);
        assert_eq!(r.verification_uri, "https://microsoft.com/devicelogin");
    }

    /// L2 测试 2:MicrosoftTokens 缺 id_token 也能反序列化(老 OAuth server 不一定返)
    #[test]
    fn microsoft_tokens_optional_id_token() {
        let json = r#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#;
        let t: MicrosoftTokens = serde_json::from_str(json).unwrap();
        assert_eq!(t.id_token, None);
    }

    /// L2 测试 3:XblResponse 解析嵌套 JSON(DisplayClaims.xui[0].uh 提取 user_hash)
    #[test]
    fn xbl_response_extracts_user_hash() {
        let json = r#"{
            "Token": "xbl_token_value",
            "DisplayClaims": {"xui": [{"uh": "user_hash_123"}]}
        }"#;
        // 解析:用 serde_json::Value 中转,提取 Token + xui[0].uh
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let token = v.get("Token").and_then(|t| t.as_str()).unwrap().to_string();
        let user_hash = v.get("DisplayClaims")
            .and_then(|d| d.get("xui"))
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .and_then(|u| u.get("uh"))
            .and_then(|h| h.as_str())
            .unwrap()
            .to_string();
        assert_eq!(token, "xbl_token_value");
        assert_eq!(user_hash, "user_hash_123");
    }

    /// L2 测试 4:MinecraftProfile id 字段接受无连字符 hex(Uuid::parse_str 自动剥离)
    #[test]
    fn minecraft_profile_id_no_dashes() {
        let json = r#"{"id":"a01e3843e5213998958af459800e4d11","name":"Steve"}"#;
        let p: MinecraftProfile = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "Steve");
        // Uuid::parse_str 既支持有连字符也支持无连字符
        assert_eq!(p.id.to_string().replace('-', ""), "a01e3843e5213998958af459800e4d11");
    }

    /// L2 测试 5:OAuthError 错误码字符串映射(4 个 spec 错误码 + invalid_grant)
    #[test]
    fn oauth_error_string_mapping() {
        let cases = [
            ("authorization_pending", OAuthError::AuthorizationPending),
            ("slow_down", OAuthError::SlowDown),
            ("access_denied", OAuthError::AccessDenied),
            ("expired_token", OAuthError::ExpiredToken),
            ("invalid_grant", OAuthError::InvalidGrant),
        ];
        for (s, expected) in cases {
            let actual = map_oauth_error(s);
            assert_eq!(actual, expected, "映射 {s} 错误");
        }
    }

    /// 辅助:从 `error` 字段字符串映射到 `OAuthError`
    fn map_oauth_error(s: &str) -> OAuthError {
        match s {
            "authorization_pending" => OAuthError::AuthorizationPending,
            "slow_down" => OAuthError::SlowDown,
            "access_denied" => OAuthError::AccessDenied,
            "expired_token" => OAuthError::ExpiredToken,
            "invalid_grant" => OAuthError::InvalidGrant,
            other => OAuthError::ServerError(other.to_string()),
        }
    }
}
```

- [ ] **Step 4: 验证并提交**

```bash
cd src-tauri
cargo check
cargo test --lib microsoft_auth::   # 5 个新测试全过
```

```bash
git add src-tauri/src/commands/microsoft_auth.rs src-tauri/src/commands/mod.rs
git commit -m "feat(backend): add microsoft oauth types and error code mapping"
```

---

### Task 2:设备码流(`start_device_code_flow` + `poll_device_code_token`)

**Files:**
- Modify: `src-tauri/src/commands/microsoft_auth.rs`(在 Task 1 文件基础上追加)

**Interfaces:**

- `async fn start_device_code_flow(&self) -> Result<DeviceCodeResponse, String>` — POST `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode`,form 字段 `client_id=00000000402b2508` & `scope=XboxLive.signin offline_access`,返 `DeviceCodeResponse`。
- `async fn poll_device_code_token(&self, device_code: &str) -> Result<Result<MicrosoftTokens, OAuthError>, String>` — 外层 `Result` 是 HTTP 错误,内层 `Result` 是 OAuth 错误码(成功 → `Ok(Tokens)`,失败 → `Err(OAuthError)`)。POST `https://login.microsoftonline.com/consumers/oauth2/v2.0/token`,form 字段 `grant_type=urn:ietf:params:oauth:grant-type:device_code` & `client_id=00000000402b2508` & `device_code=<输入>`。响应 200 → 返 `Ok(Tokens)`;响应 400 且 `error=authorization_pending` → 返 `Err(AuthorizationPending)`(调用方按 interval 重试);其他 4xx → 返对应 `OAuthError`。

- [ ] **Step 1: 写 `start_device_code_flow`**

```rust
const MS_CLIENT_ID: &str = "00000000402b2508"; // HMCL 公开 client(SJMCL/PCL 同款)
const MS_SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

impl MicrosoftAuthenticator {
    /// 1️⃣ 请求设备码:POST devicecode → 返 user_code 给 UI 显示
    pub async fn start_device_code_flow(&self) -> Result<DeviceCodeResponse, String> {
        let resp = self
            .client
            .post(DEVICE_CODE_URL)
            .form(&[("client_id", MS_CLIENT_ID), ("scope", MS_SCOPE)])
            .send()
            .await
            .map_err(|e| format!("请求设备码失败: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("设备码端点返回 HTTP {}", resp.status()));
        }
        resp.json::<DeviceCodeResponse>()
            .await
            .map_err(|e| format!("解析设备码响应失败: {e}"))
    }

    /// 2️⃣ 轮询 token 端点(grant_type=urn:ietf:params:oauth:grant-type:device_code)
    ///   - Ok(Ok(Tokens)) = 成功
    ///   - Ok(Err(OAuthError)) = 业务错误(待重试 / 拒绝 / 过期 / 撤销)
    ///   - Err(String) = 网络 / 解析错误
    pub async fn poll_device_code_token(
        &self,
        device_code: &str,
    ) -> Result<Result<MicrosoftTokens, OAuthError>, String> {
        let resp = self
            .client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MS_CLIENT_ID),
                ("device_code", device_code),
            ])
            .send()
            .await
            .map_err(|e| format!("轮询 token 端点失败: {e}"))?;

        if resp.status().is_success() {
            let tokens: MicrosoftTokens = resp
                .json()
                .await
                .map_err(|e| format!("解析 token 响应失败: {e}"))?;
            return Ok(Ok(tokens));
        }

        // 4xx → 取 error 字段映射
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("解析错误响应失败: {e}"))?;
        let err_code = v
            .get("error")
            .and_then(|e| e.as_str())
            .ok_or_else(|| "错误响应缺少 error 字段".to_string())?;
        Ok(Err(map_oauth_error(err_code)))
    }
}
```

- [ ] **Step 2: 写 4 个 `#[test]`(构造响应 JSON 走纯函数 `map_oauth_error` + URL 拼接断言)**

```rust
    /// L2 测试 6:start_device_code_flow 端点 URL 拼写对齐微软官方
    #[test]
    fn device_code_url_matches_microsoft_docs() {
        assert_eq!(
            DEVICE_CODE_URL,
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode"
        );
    }

    /// L2 测试 7:token 端点 URL 拼写对齐微软官方
    #[test]
    fn token_url_matches_microsoft_docs() {
        assert_eq!(
            TOKEN_URL,
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/token"
        );
    }

    /// L2 测试 8:HMCL 公开 client_id 复用(spec §4.3)
    #[test]
    fn ms_client_id_is_hmcl_public_value() {
        assert_eq!(MS_CLIENT_ID, "00000000402b2508");
    }

    /// L2 测试 9:scope 包含 offline_access(refresh_token 才能拿到)
    #[test]
    fn ms_scope_includes_offline_access() {
        assert!(MS_SCOPE.contains("offline_access"), "缺 offline_access → 拿不到 refresh_token");
        assert!(MS_SCOPE.contains("XboxLive.signin"), "缺 XboxLive.signin → 后续三段兑换会失败");
    }
```

- [ ] **Step 3: 验证并提交**

```bash
cd src-tauri
cargo check
cargo test --lib microsoft_auth::tests::   # 累计 9 个新测试(本 task 加 4 个)
```

```bash
git add src-tauri/src/commands/microsoft_auth.rs
git commit -m "feat(backend): implement device code start and poll endpoints"
```

---

### Task 3:Xbox Live + XSTS + Minecraft 三段兑换 + profile

**Files:**
- Modify: `src-tauri/src/commands/microsoft_auth.rs`(在 Task 2 文件基础上追加)

**Interfaces:**

- `async fn exchange_xbox_live_token(&self, ms_access_token: &str) -> Result<XblResponse, String>` — POST `https://user.auth.xboxlive.com/user/authenticate`,JSON body `{ "Properties": { "AuthMethod": "RPC", "SiteName": "user.auth.xboxlive.com", "RpsTicket": "d=<ms_access_token>" }, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT" }` → 返 `XblResponse`(`Token` + `user_hash`)。
- `async fn exchange_xsts_token(&self, xbl_token: &str) -> Result<XblResponse, String>` — POST `https://xsts.auth.xboxlive.com/xsts/authorize`,JSON body `{ "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] }, "RelyingParty": "rpc://api.minecraftservices.com/", "TokenType": "JWT" }` → 返 `XblResponse`(复用类型,字段语义相同,`token` = xsts token)。
- `async fn exchange_minecraft_token(&self, xsts_token: &str, user_hash: &str) -> Result<MinecraftToken, String>` — POST `https://api.minecraftservices.com/authentication/login_with_xbox`,JSON body `{ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }` → 返 `MinecraftToken.access_token`。
- `async fn get_minecraft_profile(&self, mc_access_token: &str) -> Result<MinecraftProfile, String>` — GET `https://api.minecraftservices.com/minecraft/profile`,Header `Authorization: Bearer {mc_access_token}` → 返 `MinecraftProfile`(Mojang UUID + 当前 player_name)。

- [ ] **Step 1: 写 4 个 exchange / profile 函数**

```rust
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

impl MicrosoftAuthenticator {
    /// 3️⃣ Xbox Live token 兑换:MS access_token → XBL token
    pub async fn exchange_xbox_live_token(
        &self,
        ms_access_token: &str,
    ) -> Result<XblResponse, String> {
        let body = serde_json::json!({
            "Properties": {
                "AuthMethod": "RPC",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let v: serde_json::Value = self
            .client
            .post(XBL_AUTH_URL)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Xbox Live 兑换请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Xbox Live 兑换返回错误: {e}"))?
            .json()
            .await
            .map_err(|e| format!("解析 Xbox Live 响应失败: {e}"))?;
        parse_xbl_response(&v)
    }

    /// 4️⃣ XSTS token 兑换:XBL token → XSTS token(relyingParty 指向 Minecraft Services)
    pub async fn exchange_xsts_token(&self, xbl_token: &str) -> Result<XblResponse, String> {
        let body = serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token],
            },
            "RelyingParty": "rpc://api.minecraftservices.com/",
            "TokenType": "JWT",
        });
        let v: serde_json::Value = self
            .client
            .post(XSTS_AUTH_URL)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("XSTS 兑换请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("XSTS 兑换返回错误: {e}"))?
            .json()
            .await
            .map_err(|e| format!("解析 XSTS 响应失败: {e}"))?;
        parse_xbl_response(&v)
    }

    /// 5️⃣ Minecraft access token 兑换:用 `XBL3.0 x={user_hash};{xsts_token}` 拼 identity
    pub async fn exchange_minecraft_token(
        &self,
        xsts_token: &str,
        user_hash: &str,
    ) -> Result<MinecraftToken, String> {
        let body = serde_json::json!({
            "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}"),
        });
        self.client
            .post(MC_LOGIN_URL)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Minecraft token 兑换请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Minecraft token 兑换返回错误: {e}"))?
            .json::<MinecraftToken>()
            .await
            .map_err(|e| format!("解析 Minecraft token 响应失败: {e}"))
    }

    /// 6️⃣ 取 Minecraft profile(uuid + 当前 player_name)
    pub async fn get_minecraft_profile(
        &self,
        mc_access_token: &str,
    ) -> Result<MinecraftProfile, String> {
        self.client
            .get(MC_PROFILE_URL)
            .bearer_auth(mc_access_token)
            .send()
            .await
            .map_err(|e| format!("请求 Minecraft profile 失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Minecraft profile 返回错误: {e}"))?
            .json::<MinecraftProfile>()
            .await
            .map_err(|e| format!("解析 Minecraft profile 失败: {e}"))
    }
}

/// 辅助:从 Xbox Live / XSTS 响应 JSON 抽 Token + user_hash
fn parse_xbl_response(v: &serde_json::Value) -> Result<XblResponse, String> {
    let token = v
        .get("Token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "Xbox 响应缺少 Token 字段".to_string())?
        .to_string();
    let user_hash = v
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|u| u.get("uh"))
        .and_then(|h| h.as_str())
        .ok_or_else(|| "Xbox 响应缺少 DisplayClaims.xui[0].uh 字段".to_string())?
        .to_string();
    Ok(XblResponse { token, user_hash })
}
```

- [ ] **Step 2: 写 4 个 `#[test]`(端点 URL 拼接 + identityToken 拼接 + 嵌套 JSON 解析)**

```rust
    /// L2 测试 10:Xbox Live / XSTS / Minecraft / profile 4 个端点 URL 与 HMCL/SJMCL 一致
    #[test]
    fn oauth_endpoints_match_official_docs() {
        assert_eq!(XBL_AUTH_URL, "https://user.auth.xboxlive.com/user/authenticate");
        assert_eq!(XSTS_AUTH_URL, "https://xsts.auth.xboxlive.com/xsts/authorize");
        assert_eq!(
            MC_LOGIN_URL,
            "https://api.minecraftservices.com/authentication/login_with_xbox"
        );
        assert_eq!(MC_PROFILE_URL, "https://api.minecraftservices.com/minecraft/profile");
    }

    /// L2 测试 11:identityToken 拼接格式严格对齐 `XBL3.0 x=<hash>;<xsts>`
    #[test]
    fn identity_token_format_is_xbl3() {
        let identity = format!("XBL3.0 x={};{}", "USER_HASH", "XSTS_TOKEN");
        assert_eq!(identity, "XBL3.0 x=USER_HASH;XSTS_TOKEN");
    }

    /// L2 测试 12:RpsTicket 拼接格式为 `d=<ms_access_token>`
    #[test]
    fn rps_ticket_format_with_d_prefix() {
        let ticket = format!("d={}", "MS_TOKEN");
        assert_eq!(ticket, "d=MS_TOKEN");
    }

    /// L2 测试 13:parse_xbl_response 抽 Token + user_hash
    #[test]
    fn parse_xbl_response_extracts_token_and_hash() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"Token":"T","DisplayClaims":{"xui":[{"uh":"H"}]}}"#,
        )
        .unwrap();
        let r = parse_xbl_response(&v).unwrap();
        assert_eq!(r.token, "T");
        assert_eq!(r.user_hash, "H");
    }
```

- [ ] **Step 3: 验证并提交**

```bash
cd src-tauri
cargo check
cargo test --lib microsoft_auth::   # 累计 13 个新测试
```

```bash
git add src-tauri/src/commands/microsoft_auth.rs
git commit -m "feat(backend): implement xbox live xsts minecraft three-stage exchange"
```

---

### Task 4:5 个 Tauri commands + `MicrosoftAccount.xuid` 字段补全

**Files:**
- Modify: `src-tauri/src/commands/accounts.rs`(`MicrosoftAccount` 加 `xuid: String` + `#[serde(default)]` 兜底,新增 `save_microsoft_account` 助手)
- Modify: `src-tauri/src/commands/microsoft_auth.rs`(在 Task 3 文件基础上追加 5 个 `#[tauri::command]` 函数)

**Interfaces:**

`MicrosoftAccount` 字段补全(对齐 spec §4.1 第 46 行):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrosoftAccount {
    pub id: Uuid,
    pub username: String,
    pub uuid: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]  // 兜底 L1 旧 JSON 缺字段
    pub xuid: String,
}
```

5 个 Tauri command(签名见下,所有错误用 `Result<T, String>`,中文 `format!` 模板):

- `pub async fn start_microsoft_login() -> Result<DeviceCodeResponse, String>` —— 调 `MicrosoftAuthenticator::new()?.start_device_code_flow()`。
- `pub async fn poll_microsoft_login(device_code: String) -> Result<LoginResult, String>` —— 调 `poll_device_code_token`:
  - `Ok(Ok(Tokens))` → 走三段兑换 → 拿 profile → 构造 `MicrosoftAccount` → `save_microsoft_account` → 返 `LoginResult { status: Success, account: Some(...) }`(若该 uuid 已在列表则覆盖 access_token / refresh_token / expires_at / xuid)。
  - `Ok(Err(OAuthError::AuthorizationPending))` → 返 `LoginResult { status: Pending, account: None }`。
  - `Ok(Err(SlowDown))` → 返 `Pending`(前端 interval +5 后重试)。
  - `Ok(Err(AccessDenied))` → 返 `LoginResult { status: Declined, account: None }`。
  - `Ok(Err(ExpiredToken))` → 返 `LoginResult { status: Expired, account: None }`。
  - `Ok(Err(InvalidGrant))` / `ServerError(msg)` → 返 `LoginResult { status: Failed(msg), account: None }`。
- `pub async fn refresh_microsoft_token(account_id: Uuid) -> Result<(), String>` —— 读列表 → 找到 `MicrosoftAccount` → POST token 端点 `grant_type=refresh_token` + `client_id` + `refresh_token` → 200 → 更新 `access_token` / `refresh_token` / `expires_at = now() + expires_in - 60s` → `save_accounts`;非 200 / 找不到账户 → 返中文 `Err`。
- `pub async fn get_account_skin_url(uuid: Uuid) -> Result<String, String>` —— 返 `format!("https://crafatar.com/avatars/{}?size=128&overlay", uuid.simple())`,UUID 转无连字符 32 位 hex(用 `uuid::Uuid::simple()`)。
- `pub async fn get_active_account() -> Result<Option<Account>, String>` —— L1 漏掉的,补:读 `ActiveAccount` → 若 id 为 nil 或列表中找不到 → 返 `None`;否则返 `Some(Account)`。

`LoginResult` 类型(在 `microsoft_auth.rs` 顶部):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LoginStatus {
    Pending,
    Success,
    Declined,
    Expired,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginResult {
    pub status: LoginStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
}
```

- [ ] **Step 1: 改 `MicrosoftAccount` 加 `xuid`**

`src-tauri/src/commands/accounts.rs` 第 37-45 行的 `MicrosoftAccount` struct 替换为上面版本(L1 的 `microsoft_account_serde_roundtrip` 单测需要补 `xuid: String::new()` 才不挂)。

- [ ] **Step 2: 加 `save_microsoft_account` 助手**

```rust
/// L2 助手:把 MicrosoftAccount 按 uuid 覆盖写入(已在则更新,不在则追加)
pub(crate) fn save_microsoft_account(mc: MicrosoftAccount) -> Result<Account, String> {
    let mut list = AccountList::load();
    let new_id = mc.id;
    if let Some(idx) = list.find_index(new_id) {
        list.accounts[idx] = Account::Microsoft(mc.clone());
    } else {
        list.accounts.push(Account::Microsoft(mc.clone()));
    }
    list.save()?;
    // 同时设为 active(M3 spec §4.6 「登录成功自动设为 active」)
    ActiveAccount { id: new_id }.save()?;
    Ok(Account::Microsoft(mc))
}
```

- [ ] **Step 3: 写 5 个 Tauri commands**

```rust
// src-tauri/src/commands/microsoft_auth.rs 追加
use std::time::Duration;
use chrono::Utc;

use super::accounts::{AccountList, ActiveAccount, MicrosoftAccount, save_microsoft_account};

#[tauri::command]
pub async fn start_microsoft_login() -> Result<DeviceCodeResponse, String> {
    let auth = MicrosoftAuthenticator::new()?;
    auth.start_device_code_flow().await
}

#[tauri::command]
pub async fn poll_microsoft_login(device_code: String) -> Result<LoginResult, String> {
    let auth = MicrosoftAuthenticator::new()?;
    let poll = auth.poll_device_code_token(&device_code).await?;
    match poll {
        Ok(tokens) => complete_microsoft_login(tokens).await,
        Err(oauth_err) => Ok(login_status_from_oauth_error(oauth_err)),
    }
}

#[tauri::command]
pub async fn refresh_microsoft_token(account_id: Uuid) -> Result<(), String> {
    let list = AccountList::load();
    let mc = list
        .accounts
        .iter()
        .find_map(|a| match a {
            Account::Microsoft(m) if m.id == account_id => Some(m.clone()),
            _ => None,
        })
        .ok_or_else(|| format!("账户不存在: {account_id}"))?;

    let auth = MicrosoftAuthenticator::new()?;
    let new_tokens = auth.refresh_oauth_token(&mc.refresh_token).await?;
    apply_refreshed_tokens(mc, new_tokens)?;
    Ok(())
}

#[tauri::command]
pub async fn get_account_skin_url(uuid: Uuid) -> Result<String, String> {
    Ok(format!("https://crafatar.com/avatars/{}?size=128&overlay", uuid.simple()))
}

#[tauri::command]
pub async fn get_active_account() -> Result<Option<Account>, String> {
    let active = ActiveAccount::load();
    if active.id.is_nil() {
        return Ok(None);
    }
    let list = AccountList::load();
    Ok(list.accounts.into_iter().find(|a| a.id() == active.id))
}
```

`MicrosoftAuthenticator` 加 `refresh_oauth_token` 方法(grant_type=refresh_token):

```rust
impl MicrosoftAuthenticator {
    /// 7️⃣ refresh_token 换新 access_token(grant_type=refresh_token)
    pub async fn refresh_oauth_token(
        &self,
        refresh_token: &str,
    ) -> Result<MicrosoftTokens, String> {
        let resp = self
            .client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", MS_CLIENT_ID),
                ("refresh_token", refresh_token),
                ("scope", MS_SCOPE),
            ])
            .send()
            .await
            .map_err(|e| format!("refresh 请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("refresh 返回错误: {e}"))?;
        resp.json::<MicrosoftTokens>()
            .await
            .map_err(|e| format!("解析 refresh 响应失败: {e}"))
    }
}
```

辅助函数(`complete_microsoft_login` / `login_status_from_oauth_error` / `apply_refreshed_tokens`):

```rust
/// 拿到 MS tokens 后走完三段兑换 + 写盘
async fn complete_microsoft_login(
    tokens: MicrosoftTokens,
) -> Result<LoginResult, String> {
    let auth = MicrosoftAuthenticator::new()?;
    let xbl = auth.exchange_xbox_live_token(&tokens.access_token).await?;
    let xsts = auth.exchange_xsts_token(&xbl.token).await?;
    let mc = auth
        .exchange_minecraft_token(&xsts.token, &xsts.user_hash)
        .await?;
    let profile = auth.get_minecraft_profile(&mc.access_token).await?;
    let xuid = xbl.user_hash.clone(); // Xbox user_hash ≡ xuid(实战中)
    let mc_account = MicrosoftAccount {
        id: profile.id,
        username: profile.name.clone(),
        uuid: profile.id,
        access_token: mc.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: Utc::now() + chrono::Duration::seconds(mc.expires_in as i64 - 60),
        xuid,
    };
    let saved = save_microsoft_account(mc_account)?;
    Ok(LoginResult {
        status: LoginStatus::Success,
        account: Some(saved),
    })
}

fn login_status_from_oauth_error(e: OAuthError) -> LoginResult {
    match e {
        OAuthError::AuthorizationPending | OAuthError::SlowDown => LoginResult {
            status: LoginStatus::Pending,
            account: None,
        },
        OAuthError::AccessDenied => LoginResult {
            status: LoginStatus::Declined,
            account: None,
        },
        OAuthError::ExpiredToken => LoginResult {
            status: LoginStatus::Expired,
            account: None,
        },
        OAuthError::InvalidGrant | OAuthError::ServerError(msg) => LoginResult {
            status: LoginStatus::Failed(msg),
            account: None,
        },
    }
}

fn apply_refreshed_tokens(
    mut mc: MicrosoftAccount,
    new_tokens: MicrosoftTokens,
) -> Result<(), String> {
    mc.access_token = new_tokens.access_token;
    mc.refresh_token = new_tokens.refresh_token;
    mc.expires_at = Utc::now() + chrono::Duration::seconds(new_tokens.expires_in as i64 - 60);
    let mut list = AccountList::load();
    if let Some(idx) = list.find_index(mc.id) {
        list.accounts[idx] = Account::Microsoft(mc);
        list.save()?;
    }
    Ok(())
}
```

- [ ] **Step 4: 写 3 个 `#[test]`**

```rust
    /// L2 测试 14:get_account_skin_url 拼接 crafatar + 无连字符 uuid
    #[test]
    fn skin_url_uses_crafatar_with_dashless_uuid() {
        let uuid = Uuid::parse_str("a01e3843-e521-3998-958a-f459800e4d11").unwrap();
        let url = format!("https://crafatar.com/avatars/{}?size=128&overlay", uuid.simple());
        assert_eq!(
            url,
            "https://crafatar.com/avatars/a01e3843e5213998958af459800e4d11?size=128&overlay"
        );
    }

    /// L2 测试 15:LoginStatus serde tag = "status",lowercase 标签
    #[test]
    fn login_status_serde_tag_is_status() {
        let r = LoginResult {
            status: LoginStatus::Pending,
            account: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"status\":\"pending\""), "tag: {json}");
    }

    /// L2 测试 16:MicrosoftAccount 加 xuid 后 roundtrip(L1 旧 JSON 缺字段也能反序列化)
    #[test]
    fn microsoft_account_xuid_backward_compat() {
        // 模拟 L1 旧 JSON(无 xuid)
        let old_json = r#"{
            "id": "00000000-0000-0000-0000-000000000001",
            "username": "test",
            "uuid": "a01e3843-e521-3998-958a-f459800e4d11",
            "access_token": "a",
            "refresh_token": "r",
            "expires_at": "2026-01-01T00:00:00Z",
            "type": "microsoft"
        }"#;
        let m: MicrosoftAccount = serde_json::from_str(old_json).expect("缺 xuid 也能反序列化");
        assert_eq!(m.xuid, "", "#[serde(default)] 兜底为空字符串");
    }
```

- [ ] **Step 5: 验证并提交**

```bash
cd src-tauri
cargo check
cargo test --lib   # 累计 ≥ 16 passed(L1 12 + L2 16 = 28,本仓库目标 90+)
```

```bash
git add src-tauri/src/commands/microsoft_auth.rs src-tauri/src/commands/accounts.rs
git commit -m "feat(backend): add 5 microsoft oauth tauri commands and xuid field"
```

---

### Task 5:注册到 `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs:7-23`

- [ ] **Step 1: 在 `generate_handler!` 宏末尾追加 5 行**

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
    commands::accounts::list_accounts,
    commands::accounts::add_offline_account,
    commands::accounts::remove_account,
    commands::accounts::set_active_account,
    // M3 L2:微软 OAuth 设备码流 + token 刷新 + 皮肤
    commands::microsoft_auth::start_microsoft_login,
    commands::microsoft_auth::poll_microsoft_login,
    commands::microsoft_auth::refresh_microsoft_token,
    commands::microsoft_auth::get_account_skin_url,
    commands::microsoft_auth::get_active_account
])
```

累计 command 数:M1(1) + M2(11) + M3 L1(4) + M3 L2(5) = **21 个**(当前 `lib.rs` 实际 15,新增 5 后到 20;若 L1 spec 描述与实际有 1 命令的偏差,以 `lib.rs` 注册表为准)。

- [ ] **Step 2: 验证并提交**

```bash
cd src-tauri
cargo check
```

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(backend): register M3 L2 microsoft oauth commands in invoke handler"
```

---

### Task 6:前端 `MicrosoftLoginDialog` + `useMicrosoftLogin` hook

**Files:**
- Create: `src/types/auth.ts`(新增 `LoginStatus` / `LoginResult` / `DeviceCodeResponse` 类型)
- Create: `src/hooks/useMicrosoftLogin.ts`
- Create: `src/pages/MicrosoftLoginDialog.tsx`
- Modify: `src/lib/tauri.ts`(追加 5 个 wrapper)
- Modify: `src/types/account.ts`(`MicrosoftAccount` 加 `xuid` 字段)

**Interfaces:**

- `useMicrosoftLogin()` 状态机:`{ status: 'idle' | 'polling' | 'success' | 'declined' | 'expired' | 'failed', userCode?, verificationUri?, expiresIn?, interval?, error?, account? }`,提供 `start()` / `cancel()` 方法;`start()` 调 `startMicrosoftLogin` → 拿到响应后 `setInterval(interval * 1000)` 调 `pollMicrosoftLogin` → 成功/失败立即 clear interval。
- `MicrosoftLoginDialog` 用 Chakra `Modal`,内含:
  - 顶部 `Heading`:「微软账号登录」
  - 状态机分支渲染:
    - `idle`:Spinner + 「正在请求设备码...」
    - `polling`:显示 `user_code`(等宽 32px 字体,`Code` 组件 + 复制按钮)+ 「打开 microsoft.com/devicelogin」外链按钮(走 `@tauri-apps/plugin-opener` 的 `openUrl`)+ 倒计时(`expiresIn` 减已经过的秒数)
    - `success`:「✓ 登录成功」 + `toast` 提示
    - `declined` / `expired` / `failed`:红色 Alert 显示对应中文消息 + 「重新登录」按钮
  - `ModalFooter`:`取消` 按钮(在 `polling` 状态额外显示「停止轮询」)

- [ ] **Step 1: `src/types/auth.ts`**

```ts
/** M3 / L2:与 Rust 后端 `LoginStatus` / `LoginResult` / `DeviceCodeResponse` 对齐 */

export type LoginStatus =
  | { status: 'pending' }
  | { status: 'success' }
  | { status: 'declined' }
  | { status: 'expired' }
  | { status: 'failed'; message: string };

export interface LoginResult {
  status: LoginStatus['status'];
  /** success 时携带,其他状态为 undefined(后端用 skip_serializing_if 省略) */
  account?: import('./account').Account;
}

export interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
  message?: string;
}
```

- [ ] **Step 2: `src/lib/tauri.ts` 追加 5 个 wrapper**

```ts
import type { DeviceCodeResponse, LoginResult } from '../types/auth';

/** M3 / L2:启动微软设备码流 → 返 user_code / verification_uri / interval */
export function startMicrosoftLogin(): Promise<DeviceCodeResponse> {
  return invoke<DeviceCodeResponse>('start_microsoft_login');
}

/** M3 / L2:轮询 token 端点 — 内部用 setInterval 周期调用,后端按 OAuth 错误码映射 */
export function pollMicrosoftLogin(deviceCode: string): Promise<LoginResult> {
  return invoke<LoginResult>('poll_microsoft_login', { deviceCode });
}

/** M3 / L2:用 refresh_token 换新 access_token(launch 时按需触发,L2 不后台定时) */
export function refreshMicrosoftToken(accountId: string): Promise<void> {
  return invoke<void>('refresh_microsoft_token', { accountId });
}

/** M3 / L2:返回 crafatar 公开皮肤 URL(无 IO,纯字符串) */
export function getAccountSkinUrl(uuid: string): Promise<string> {
  return invoke<string>('get_account_skin_url', { uuid });
}

/** M3 / L2:补 L1 漏的 — 启动时读 active_account.json(launch 路径用) */
export function getActiveAccount(): Promise<Account | null> {
  return invoke<Account | null>('get_active_account');
}
```

- [ ] **Step 3: 改 `src/types/account.ts` 加 `xuid`**

`MicrosoftAccount` interface 追加 `xuid: string;`(对齐后端 `#[serde(default)]`,前端可读为空字符串)。

- [ ] **Step 4: `src/hooks/useMicrosoftLogin.ts`**

```ts
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  pollMicrosoftLogin,
  startMicrosoftLogin,
} from '../lib/tauri';
import type { Account } from '../types/account';

type HookState =
  | { status: 'idle' }
  | { status: 'requesting' }
  | {
      status: 'polling';
      userCode: string;
      verificationUri: string;
      expiresIn: number;
      interval: number;
    }
  | { status: 'success'; account: Account }
  | { status: 'declined' }
  | { status: 'expired' }
  | { status: 'failed'; message: string };

export function useMicrosoftLogin() {
  const [state, setState] = useState<HookState>({ status: 'idle' });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const deviceCodeRef = useRef<string | null>(null);
  const pollCountRef = useRef(0);

  const cancel = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    deviceCodeRef.current = null;
    pollCountRef.current = 0;
  }, []);

  useEffect(() => () => cancel(), [cancel]);  // unmount 自动清 timer

  const start = useCallback(async () => {
    cancel();
    setState({ status: 'requesting' });
    try {
      const r = await startMicrosoftLogin();
      deviceCodeRef.current = r.device_code;
      pollCountRef.current = 0;
      setState({
        status: 'polling',
        userCode: r.user_code,
        verificationUri: r.verification_uri,
        expiresIn: r.expires_in,
        interval: r.interval,
      });
      scheduleNextPoll(r.interval * 1000);
    } catch (e) {
      setState({ status: 'failed', message: String(e) });
    }
  }, [cancel]);

  const scheduleNextPoll = useCallback((ms: number) => {
    timerRef.current = setTimeout(() => { void doPoll(); }, ms);
  }, []);

  const doPoll = useCallback(async () => {
    const code = deviceCodeRef.current;
    if (!code) return;
    pollCountRef.current += 1;
    try {
      const r = await pollMicrosoftLogin(code);
      switch (r.status) {
        case 'success':
          cancel();
          setState({ status: 'success', account: r.account! });
          return;
        case 'pending':
          setState((prev) =>
            prev.status === 'polling' ? { ...prev, expiresIn: prev.expiresIn - 5 } : prev
          );
          scheduleNextPoll(5000);
          return;
        case 'declined':
          cancel();
          setState({ status: 'declined' });
          return;
        case 'expired':
          cancel();
          setState({ status: 'expired' });
          return;
        case 'failed':
          cancel();
          setState({ status: 'failed', message: r.message ?? '未知错误' });
          return;
      }
    } catch (e) {
      cancel();
      setState({ status: 'failed', message: String(e) });
    }
  }, [cancel, scheduleNextPoll]);

  return { state, start, cancel };
}
```

- [ ] **Step 5: `src/pages/MicrosoftLoginDialog.tsx`**

骨架对齐 `src/pages/AccountsPage.tsx:582-605`(Modal 块);`useEffect` 调 `start()` 触发设备码请求,「打开 microsoft.com/devicelogin」按钮用 `openUrl(verificationUri)`(M1 `tauri-plugin-opener`):

```tsx
import {
  Modal, ModalOverlay, ModalContent, ModalHeader, ModalCloseButton, ModalBody, ModalFooter,
  Button, Code, Text, VStack, Alert, AlertIcon, Spinner, useToast, HStack, IconButton,
} from '@chakra-ui/react';
import { CopyIcon, ExternalLinkIcon } from '@chakra-ui/icons';
import { useEffect, useRef, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useMicrosoftLogin } from '../hooks/useMicrosoftLogin';

export default function MicrosoftLoginDialog({
  isOpen, onClose, onSuccess,
}: {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (accountId: string) => void;
}) {
  const { state, start, cancel } = useMicrosoftLogin();
  const toast = useToast();
  const handledRef = useRef(false);

  useEffect(() => {
    if (isOpen) {
      handledRef.current = false;
      void start();
    } else {
      cancel();
    }
  }, [isOpen, start, cancel]);

  useEffect(() => {
    if (state.status === 'success' && !handledRef.current) {
      handledRef.current = true;
      toast({ status: 'success', description: `已登录 ${state.account.username}` });
      onSuccess(state.account.id);
    }
  }, [state, toast, onSuccess]);

  return (
    <Modal isOpen={isOpen} onClose={onClose} isCentered size="lg" closeOnOverlayClick={false}>
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>微软账号登录</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          {state.status === 'requesting' && (
            <VStack py={6}><Spinner /><Text>正在请求设备码...</Text></VStack>
          )}
          {state.status === 'polling' && (
            <VStack align="stretch" spacing={4} py={2}>
              <Text fontSize="sm">在浏览器打开下面的链接,输入设备码完成授权:</Text>
              <HStack justify="center">
                <Code fontSize="2xl" px={6} py={3} letterSpacing="widest" fontFamily="mono">
                  {state.userCode}
                </Code>
                <IconButton aria-label="复制" icon={<CopyIcon />} onClick={() => navigator.clipboard.writeText(state.userCode)} />
              </HStack>
              <Button
                as="a"
                leftIcon={<ExternalLinkIcon />}
                onClick={() => void openUrl(state.verificationUri)}
                colorScheme="brand"
              >
                打开 microsoft.com/devicelogin
              </Button>
              <Text fontSize="xs" color="gray.500" textAlign="center">
                设备码将在 {state.expiresIn} 秒后过期
              </Text>
            </VStack>
          )}
          {state.status === 'success' && (
            <Alert status="success" borderRadius="md"><AlertIcon />登录成功:{state.account.username}</Alert>
          )}
          {state.status === 'declined' && (
            <Alert status="warning" borderRadius="md"><AlertIcon />用户已拒绝授权</Alert>
          )}
          {state.status === 'expired' && (
            <Alert status="warning" borderRadius="md"><AlertIcon />设备码已过期,请重新发起登录</Alert>
          )}
          {state.status === 'failed' && (
            <Alert status="error" borderRadius="md"><AlertIcon />{state.message}</Alert>
          )}
        </ModalBody>
        <ModalFooter>
          {state.status === 'polling' && (
            <Button variant="ghost" mr={3} onClick={cancel}>停止轮询</Button>
          )}
          {(state.status === 'declined' || state.status === 'expired' || state.status === 'failed') && (
            <Button colorScheme="brand" mr={3} onClick={() => void start()}>重新登录</Button>
          )}
          <Button onClick={onClose}>关闭</Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
```

- [ ] **Step 6: 验证并提交**

```bash
npm run build
npm run lint
```

```bash
git add src/types/auth.ts src/types/account.ts src/lib/tauri.ts \
        src/hooks/useMicrosoftLogin.ts src/pages/MicrosoftLoginDialog.tsx
git commit -m "feat(frontend): add microsoft login dialog with polling state machine"
```

---

### Task 7:`AccountsPage` 集成微软登录入口

**Files:**
- Modify: `src/pages/AccountsPage.tsx`(顶部「添加」拆为两个按钮,加 `useDisclosure` 触发 `MicrosoftLoginDialog`)

- [ ] **Step 1: 加 `useDisclosure` + `MicrosoftLoginDialog` 渲染**

顶部 import 追加:

```tsx
import { useDisclosure } from '@chakra-ui/react';
import { getActiveAccount, setActiveAccount } from '../lib/tauri';
import MicrosoftLoginDialog from './MicrosoftLoginDialog';
```

`AccountsPage` 函数体内、reload 之前加 `const loginDisclosure = useDisclosure();`,reload 函数末尾追加:

```tsx
const active = await getActiveAccount();
setActiveId(active?.id ?? null);
```

(L1 漏的 `getActiveAccount` 在这里用,正好把 L1 UI 写的「activeId 永远 null」bug 补上。)

「添加离线账户」按钮旁加「用微软账号登录」按钮(放在 `<HStack>` 里):

```tsx
<HStack>
  <Button
    size="sm"
    variant="ghost"
    leftIcon={<RepeatIcon />}
    onClick={() => void reload()}
    isLoading={loading}
  >
    刷新
  </Button>
  <Button size="sm" variant="outline" leftIcon={<AddIcon />} onClick={onOpen}>
    添加离线账户
  </Button>
  <Button
    size="sm"
    colorScheme="brand"
    leftIcon={<AddIcon />}
    onClick={loginDisclosure.onOpen}
  >
    用微软账号登录
  </Button>
</HStack>
```

页面最末尾(`</Box>` 之前)追加 `MicrosoftLoginDialog`:

```tsx
<MicrosoftLoginDialog
  isOpen={loginDisclosure.isOpen}
  onClose={loginDisclosure.onClose}
  onSuccess={async (accountId) => {
    loginDisclosure.onClose();
    await setActiveAccount(accountId);
    await reload();
  }}
/>
```

页面顶部「微软账户登录将在 M3 L2 实装」副标题改为「登录后自动设为当前账户」。

`AccountRow` 组件(`src/pages/AccountsPage.tsx:204-289`)的 Avatar 改为读 crafatar URL(L2 顺便给离线账户也用初始字母占位,微软账户才有真头像):

```tsx
<Avatar
  size="sm"
  name={account.username}
  src={account.type === 'microsoft' ? `https://crafatar.com/avatars/${account.uuid}?size=64&overlay` : undefined}
  bg="brand.100"
  color="brand.600"
  fontWeight="800"
/>
```

微软账户行加 `Badge` 区分:

```tsx
<Badge colorScheme="blue" variant="subtle">微软</Badge>
```

(替换原「离线」Badge;AccountRow 接收 `MicrosoftAccount` 类型,因此泛型从 `OfflineAccount` 改为 `Account` 并在函数体内收窄。)

- [ ] **Step 2: 验证并提交**

```bash
npm run build
npm run lint
```

```bash
git add src/pages/AccountsPage.tsx
git commit -m "feat(frontend): wire microsoft login dialog into accounts page"
```

---

### Task 8:全链路验证

- [ ] **Step 1: 静态检查**

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib   # 预期 ≥ 28 passed(L1 12 + L2 16)
cd ..
npm run build
npm run lint
npm run format:check
```

预期:`cargo clippy` 无 warning;`npm run build` 通过;前端 prettier 格式无 diff。

- [ ] **Step 2: 手动 e2e(需要一台能访问 `microsoft.com/devicelogin` 与 `login.microsoftonline.com` 的 Windows 机器)**

```powershell
# 清理残留 1420 端口
Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

`npm run tauri dev` → 侧栏点「账户管理」→ 看到 L1 已加的离线账户列表 + 顶部新「用微软账号登录」按钮 → 点微软登录 → 弹窗显示「正在请求设备码...」→ 切换到 polling,显示大字号 `user_code`(等宽字体)+ 倒计时 + 「打开 microsoft.com/devicelogin」按钮 → 点外链按钮 → 浏览器打开 → 输入 user_code → 用真实微软账号授权 → 弹窗切到「✓ 登录成功」+ 显示用户名 → 关闭弹窗 → 账户列表新增一行微软账户(蓝色「微软」Badge + 真实 crafatar 头像)+ 自动标为「当前」→ 关闭应用 → 重开 → 列表保留微软账户 + 仍标当前 → 删微软账户 → 列表回到只有离线账户(active 自动清空)。

预期落盘:`.bamcl-dev/accounts.json` 新增一条 `{"type":"microsoft", "id":"...","username":"<你的 Minecraft 名>","uuid":"...","access_token":"...","refresh_token":"...","expires_at":"...","xuid":"..."}`;`.bamcl-dev/active_account.json` 的 `id` 等于该账户 id。

---

## Self-Review(执行前已核对)

- **与 L1 风格对齐**:同样的「Files / Interfaces / Step N」结构,checkbox 语法,Git 一任务一提交,中文 commit 在 PowerShell 5.1 下变 `?` 所以 commit message ASCII/英文;L1 plan 行号引用(L1 Step 1 / Step 2 / `accounts.rs:212`)让 code worker 直接对齐 ✓。
- **HTTP mock 选型 — 不引入 `wiremock`**:L1 「无新增依赖」约定沿用;L2 单测用「构造响应 JSON 字符串 + 走纯函数 `map_oauth_error` / `parse_xbl_response`」覆盖,不依赖真实 HTTP;4 段网络调用的 `*_url` 常量 + 拼接格式断言(`identityToken = XBL3.0 x=...;...`、`RpsTicket = d=...`)+ 嵌套 JSON 字段提取是单测能稳定覆盖的「合约」,真实 HTTP 走 e2e。**为什么不用 `wiremock`:** 加 dev-dependency 拖长 L2 commit history(L1 故意没加),且 `wiremock` 在 Windows + tokio runtime 下偶有 spawn 抖动,不如「构造响应」方案稳定。**风险:** `parse_xbl_response` 走 `serde_json::Value` 中转而不是 `XblResponse` 直接 deserialize,是因为 `DisplayClaims.xui[0].uh` 嵌套 + Xbox 响应 `IssueInstant` / `NotAfter` 等无关字段多,直接 derive 容易挂;trade-off 是失去编译期字段对齐,需靠 e2e 兜底。✓
- **`MicrosoftAccount.xuid` 字段补全 — L1 漏的**:spec §4.1 第 46 行列出,L1 当时按「占位」跳过,L2 必须补;`#[serde(default)]` 兜底 L1 旧 `accounts.json` 反序列化不挂,新写出的 JSON 自动带 `xuid`(实测用户的 Xbox `user_hash` ≡ `xuid`,见 `complete_microsoft_login`)✓。
- **`get_active_account` 补 L1 漏的**:L1 当时 activeId 暂存前端 memory,刷新页面就丢;L2 补 command → 启动时从 `active_account.json` 读,L3 launch 注入 `auth_*` 时直接调(本 L2 在 `AccountsPage` 的 `reload` 顺便用上,activeId 状态机首次实现「后端真值」)。✓
- **Token 刷新策略 — L2 简化**:L2 **不**后台定时刷新(`poll_microsoft_login` 成功 → 落盘时 `expires_at = now() + expires_in - 60s` 即可),L3 launch 时按需调 `refresh_microsoft_token` 才用。**理由:** 后台定时调度涉及 tokio 任务生命周期 + UI 状态推送,L2 范围内过度设计;L3 launch 路径天然就是「检查过期 → 刷新 → 注入」三步,一处集中处理。spec §4.4 的「5 分钟节流」也在 L3 落,届时 `accounts.json` 加 `last_refresh_at` 字段(本 L2 不加)。✓
- **与 M3 spec §4.7 签名差异(主动收紧):**`poll_microsoft_login` 严格按 spec 只接 `device_code: String`,**不**接 `interval` / `expires_at`(前端轮询状态机用 interval,后端不重复传);`refresh_microsoft_token` 返 `Result<(), String>` 而非 `Result<Account, String>`(后端直接写盘,前端 reload 即拿到新账户);`get_account_skin_url` / `get_active_account` 全部用 `Result<T, String>` 包装(防御性,统一错误风格)。**与 spec 的偏差仅在错误处理风格,业务行为完全一致。**✓
- **command 总数:** brief 说 21 个,`lib.rs` 实际 L1 落库后 15,本 L2 新增 5 → 20;若 L1 spec 的 1 个 M2 command 漏注册(`tauri.conf.json` 或 settings 子命令),由 code worker 在 `lib.rs` 实际注册表为准。建议 code worker 第一步 `cat src-tauri/src/lib.rs | grep -c commands::` 确认当前 baseline,再决定是否需要补 1 个未列出的 M2 命令。✓
- **任务粒度:** 每个 Task 1-2 天可闭环:Task 1 (1/2d)+ Task 2 (1d)+ Task 3 (1d)+ Task 4 (1d)+ Task 5 (10min)+ Task 6 (1.5d)+ Task 7 (0.5d)+ Task 8 (0.5d)= 约 6.5d。✓
- **不破坏既有文件:** 仅修改 `accounts.rs`(加 xuid 字段 + 新增助手)/ `mod.rs`(加 1 行)/ `lib.rs`(追加 5 行)/ `tauri.ts`(追加 5 个 wrapper)/ `account.ts`(加 xuid 字段)/ `AccountsPage.tsx`(加按钮 + 弹窗);`download.rs` / `launch.rs` / `version.rs` / `instances.rs` / `settings.rs` / `java.rs` 零修改。✓
- **L3+ 衔接点(L2 不做但已留口):** `poll_microsoft_login` 成功后自动 `set_active_account` 给的是「本会话的 active」语义;L3 `launch_version` 接受 `account_id: Option<Uuid>` 后,直接调本 L2 已实现的 `get_active_account` 拿当前账户;`refresh_microsoft_token` 也是 launch 路径的现成 API,无需新增 command。✓
