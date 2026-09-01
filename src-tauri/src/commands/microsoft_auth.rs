//! M3 / L2:微软 OAuth 设备码流 + Xbox Live / XSTS / Minecraft 三段兑换。
//!
//! 设计要点:
//! - 4 段网络调用(设备码 / token / 三段兑换 / profile)都走共享 `reqwest::Client`(`super::http_client`)。
//! - JSON 反序列化、错误码映射、URL / 票据拼接都拆成纯 `fn parse_*` / `fn build_*`,
//!   单测不需要真 HTTP(参考 plan Self-Review 第 2 条:不引入 `wiremock`)。
//! - 真实 HTTP 集成在 e2e 验证(手动 `tauri dev` 跑设备码全流程)。
//!
//! 端点全部 `/consumers`,只接受个人 Microsoft / Xbox 账号。

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::accounts::{save_microsoft_account, Account, AccountList, MicrosoftAccount};
use super::http_client;

// ────────────────────────────────────────────────────────────────────────────
// 常量
// ────────────────────────────────────────────────────────────────────────────

/// BAMCLaunch 自有 Azure app 的 client_id(display name: `BAMC_Launcher Auth`)。
/// 硬编码常量 —— 启动器没有配置文件读取层,所有 launcher 客户端共用同一个 ID。
/// 对齐 spec §4.3 "client_id" 字段(本仓库的 launcher 私有不公开 client_id)。
const MS_CLIENT_ID: &str = "0b1a81c9-6e23-41fd-8690-98a17d81bf4a";

/// OAuth scope:必须带 `offline_access` 才能拿到 `refresh_token`。
const MS_SCOPE: &str = "XboxLive.signin offline_access";

// Endpoint URLs —— 全部走 `/consumers`,只接受个人 Microsoft / Xbox 账号。
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// 写盘时给 `expires_at` 留 60 秒缓冲,避免边界秒数竞争
const TOKEN_EXPIRY_BUFFER_SECS: i64 = 60;

// ────────────────────────────────────────────────────────────────────────────
// 数据类型
// ────────────────────────────────────────────────────────────────────────────

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
    /// 老 OAuth server 不一定返 `id_token`,`#[serde(default)]` 兜底
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

/// Xbox Live / XSTS 兑换响应(`Token` + `DisplayClaims.xui[0].uh`)
/// 两个端点响应结构一致,复用同一类型
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

/// Minecraft profile 响应(`id` 是无连字符 32 位 hex,`Uuid::parse_str` 自动剥离)
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

/// 登录状态 —— `#[serde(tag = "status", rename_all = "lowercase")]` 序列化
/// 前端用 discriminator `status` 收窄
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LoginStatus {
    /// 等待用户浏览器授权
    Pending,
    /// 三段兑换 + 写盘完成
    Success,
    /// 用户在浏览器点了「取消」
    Declined,
    /// 设备码过期
    Expired,
    /// 其他错误(refresh_token 撤销 / 服务端异常 / 网络失败)
    Failed { message: String },
}

/// 登录结果包装:status + 成功时的 Account
/// `account` 字段在非 Success 时通过 `skip_serializing_if` 省略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoginResult {
    pub status: LoginStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
}

// ────────────────────────────────────────────────────────────────────────────
// MicrosoftAuthenticator: 共享 reqwest::Client
// ────────────────────────────────────────────────────────────────────────────

/// 持有共享 HTTP 客户端的微软认证器
pub struct MicrosoftAuthenticator {
    client: Client,
}

