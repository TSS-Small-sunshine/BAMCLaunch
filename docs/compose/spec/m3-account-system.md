---
feature: m3-account-system
status: planned
updated: 2026-09-01
branch: main
commits: <base-sha>..<head-sha> # filled at delivery
---

# BAMCLaunch 里程碑 3:账户系统

## Report

**What will be built** — M3 交付完整账户系统:微软 OAuth 设备码流登录 + 离线模式登录,账户在 `<game_dir>/accounts.json` 持久化(沿用 M2-L7 settings.json 模式),皮肤 URL 通过 crafatar 公开镜像实时合成,前端 `AccountsPage` 替换 `PlaceholderPage`(路由 `/accounts`)。账户数据(玩家名/UUID/access_token)通过新参数 `account_id` 注入到 M2-L6 的 `launch_version` 命令,目前 `launch.rs:294-309` 写死的 `"Player"` 与空 token 在 M3 中退役。

**Verification** — `cargo test` 在 M2 既有 34 passed 基础上新增 ≥6 个(账户 JSON 往返 / 离线 UUID 算法 / token 过期判定等);`npm run build` ✓;`tauri dev` e2e:① 添加离线账户 `Tester` → 启动 26.2 → `auth_player_name=Tester`、`auth_uuid=a01e38…` 风格的离线 UUID 注入 JVM args ② 添加微软账户 → 设备码弹窗显示 user_code → 用户去 `microsoft.com/devicelogin` 完成 → 自动写入 accounts.json → 选中该账户启动 → `auth_access_token` 非空。`accounts.json` 缺失 / 损坏 / 字段缺失均降级到 `[]`(沿用 M2-L7 `Settings::load` 策略,见 `src-tauri/src/commands/settings.rs:65-77`)。

**关系** — M3 上承 M2-L6(`launch_version` 接收 account_id 而非仅 java_path)、M2-L7(settings 持久化范式);下启 M4(资源下载可按账户统计)、M5(微软登录后实链 Xbox/Minecraft Services 取完整 profile)。

## [S1] Problem

M1(版本清单)、M2(下载 + 启动)已上线,但 M2-L6 的 `launch_version` 在 `src-tauri/src/commands/launch.rs:294-309` 把玩家身份硬编码为:

- `auth_player_name = "Player"`
- `auth_uuid = uuid_offline_for("Player")` → `a01e3843-e521-3998-958a-f459800e4d11`(已用 L6 测试 13 锁定,见 `launch.rs:671-678`)
- `auth_access_token = ""` → 实链正版服务器会被 Mojang session server 拒绝

玩家核心需求:

1. **作为正版玩家**,我能用微软账号登录 BAMCLaunch,启动后用我的 Mojang 身份进入正版服务器(此时 `auth_access_token` 必须有效)
2. **作为离线玩家**(没买 Java 版),我能用任意用户名启动,`auth_uuid` 稳定可复算(重新安装不换号)
3. **作为多账号玩家**,我能在本机保存多个账户并切换
4. **作为追求仪式感的玩家**,我能在账户列表看到自己皮肤头像

参考 SJMCL、HMCL、PCL 都是「离线 + 微软」双轨,微软登录统一走 OAuth 2.0 设备码流(Desktop app 不弹内嵌浏览器,用户体验最干净)。

## [S2] Design

### 4.1 账户类型与存储

```
Account (enum,serde tag = "type")
├── Offline    { id: Uuid, username: String, created_at: DateTime<Utc> }  // id = Uuid::new_v3(NAMESPACE_OID, "offline:{username}"),确定性派生(同名跨设备同 id,参考 HMCL 离线 UUID 做法)
└── Microsoft  { uuid: String, player_name: String,    // uuid / name 从 Xbox Live profile 取
                access_token: String, refresh_token: String,
                expires_at: DateTime<Utc>,             // chrono::Utc + serde
                xuid: String }                         // Xbox User ID,正版会话需要
```

- 存储文件(两个独立 JSON):`<game_dir>/accounts.json`(账户列表,顶层 = `Vec<Account>` 数组) + `<game_dir>/active_account.json`(当前激活账户,顶层 = `{"id": "<uuid>"}` 对象),均沿用 M2-L7 锚点 `game_dir()`,见 `src-tauri/src/commands/settings.rs:106`
- active 状态:存在 → 文件内有 `{"id": "..."}`;不存在 → 文件被 `ActiveAccount::clear()` 整文件删掉(而不是空对象 / None)
- 持久化策略复用 `Settings::save()` 的**原子写盘**模式:`<file>.tmp` → rename,见 `settings.rs:80-91`
- 加载策略复用 `Settings::load()` 的**降级语义**:文件不存在/解析失败 → 空账户列表、active=None,启动器永远能跑,见 `settings.rs:65-77`
- 教学点:账户文件 0 字节 / 部分字段缺失 / 旧 schema 字段冗余 — serde `#[serde(default)]` 兜底,跟 settings.json 保持一致(见 `settings.rs:52-58`)

