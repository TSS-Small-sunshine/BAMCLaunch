---
feature: m2-download-and-launch
status: in-progress
updated: 2026-08-18
branch: main
commits: <base-sha>..<head-sha> # filled at delivery
---

# BAMCLaunch 里程碑 2:版本下载与游戏启动

## Report

## [S1] Problem

M1(Mojang 版本清单)已上线,但用户只能"看列表",不能下载、不能启动。M2 的目标是打通"下载 → 启动"链路,并保持教学优先:每一步拆成独立小课(L1~L6),每课一个可验证的小功能。

核心概念(教学主线):**Minecraft 启动器 = 读说明书(version JSON)→ 买齐物料(下载 jar/libraries/assets)→ 组装(拼参数)→ 启动(交给 Java)**。

## [S2] Design

### 游戏目录(L1 决定)

- 开发期根目录:`<项目根>/.bamcl-dev/`(方便查看文件;需 gitignore)
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

- [ ] T-L1: 下载版本 JSON 全链路(设计见 [S2]/L1)—— acceptance: `npm run build` 与 `cargo check` 通过;`tauri dev` 中点「下载」后 `.bamcl-dev/versions/<id>/<id>.json` 真实落盘,按钮状态正确切换 (covers: S2)
- [ ] T-L2: client.jar 下载 + sha1 校验 (covers: S2)
- [ ] T-L3: assets 资源下载 (covers: S2)
- [ ] T-L4: libraries + natives (covers: S2)
- [ ] T-L5: Java 发现 (covers: S2)
- [ ] T-L6: 启动参数 + 进程拉起(离线) (covers: S2)