---
feature: m2-download-and-launch
status: in-progress
updated: 2026-08-18
branch: main
commits: <base-sha>..<head-sha> # filled at delivery
---

# BAMCLaunch 里程碑 2:版本下载与游戏启动

## Report

**L1:下载版本 JSON(2026-08-18 完成)**

- 前端:每张版本卡新增「下载」按钮(`useVersionDownload` 状态机 idle/downloading/done/error),完成后按钮 Tooltip 显示保存路径
- 后端:新增 `download_version_json` 命令;HTTP 客户端提取为共享 `http_client()`(DRY,M1 的 version.rs 一并复用)
- 游戏目录 = 便携模式(exe 旁 `.bamcl-dev`),回归测试 `game_dir_is_anchored_to_executable` 锁定行为(`cargo test` 首个测试)

**Verification** — `cargo test` 1 passed / `cargo check` ok / `npm run build` ✓(前端与 Rust 均通过);`tauri dev` 手动验收:点击下载→文件落在 exe 同目录(开发期 `target/debug/.bamcl-dev/versions/<id>/<id>.json`)→Tooltip 显示绝对路径

**Journey log** — 相对路径 bug(2026-08-18):`tauri dev` 下 Rust 进程 cwd=`src-tauri/`,`.bamcl-dev` 落到 src-tauri 下而非项目根;用户提出"便携版在 exe 目录生成 .minecraft"的正确观察 → 改用 `current_exe()` 锚定 + TDD 红绿循环写回归测试。经验:持久化路径绝不依赖进程 cwd。

**L2:下载 client.jar + sha1 校验(2026-08-18 完成)**

- 后端:`download_version_jar(version_id)` 一步到位——读本地说明书 → serde 解析 `downloads.client{sha1,url}` → 下载 jar → `sha1_hex` 与官方指纹比对 → 不一致报错**不写盘**,一致落盘 `versions/<id>/client.jar`
- 依赖 `sha1 = "0.10"`;纯函数 `sha1_hex` / `verify_sha1` 均 TDD 先行
- 前端:每卡「客户端」按钮(`useVersionJar` 同款状态机),与「下载」并列;完成 Tooltip 显示路径

**Verification(L2)** — `cargo test` 4 passed(sha1 官方向量 / verify 匹配与不匹配 / JSON 解析)/ `cargo check` ok / `npm run build` ✓ / `tauri dev` 手动:26.2 client.jar 落盘且大小 39,193,383 与说明书一致

**Journey log(L2)** — serde 默认忽略未知字段:结构体只声明用到的字段,JSON 里多余字段(如 size/server)不声明也不会报错,保持解析最小化;`sha1` crate 的 `format!("{:x}", digest)` 直接输出 hex,无需额外 hex 库。

## [S1] Problem

M1(Mojang 版本清单)已上线,但用户只能"看列表",不能下载、不能启动。M2 的目标是打通"下载 → 启动"链路,并保持教学优先:每一步拆成独立小课(L1~L6),每课一个可验证的小功能。

核心概念(教学主线):**Minecraft 启动器 = 读说明书(version JSON)→ 买齐物料(下载 jar/libraries/assets)→ 组装(拼参数)→ 启动(交给 Java)**。

## [S2] Design

### 游戏目录(L1 决定,2026-08-18 修正为便携模式)

- **便携模式**:游戏数据放在**可执行文件(exe)所在目录**下的 `.bamcl-dev/`(PCL/HMCL 便携版同思路),与当前工作目录无关
  - 开发期:`src-tauri/target/debug/.bamcl-dev`(exe 在 target/debug 下)
  - 发布后:exe 旁边自动出现 `.bamcl-dev`,整个文件夹可拷贝带走
  - 修正原因:`tauri dev` 启动时 Rust 进程 cwd=`src-tauri/`,相对路径落到非预期位置;打包后 cwd 更不可控。已加回归测试 `game_dir_is_anchored_to_executable`