### 4.2 离线模式

- UUID 算法**复用** `launch.rs:468-477` 的 `uuid_offline_for(player_name)`,M3 只把它抽到 `commands/account.rs::mod` 供账户创建 + 启动两个调用方共用(避免双份实现)
- 用户名规则(Mojang 协议要求):3-16 字符、`[A-Za-z0-9_]`,前后端各做一次校验(防御性,信任边界在 Rust 侧)
- 添加时即时派生 UUID,持久化进 `accounts.json` 的 `Offline.id` 字段(避免每次启动都重算)

### 4.3 微软 OAuth 设备码流

| 步骤 | 端点 | 方法 | 载荷关键字段 |
|---|---|---|---|
| 1. 请求设备码 | `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` | POST form | `client_id`, `scope` |
| 2. 用户在浏览器输入 user_code | `https://microsoft.com/devicelogin`(由 verification_uri 字段给) | 浏览器 | (用户交互) |
| 3. 轮询 token 端点 | `https://login.microsoftonline.com/consumers/oauth2/v2.0/token` | POST form | `grant_type=urn:ietf:params:oauth:grant-type:device_code`, `device_code` |

- **client_id**:HMCL 公开的 `00000000402b2508`(Mojang 为 Minecraft Java 启用的公开 client,无 secret、PKCE 可选;SJMCL、HMCL、PCL 均使用此 ID,Azure 端属「公共客户端/原生应用」)
- **scope**:`XboxLive.signin offline_access`(`offline_access` 必带,才能换 refresh_token)
- **tenant**:`/consumers`(只允许个人微软账号,跟 Yggdrasil 协议一致;`/organizations` 会拒个人账号)
- 设备码响应字段:`{ device_code, user_code, verification_uri, expires_in(默认 900s), interval(默认 5s), message }`
- 轮询错误码语义(全部来自 [OAuth 2.0 device authorization grant](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code)):
  - `authorization_pending` → 继续按 `interval` 秒重试
  - `authorization_declined` → 中止,UI 提示「用户拒绝授权」
  - `expired_token` → 中止,UI 提示「设备码已过期,请重新发起登录」
  - `invalid_grant` / 其他 → 中止 + 错误消息透传