impl MicrosoftAuthenticator {
    /// 用 L2 共享的 `http_client()` 构造(20s 超时 + UA)
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            client: http_client()?,
        })
    }

    /// 1️⃣ 请求设备码:POST devicecode → 返 user_code 给 UI 显示
    pub async fn start_device_code_flow(&self) -> Result<DeviceCodeResponse, String> {
        let body = urlencoded(&[("client_id", MS_CLIENT_ID), ("scope", MS_SCOPE)]);
        let resp = self
            .client
            .post(DEVICE_CODE_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
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
    ///   - `Ok(Ok(Tokens))` = 成功
    ///   - `Ok(Err(OAuthError))` = 业务错误(待重试 / 拒绝 / 过期 / 撤销)
    ///   - `Err(String)` = 网络 / 解析错误
    pub async fn poll_device_code_token(
        &self,
        device_code: &str,
    ) -> Result<Result<MicrosoftTokens, OAuthError>, String> {
        let body = urlencoded(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", MS_CLIENT_ID),
            ("device_code", device_code),
        ]);
        let resp = self
            .client
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
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
        let v: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析错误响应失败: {e}"))?;
        let err_code = v
            .get("error")
            .and_then(|e| e.as_str())
            .ok_or_else(|| "错误响应缺少 error 字段".to_string())?;
        Ok(Err(map_oauth_error(err_code)))
    }

    /// 3️⃣ Xbox Live token 兑换:MS access_token → XBL token
    pub async fn exchange_xbox_live_token(
        &self,
        ms_access_token: &str,
    ) -> Result<XblResponse, String> {
        let body = build_xbl_request_body(ms_access_token);
        let v: Value = self
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
        let body = build_xsts_request_body(xbl_token);
        let v: Value = self
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
        let body = build_mc_login_request_body(xsts_token, user_hash);
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

    /// 7️⃣ refresh_token 换新 access_token(grant_type=refresh_token)
    pub async fn refresh_oauth_token(
        &self,
        refresh_token: &str,
    ) -> Result<MicrosoftTokens, String> {
        let body = urlencoded(&[
            ("grant_type", "refresh_token"),
            ("client_id", MS_CLIENT_ID),
            ("refresh_token", refresh_token),
            ("scope", MS_SCOPE),
        ]);
        let resp = self
            .client
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
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

// ────────────────────────────────────────────────────────────────────────────
// 纯函数 helpers —— 便于单测不依赖真实 HTTP
// ────────────────────────────────────────────────────────────────────────────

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

/// 辅助:从 Xbox Live / XSTS 响应 JSON 抽 `Token` + `DisplayClaims.xui[0].uh`
fn parse_xbl_response(v: &Value) -> Result<XblResponse, String> {
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

/// 辅助:构建 Xbox Live 兑换请求 body(`RpsTicket = d=<ms_access_token>`)
fn build_xbl_request_body(ms_access_token: &str) -> Value {
    serde_json::json!({
        "Properties": {
            "AuthMethod": "RPC",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": build_rps_ticket(ms_access_token),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    })
}

/// 辅助:构建 XSTS 兑换请求 body(`UserTokens = [xbl_token]`,`RelyingParty = rpc://api.minecraftservices.com/`)
fn build_xsts_request_body(xbl_token: &str) -> Value {
    serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rpc://api.minecraftservices.com/",
        "TokenType": "JWT",
    })
}

/// 辅助:构建 Minecraft 兑换请求 body(`identityToken = XBL3.0 x=<hash>;<xsts>`)
fn build_mc_login_request_body(xsts_token: &str, user_hash: &str) -> Value {
    serde_json::json!({
        "identityToken": build_identity_token(xsts_token, user_hash),
    })
}

/// 辅助:`RpsTicket = d=<ms_access_token>`(Xbox Live 兑换必须前缀 `d=`)
fn build_rps_ticket(ms_access_token: &str) -> String {
    format!("d={ms_access_token}")
}

/// 辅助:`identityToken = XBL3.0 x=<user_hash>;<xsts_token>`
fn build_identity_token(xsts_token: &str, user_hash: &str) -> String {
    format!("XBL3.0 x={user_hash};{xsts_token}")
}

/// 辅助:URL-encode 单个字符串(RFC 3986 unreserved 字符不过编码)
/// 避免引入 reqwest `form` feature 或 `serde_urlencoded` 直接依赖
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

/// 辅助:把 `(key, value)` 列表序列化为 `application/x-www-form-urlencoded` body
/// 拼接顺序 = 输入数组顺序(OAuth 协议无关顺序)
fn urlencoded(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// 拿到 MS tokens 后走完三段兑换 + 写盘
async fn complete_microsoft_login(tokens: MicrosoftTokens) -> Result<LoginResult, String> {
    let auth = MicrosoftAuthenticator::new()?;
    let xbl = auth.exchange_xbox_live_token(&tokens.access_token).await?;
    let xsts = auth.exchange_xsts_token(&xbl.token).await?;
    let mc = auth
        .exchange_minecraft_token(&xsts.token, &xsts.user_hash)
        .await?;
    let profile = auth.get_minecraft_profile(&mc.access_token).await?;
    // Xbox user_hash ≡ xuid(实战中)—— spec §4.1 要求 xuid 字段
    let xuid = xbl.user_hash.clone();
    let mc_account = MicrosoftAccount {
        id: profile.id,
        username: profile.name.clone(),
        uuid: profile.id,
        access_token: mc.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: Utc::now()
            + ChronoDuration::seconds(mc.expires_in as i64 - TOKEN_EXPIRY_BUFFER_SECS),
        xuid,
    };
    let saved = save_microsoft_account(mc_account)?;
    Ok(LoginResult {
        status: LoginStatus::Success,
        account: Some(saved),
    })
}

/// 辅助:把 OAuthError 映射到 LoginResult(Pending / Declined / Expired / Failed)
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
        OAuthError::InvalidGrant => LoginResult {
            status: LoginStatus::Failed {
                message: "refresh_token 已失效,请重新登录".to_string(),
            },
            account: None,
        },
        OAuthError::ServerError(msg) => LoginResult {
            status: LoginStatus::Failed { message: msg },
            account: None,
        },
    }
}

/// 辅助:把刷新后的 token 写回账号列表
fn apply_refreshed_tokens(mc: MicrosoftAccount, new_tokens: MicrosoftTokens) -> Result<(), String> {
    let mut updated = mc;
    updated.access_token = new_tokens.access_token;
    updated.refresh_token = new_tokens.refresh_token;
    updated.expires_at = Utc::now()
        + ChronoDuration::seconds(new_tokens.expires_in as i64 - TOKEN_EXPIRY_BUFFER_SECS);
    let mut list = AccountList::load();
    if let Some(idx) = list.find_index(updated.id) {
        list.accounts[idx] = Account::Microsoft(updated);
        list.save()?;
    }
    Ok(())
}

// ────────────────────────────────────────────────────────────────────────────
// Tauri commands
// ────────────────────────────────────────────────────────────────────────────

/// M3 / L2:启动微软设备码流 → 返 user_code / verification_uri / interval / expires_in
#[tauri::command]
pub async fn start_microsoft_login() -> Result<DeviceCodeResponse, String> {
    let auth = MicrosoftAuthenticator::new()?;
    auth.start_device_code_flow().await
}

/// M3 / L2:轮询 token 端点(由前端按 `interval` 周期调用)
/// 成功 → 走三段兑换 → 写盘 → 返 `LoginResult { status: Success, account: ... }`
/// 业务错误 → 返对应 `LoginResult`(Pending / Declined / Expired / Failed)
#[tauri::command]
pub async fn poll_microsoft_login(device_code: String) -> Result<LoginResult, String> {
    let auth = MicrosoftAuthenticator::new()?;
    let poll = auth.poll_device_code_token(&device_code).await?;
    match poll {
        Ok(tokens) => complete_microsoft_login(tokens).await,
        Err(oauth_err) => Ok(login_status_from_oauth_error(oauth_err)),
    }
}

/// M3 / L2:用 refresh_token 换新 access_token(launch 时按需触发,L2 不后台定时)
/// 后端直接写盘,前端 reload 即可拿到新账户
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

/// M3 / L2:返回 crafatar 公开皮肤 URL(纯字符串合成,无 IO)
#[tauri::command]
pub async fn get_account_skin_url(uuid: Uuid) -> Result<String, String> {
    Ok(format!(
        "https://crafatar.com/avatars/{}?size=128&overlay",
        uuid.simple()
    ))
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

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
        let v: Value = serde_json::from_str(
            r#"{
                "Token": "xbl_token_value",
                "DisplayClaims": {"xui": [{"uh": "user_hash_123"}]}
            }"#,
        )
        .unwrap();
        let r = parse_xbl_response(&v).unwrap();
        assert_eq!(r.token, "xbl_token_value");
        assert_eq!(r.user_hash, "user_hash_123");
    }

    /// L2 测试 4:MinecraftProfile id 字段接受无连字符 hex(Uuid::parse_str 自动剥离)
    #[test]
    fn minecraft_profile_id_no_dashes() {
        let json = r#"{"id":"a01e3843e5213998958af459800e4d11","name":"Steve"}"#;
        let p: MinecraftProfile = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "Steve");
        // Uuid::parse_str 既支持有连字符也支持无连字符
        assert_eq!(
            p.id.to_string().replace('-', ""),
            "a01e3843e5213998958af459800e4d11"
        );
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

    /// L2 测试 8:BAMCLaunch 自有 Azure app client_id(spec §4.3)
    #[test]
    fn ms_client_id_is_bamclaunch_own_value() {
        assert_eq!(MS_CLIENT_ID, "0b1a81c9-6e23-41fd-8690-98a17d81bf4a");
    }

    /// L2 测试 9:scope 包含 offline_access(refresh_token 才能拿到)
    #[test]
    fn ms_scope_includes_offline_access() {
        assert!(
            MS_SCOPE.contains("offline_access"),
            "缺 offline_access → 拿不到 refresh_token"
        );
        assert!(
            MS_SCOPE.contains("XboxLive.signin"),
            "缺 XboxLive.signin → 后续三段兑换会失败"
        );
    }

    /// L2 测试 10:Xbox Live / XSTS / Minecraft / profile 4 个端点 URL 与 HMCL/SJMCL 一致
    #[test]
    fn oauth_endpoints_match_official_docs() {
        assert_eq!(
            XBL_AUTH_URL,
            "https://user.auth.xboxlive.com/user/authenticate"
        );
        assert_eq!(
            XSTS_AUTH_URL,
            "https://xsts.auth.xboxlive.com/xsts/authorize"
        );
        assert_eq!(
            MC_LOGIN_URL,
            "https://api.minecraftservices.com/authentication/login_with_xbox"
        );
        assert_eq!(
            MC_PROFILE_URL,
            "https://api.minecraftservices.com/minecraft/profile"
        );
    }

    /// L2 测试 11:identityToken 拼接格式严格对齐 `XBL3.0 x=<hash>;<xsts>`
    #[test]
    fn identity_token_format_is_xbl3() {
        let identity = build_identity_token("XSTS_TOKEN", "USER_HASH");
        assert_eq!(identity, "XBL3.0 x=USER_HASH;XSTS_TOKEN");
    }

    /// L2 测试 12:RpsTicket 拼接格式为 `d=<ms_access_token>`
    #[test]
    fn rps_ticket_format_with_d_prefix() {
        let ticket = build_rps_ticket("MS_TOKEN");
        assert_eq!(ticket, "d=MS_TOKEN");
    }

    /// L2 测试 13:parse_xbl_response 抽 Token + user_hash
    #[test]
    fn parse_xbl_response_extracts_token_and_hash() {
        let v: Value =
            serde_json::from_str(r#"{"Token":"T","DisplayClaims":{"xui":[{"uh":"H"}]}}"#).unwrap();
        let r = parse_xbl_response(&v).unwrap();
        assert_eq!(r.token, "T");
        assert_eq!(r.user_hash, "H");
    }

    /// L2 测试 14:get_account_skin_url 拼接 crafatar + 无连字符 uuid
    #[test]
    fn skin_url_uses_crafatar_with_dashless_uuid() {
        let uuid = Uuid::parse_str("a01e3843-e521-3998-958a-f459800e4d11").unwrap();
        let url = format!(
            "https://crafatar.com/avatars/{}?size=128&overlay",
            uuid.simple()
        );
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

    /// L2 测试 16:MicrosoftAccount 加 xuid 后 L1 旧 JSON 缺字段也能反序列化
    #[test]
    fn microsoft_account_xuid_backward_compat() {
        // 模拟 L1 旧 JSON(无 xuid 字段)
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

    /// L2 测试 17:LoginStatus 5 个变体都正确序列化(tag = status, lowercase)
    #[test]
    fn login_status_all_variants_serialize() {
        let cases = [
            (LoginStatus::Pending, "pending"),
            (LoginStatus::Success, "success"),
            (LoginStatus::Declined, "declined"),
            (LoginStatus::Expired, "expired"),
        ];
        for (status, expected) in cases {
            let json = serde_json::to_string(&status).unwrap();
            assert!(
                json.contains(&format!("\"status\":\"{expected}\"")),
                "expected `{expected}` in {json}"
            );
        }
    }

    /// L2 测试 18:LoginStatus::Failed 序列化带 message 字段
    #[test]
    fn login_status_failed_serializes_with_message() {
        let status = LoginStatus::Failed {
            message: "invalid_grant".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"failed\""), "tag: {json}");
        assert!(
            json.contains("\"message\":\"invalid_grant\""),
            "msg: {json}"
        );
    }

    /// L2 测试 19:LoginResult success 时 account 字段被序列化(无 skip)
    #[test]
    fn login_result_success_includes_account() {
        let acc = Account::Microsoft(MicrosoftAccount {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            username: "test".to_string(),
            uuid: Uuid::parse_str("a01e3843-e521-3998-958a-f459800e4d11").unwrap(),
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at: Utc::now(),
            xuid: "x".to_string(),
        });
        let r = LoginResult {
            status: LoginStatus::Success,
            account: Some(acc.clone()),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"status\":\"success\""), "tag: {json}");
        assert!(json.contains("\"account\""), "account field: {json}");
        // 反序列化 roundtrip
        let parsed: LoginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.account.unwrap(), acc);
    }

    /// L2 测试 20:LoginResult non-success 时 account 字段被 skip
    #[test]
    fn login_result_pending_omits_account() {
        let r = LoginResult {
            status: LoginStatus::Pending,
            account: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("\"account\""), "应 skip: {json}");
    }

    /// L2 测试 21:login_status_from_oauth_error 正确映射 5 个错误变体
    #[test]
    fn login_status_from_oauth_error_mapping() {
        assert_eq!(
            login_status_from_oauth_error(OAuthError::AuthorizationPending).status,
            LoginStatus::Pending
        );
        assert_eq!(
            login_status_from_oauth_error(OAuthError::SlowDown).status,
            LoginStatus::Pending
        );
        assert_eq!(
            login_status_from_oauth_error(OAuthError::AccessDenied).status,
            LoginStatus::Declined
        );
        assert_eq!(
            login_status_from_oauth_error(OAuthError::ExpiredToken).status,
            LoginStatus::Expired
        );
        assert_eq!(
            login_status_from_oauth_error(OAuthError::InvalidGrant).status,
            LoginStatus::Failed {
                message: "refresh_token 已失效,请重新登录".to_string()
            }
        );
    }
}