- 布局镜像真实启动器(`.minecraft` 结构):
  ```
  .bamcl-dev/
  └── versions/
      └── <version-id>/
          └── <version-id>.json      # L1:L2 起还会有 client.jar 等
  ```

### L1:下载版本 JSON(本里程碑当前进行中的课)

数据流:

```
React(VersionCard 内 useVersionDownload hook)
  → invoke("download_version_json", { versionId, url })   # url 来自 manifest 条目
  → Rust 命令:校验 id → reqwest GET url → tokio::fs 建目录 + 写盘
  → 返回保存路径(字符串)→ 前端按钮状态机:下载中 / 已下载 / 失败(重试)
```

契约:

- `download_version_json(version_id: String, url: String) -> Result<String, String>`
- 错误(系统边界):网络失败 / 写盘失败 → 错误字符串 → 前端展示"重试"
- 安全:version_id 会拼进文件路径,拒绝包含 `\` `/` `..` 的 id
- 共享 HTTP 客户端:统一超时 20s + UA,提取到 `commands/mod.rs` 的 `pub(crate) fn http_client()`,M1 的 `version.rs` 也改用它(DRY)

前端交互:

- 每张版本卡新增「下载」按钮(图标 DownloadIcon),「启动」按钮保持禁用(M2-L6 再做)
- 按钮文案/状态:下载 → 下载中…(loading) → 已下载(绿色 outline)/ 重试(error)

### L2:下载 client.jar + sha1 完整性校验(设计,2026-08-18)

数据流:

```
VersionCard「客户端」按钮(useVersionJar hook)
  → invoke("download_version_jar", { versionId })            # 只传 id,Rust 全包
  → Rust:读本地 <id>.json → serde 解析 downloads.client{sha1,size,url}
         → reqwest 下载 jar → sha1_hex 本地指纹与官方比对
         → 一致:写 versions/<id>/client.jar;不一致:Err 且不写盘
```

契约:

- `download_version_jar(version_id: String) -> Result<String, String>`(复用 http_client / game_dir / id 校验)
- 本地未先下版本信息 → Err "请先下载该版本的版本信息"
- 纯逻辑:`fn sha1_hex(&[u8]) -> String`、`fn verify_sha1(&[u8], &str) -> bool`(大小写不敏感) — TDD 已测(官方向量 SHA1("abc")=a9993e36…)
- UI:每卡「客户端」按钮与「下载」并列,同款三态状态机,完成后 Tooltip 显示路径

### 后续课(概要,逐课细化)

- L2:下载 client.jar + sha1 完整性校验
- L3:assets(asset index + 资源批量下载)
- L4:libraries 解析 + natives 下载与解压
- L5:Java 自动发现(版本适配 + 路径探测)
- L6:启动参数拼接 + 进程拉起(离线模式)→ 游戏真跑起来

## [S3] Out of Scope

- 除 L1 之外的 M2 内容(见后续课)
- 微软账户登录、Mod/整合包、设置持久化(M3/M4)
- 自动化测试(沿用 M1 约定:验证 = `npm run build` + `cargo check` + `tauri dev` 手动)

## Tasks

- [x] T-L1: 下载版本 JSON 全链路(设计见 [S2]/L1)—— acceptance: `npm run build` 与 `cargo check` 通过;`tauri dev` 中点「下载」后 `.bamcl-dev/versions/<id>/<id>.json` 真实落盘,按钮状态正确切换 (covers: S2)
- [x] T-L2: client.jar 下载 + sha1 校验 —— acceptance: `cargo test` 全绿(sha1/verify/解析 3 测试);`npm run build` 通过;`tauri dev` 点「客户端」后 `versions/<id>/client.jar` 落盘(≈37MB 真实数据),按钮状态正确;篡改说明书中的 sha1 后点下载应报"校验失败" (covers: S2)
- [ ] T-L3: assets 资源下载 (covers: S2)
- [ ] T-L4: libraries + natives (covers: S2)
- [ ] T-L5: Java 发现 (covers: S2)
- [ ] T-L6: 启动参数 + 进程拉起(离线) (covers: S2)