- 成功响应:`{ access_token, refresh_token, expires_in, id_token }`
- **后续 Xbox Live 兑换**(M3 内做最小可用版):access_token → XSTS token → Minecraft access token → profile(取 uuid / player_name)
  - 这是 HMCL `MicrosoftAuthenticator` 的标准链路,M3 取最小集保证「能进正版服务器」即可
  - 完整 Xbox Live 文档见 [Minecraft Services API](https://minecraft.wiki/w/Microsoft_account)

### 4.4 Token 刷新与持久化

- 写入 `accounts.json` 时,`expires_at = now() + expires_in - 60s`(**-60s 缓冲**避免边界竞争)
- 启动检查:任意 Microsoft 账户 `now() >= expires_at` → 自动 `refresh_microsoft_token` 后再启动;若 refresh 失败(`invalid_grant` 通常意味着用户撤销了授权)→ UI 标记账户为「需重新登录」
- `refresh_microsoft_token` 走 `https://login.microsoftonline.com/consumers/oauth2/v2.0/token` 的 `grant_type=refresh_token`,拿新 access_token + refresh_token 回写
- 刷新节流:同一账户 5 分钟内不重复刷新(防止 launch 多次触发导致 OAuth 限流)

### 4.5 皮肤下载

不存皮肤文件、不内嵌到本地 — 头像 URL 用 UUID 合成公开镜像,前端 `<img src={url}>` 即用即取:

- 主:`https://crafatar.com/avatars/<uuid>?size=64&overlay`(crafatar,稳定)
- 备:`https://mc-heads.net/avatar/<uuid>/64`(mc-heads,备用镜像)

`get_account_skin_url(uuid)` 命令返回主 URL;前端降级逻辑(M3 留 hook,具体实现在 plan 层):crafatar 失败自动换 mc-heads。

### 4.6 UI 设计

路由 `/accounts` 替换 `src/pages/PlaceholderPage.tsx`(目前 `App.tsx:21` 指向 `kind="accounts"` 的占位页)。

页面布局(单页,无侧边分页):

```
┌──────────────────────────────────────────────────┐
│ 账户管理                              [＋ 添加]   │
├──────────────────────────────────────────────────┤
│ ● [头像] 玩家名      微软账户   2026-09-01 到期  │ ← 选中态蓝色描边
│              末 4 位: xxxx                       │
├──────────────────────────────────────────────────┤
│ ○ [头像] Player        离线       UUID: a01e38…  │
│              2026-09-01 添加                      │
└──────────────────────────────────────────────────┘
```

- **添加微软账户**流程:点击「＋ 添加」→ 选「微软登录」→ Rust 调 `start_microsoft_login` → 弹 Chakra Modal 显示 user_code + 「打开 microsoft.com/devicelogin」按钮(`tauri-plugin-opener` 已在 M1 加入,见 `src-tauri/Cargo.toml:22`)→ 前端用 `setInterval` 调 `poll_microsoft_login` → 成功 → 关闭 Modal + 刷新列表 + 设为 active
- **添加离线账户**流程:点击「＋ 添加」→ 选「离线账户」→ 弹 Modal 输入用户名(实时校验 + UUID 预览「派生 UUID: a01e…」)→ 确认 → `add_offline_account` → 关闭 + 刷新
- **切换账户**:点列表行任意位置 → `set_active_account` → 列表重新渲染(active 行有蓝色描边)
- **删除账户**:行右侧「…」菜单 → 确认 → `remove_account`(若是 active,则 active 清空;若列表空,允许「无账户启动」状态)
- **空状态**:首次进入「还没有账户,点 + 添加一个吧」+ 一个「添加离线账户」CTA 按钮

### 4.7 Tauri 命令清单(共 9 个,M3 全部新增)

| # | 命令签名 | 用途 |
|---|---|---|
| 1 | `list_accounts() -> Vec<Account>` | 列出所有已保存账户(原 PlaceholderPage stub 用) |
| 2 | `get_active_account() -> Option<Account>` | 取出当前选中账户;启动前用 |
| 3 | `add_offline_account(username: String) -> Result<Account, String>` | 离线账户添加(派生 UUID + 写盘 + 设为 active) |
| 4 | `remove_account(account_id: Uuid) -> Result<(), String>` | 删除账户(若是 active 一并清空) |
| 5 | `set_active_account(account_id: Uuid) -> Result<(), String>` | 切换当前账户(写回 active_account_id) |
| 6 | `start_microsoft_login() -> Result<DeviceCodeResponse, String>` | 调 device code 端点,返回 user_code / verification_uri / expires_in / interval |
| 7 | `poll_microsoft_login(device_code: String) -> Result<LoginStatus, String>` | 轮询 token 端点;`LoginStatus = Pending \| Success(Account) \| Declined \| Expired \| Failed(String)` |
| 8 | `refresh_microsoft_token(account_id: Uuid) -> Result<Account, String>` | 主动刷新 + 回写;5 分钟节流 |
| 9 | `get_account_skin_url(uuid: String) -> String` | 返回 crafatar URL(UUID 无连字符格式需先剥) |

累计命令数:M1(1) + M2(11) + M3(9) = **21 个**,在 `src-tauri/src/lib.rs:7-19` 的 `tauri::generate_handler!` 块追加注册。

### 4.8 与 launch.rs 的衔接

`launch_version` 签名扩展:`launch_version(version_id, java_path, account_id: Option<Uuid>)` —— 第三个参数从 M3 起**必填**(无账户禁止启动,前端在 HomePage 顶部加「请先在『账户』页添加账户」红色 Banner)。launch.rs:294-309 的硬编码删除,改为:

- `account_id == None` 或账户列表为空 → `Err("未选择账户,请先在账户页添加")`
- `Account::Offline { id, username, created_at }` → `auth_player_name = username`、`auth_uuid = id`、`auth_access_token = ""`、`user_type = "legacy"`
- `Account::Microsoft { uuid, player_name, access_token, xuid, .. }` → 先检查 `expires_at`,过期自动 `refresh_microsoft_token`;成功后用新鲜 token 填 `auth_access_token`、`user_type = "msa"`、`auth_xuid = xuid`

## [S3] Out of Scope

- 第三方 Yggdrasil 认证服务器(authlib-injector 注入) —— SJMCL 已实现,M4+ 考虑
- 微软账户头像/披风下载到本地 —— 始终走公开镜像 URL,M3 不缓存
- 多人游戏好友列表、服务器列表联动
- 账户云同步(目前只用本地 accounts.json)
- 第三方皮肤站(OptiFine、LabyMod、SkinnedCitizen 等)加载
- 启动器账户统计 / 在线时长显示
- 深链(URL Scheme 登录) —— 设备码流够用

## Tasks

- [ ] T-M3-1:账户存储层(`commands/account.rs` + `<game_dir>/accounts.json`,原子写 + 降级 load) — acceptance: `cargo test` 含 3 个(JSON 往返 / 缺文件降级 / 缺字段降级) (covers: 4.1)
- [ ] T-M3-2:离线账户 add / list / remove / set_active + UUID 复用 `uuid_offline_for` — acceptance: 4 个 tauri 命令签名 + 前端 hook + 用户名校验 (covers: 4.2 / 4.7-1~5)
- [ ] T-M3-3:微软 OAuth 设备码流(start + poll,标准 3 步 + 4 个错误码语义) — acceptance: 2 个 tauri 命令 + 单元测试模拟 4 个错误码响应 (covers: 4.3 / 4.7-6~7)
- [ ] T-M3-4:Token 刷新(自动 + 主动,5 分钟节流,Xbox Live + XSTS + Minecraft 兑换) — acceptance: 1 个 tauri 命令 + 1 个测试(过期判定) (covers: 4.4 / 4.7-8)
- [ ] T-M3-5:皮肤 URL 命令(无 IO,纯字符串合成) — acceptance: 1 个 tauri 命令 + 2 个测试(无连字符 / 有连字符 UUID) (covers: 4.5 / 4.7-9)
- [ ] T-M3-6:AccountsPage UI(列表 / Modal 两种添加流 / 切换 / 删除 / 空状态) + 路由替换 PlaceholderPage — acceptance: 列表渲染、2 个 Modal 流程、active 高亮、删除确认 (covers: 4.6)
- [ ] T-M3-7:launch.rs 衔接(增加 account_id 参数 + 离线 / 微软两路 auth 变量填法 + 过期自动刷新) — acceptance: 原 294-309 硬编码删除、`useVersionLaunch` hook 加 account 参数、HomePage 加「无账户」Banner (covers: 4.8)
- [ ] T-M3-8:单元测试覆盖(`cargo test` ≥ 40 passed)+ 手动 e2e(添加离线 / 微软两路跑通 26.2 启动) — acceptance: 全量测试通过、tauri dev 两条启动路径都验过 (covers: 全部)

## References

- 微软 OAuth 设备码流官方文档:<https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code>
- Minecraft Services API(Xbox Live + XSTS + Minecraft access token 兑换链路):<https://minecraft.wiki/w/Microsoft_account>
- SJMCL 账户模块源码(微软登录 + authlib-injector 双轨):<https://github.com/UNIkeEN/SJMCL/tree/main/src-tauri/src/account>
- HMCL `MicrosoftAuthenticator.java`(设备码流 + Xbox Live 兑换参考实现):<https://github.com/huanghongxun/HMCL/blob/master/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/MicrosoftAccount.java>
- HMCL 公开 client_id 来源讨论(社区约定值 `00000000402b2508`):<https://github.com/huanghongxun/HMCL/issues/1240>
- PCL2 账户系统(微软登录 + 离线模式):<https://github.com/PCL-Community/PCL2>
- Crafatar 皮肤公开镜像:<https://crafatar.com>(API:<https://github.com/Crafatar/crafatar>)
- mc-heads 备用头像镜像:<https://mc-heads.net>
- 离线 UUID 协议(Mojang `nameUUIDFromBytes`):<https://minecraft.wiki/w/UUID>

## Plan 索引

本 spec 的具体子任务拆分、代码块、TDD 步骤在 `plans/2026-09-XX-l?-xxx.md` 系列(每个 T-M3-N 对应一个 plan,创建日期为 plan 起始日):

- `plans/2026-09-01-l1-accounts-json-storage.md`(T-M3-1)
- `plans/2026-09-XX-l2-offline-account.md`(T-M3-2)
- `plans/2026-09-XX-l3-microsoft-device-code.md`(T-M3-3)
- `plans/2026-09-XX-l4-token-refresh.md`(T-M3-4)
- `plans/2026-09-XX-l5-skin-url.md`(T-M3-5)
- `plans/2026-09-XX-l6-accounts-page-ui.md`(T-M3-6)
- `plans/2026-09-XX-l7-launch-integration.md`(T-M3-7)
- `plans/2026-09-XX-l8-e2e.md`(T-M3-8)
