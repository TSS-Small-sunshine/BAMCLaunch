# L1:下载版本 JSON 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 前端点「下载」→ Rust 把该版本的 version JSON 落到 `.bamcl-dev/versions/<id>/<id>.json` → 按钮显示状态。

**Architecture:** 前端复用 M1 的"三态状态机"模式(新 hook `useVersionDownload`,每张版本卡独立状态);Rust 新增 `download_version_json` 命令(校验 id → reqwest 下载 → tokio::fs 写盘);共享 HTTP 客户端提取到 `commands/mod.rs` 供 M1/M2 命令复用。

**Tech Stack:** Tauri 2 / Rust(reqwest 0.13, tokio fs)/ React 19 / Chakra UI v2。

## Global Constraints

- **无自动化测试**(用户约定,覆盖 TDD):验证 = `npm run build` + `cargo check` + `npm run tauri dev` 手动验收。
- **提交信息必须 ASCII/英文**(PowerShell 5.1 会把中文变 `?` 字节);一任务一提交。
- git 本地身份:`TSS-Small-sunshine <small_sunshine@tssplus.top>`(仓库本地配置,已就位),顶格不动全局配置。
- 游戏目录:`<项目根>/.bamcl-dev/`(相对 cwd,开发期约定;`tauri dev` 的 cwd = 项目根)。
- `version_id` 会拼进文件路径 → 命令入口校验:拒绝含 `/` `\` `..` 的 id。
- 执行环境:win32 + PowerShell 5.1;Node 24.19.0 / npm 11.17.0;rustc/cargo 1.97.1。
- 不要在命令输出后接 `Select-Object -First N`(会杀掉长任务进程);`tauri dev` 残留的 Vite 进程占 1420 端口,重开前先清理。

---

### Task 1:写 M2 spec 与 L1 计划(本文档)

**Files:** Create `docs/compose/spec/m2-download-and-launch.md`(已写)、`docs/compose/plans/2026-08-18-l1-version-json-download.md`(本文档)

- [ ] **Step 1: 提交**

```bash
git add docs/compose/spec/m2-download-and-launch.md docs/compose/plans/2026-08-18-l1-version-json-download.md
git commit -m "docs: add M2 download-and-launch spec and L1 plan"
```

注:本文档是 Task 1 提交物的一部分(先有 spec/plan 再实现,符合 compose 流程)。

---

### Task 2:配置(.gitignore + Cargo.toml)

**Files:**
- Modify: `.gitignore`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: .gitignore 追加**

在 "# Agent 工具目录" 块后面追加上下文:

```gitignore
# Agent 工具目录
.mimocode/
.worktrees/

# 本地游戏目录(dev)
.bamcl-dev/
```

- [ ] **Step 2: Cargo.toml 追加 tokio**

`[dependencies]` 块加:

```toml
tokio = { version = "1", features = ["fs"] }
```

- [ ] **Step 3: 验证并提交**

`cargo check`(在 `src-tauri/` 下)——预期通过,仅新增依赖。
`npm run build`——预期通过(前端未动)。

```bash
git add .gitignore src-tauri/Cargo.toml
git commit -m "chore: add tokio fs dep and gitignore local game dir"
```

---

### Task 3:Rust 后端(download 命令 + 共享 http_client)

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/version.rs`(改用共享 client,只删不增)
- Create: `src-tauri/src/commands/download.rs`
- Modify: `src-tauri/src/lib.rs`(注册新命令)

**Interfaces:**
- Consumes: 现有 `reqwest`(features = ["json"])、`serde`、`tokio`(features = ["fs"],Task 2)
- Produces: `pub(crate) fn http_client() -> Result<reqwest::Client, String>`(commands/mod.rs);`#[tauri::command] pub async fn download_version_json(version_id: String, url: String) -> Result<String, String>`(download.rs)。前端经 `invoke("download_version_json", { versionId, url })` 调用(Tauri 参数 camelCase←snake_case 自动映射)。

- [ ] **Step 1: commands/mod.rs 换成共享 client 助手**

```rust
pub mod download;
pub mod version;

use reqwest::Client;

/// 统一超时与 UA 的 HTTP 客户端,所有联网命令共用(DRY)
pub(crate) fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("BAMCLaunch/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}
```

- [ ] **Step 2: version.rs 改用共享 client**

`fetch_version_manifest` 函数体开头替换(删掉手写 builder):

```rust
use super::http_client;

/// 拉取 Minecraft 版本清单。前端通过 invoke("fetch_version_manifest") 调用。
#[tauri::command]
pub async fn fetch_version_manifest() -> Result<VersionManifest, String> {
    let client = http_client()?;

    let manifest: VersionManifest = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("请求版本清单失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("解析版本清单失败: {e}"))?;

    Ok(manifest)
}
```

- [ ] **Step 3: 新建 download.rs**

```rust
use std::path::PathBuf;

use super::http_client;

/// 游戏根目录:开发期放项目根下便于查看;正式版应改用户目录(后续里程碑)
fn game_dir() -> PathBuf {
    PathBuf::from(".bamcl-dev")
}

/// 下载某个版本的 version JSON(说明书)到 .bamcl-dev/versions/<id>/<id>.json,
/// 返回保存路径。前端通过 invoke("download_version_json", { versionId, url }) 调用。
#[tauri::command]
pub async fn download_version_json(version_id: String, url: String) -> Result<String, String> {
    // 系统边界校验:version_id 会拼成文件路径,拒绝路径分隔符与 ..
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err("非法的版本标识".into());
    }

    let client = http_client()?;
    let bytes = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载版本信息失败: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("读取版本信息失败: {e}"))?;

    let dir = game_dir().join("versions").join(&version_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建目录失败: {e}"))?;

    let path = dir.join(format!("{version_id}.json"));
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(path.to_string_lossy().into_owned())
}
```

- [ ] **Step 4: lib.rs 注册命令**

```rust
.invoke_handler(tauri::generate_handler![
    commands::version::fetch_version_manifest,
    commands::download::download_version_json
])
```

- [ ] **Step 5: 验证并提交**

在 `src-tauri/` 下 `cargo check`——预期通过。

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/version.rs src-tauri/src/commands/download.rs src-tauri/src/lib.rs
git commit -m "feat(backend): add download_version_json command with shared http client"
```

---

### Task 4:前端(tauri.ts + useVersionDownload + 版本卡下载按钮)

**Files:**
- Modify: `src/lib/tauri.ts`
- Create: `src/hooks/useVersionDownload.ts`
- Modify: `src/pages/HomePage.tsx`

**Interfaces:**
- Consumes: `downloadVersionJson(versionId: string, url: string): Promise<string>`(tauri.ts);`useVersionDownload(versionId, url)` 返回 `{ state, download }`,`state` 为 `{status:"idle"|"downloading"|"done"|"error", ...}`。
- Produces: VersionCard 内自含下载按钮,无需向 HomePage 传状态。

- [ ] **Step 1: tauri.ts 追加调用封装**

```ts
import { invoke } from "@tauri-apps/api/core";
import type { VersionManifest } from "../types/version";

/** 调用 Rust 后端的 fetch_version_manifest 命令 */
export function fetchVersionManifest(): Promise<VersionManifest> {
  return invoke<VersionManifest>("fetch_version_manifest");
}

/** 把指定版本的 version JSON 下载到本地游戏目录,返回保存路径 */
export function downloadVersionJson(versionId: string, url: string): Promise<string> {
  return invoke<string>("download_version_json", { versionId, url });
}
```

- [ ] **Step 2: 新建 hooks/useVersionDownload.ts**

```ts
import { useCallback, useState } from "react";
import { downloadVersionJson } from "../lib/tauri";

/** 单版本下载状态:和 useVersionManifest 同款三态状态机 */
type DownloadState =
  | { status: "idle" }
  | { status: "downloading" }
  | { status: "done"; path: string }
  | { status: "error"; message: string };

export function useVersionDownload(versionId: string, url: string) {
  const [state, setState] = useState<DownloadState>({ status: "idle" });

  const download = useCallback(async () => {
    setState({ status: "downloading" });
    try {
      const path = await downloadVersionJson(versionId, url);
      setState({ status: "done", path });
    } catch (err) {
      setState({ status: "error", message: String(err) });
    }
  }, [versionId, url]);

  return { state, download };
}
```

- [ ] **Step 3: HomePage.tsx 的 VersionCard 加下载按钮**

顶部 import 追加 `DownloadIcon` 和 hook:

```tsx
import { DownloadIcon, RepeatIcon } from "@chakra-ui/icons";
import { useVersionDownload } from "../hooks/useVersionDownload";
```

`VersionCard` 函数体开头加 hook,并在「启动」按钮旁加「下载」按钮(替换原 Tooltip 所在块前后的 JSX):

```tsx
function VersionCard({
  version,
  isLatest,
}: {
  version: ManifestVersion;
  isLatest: boolean;
}) {
  const isRelease = version.type === "release";
  const { state, download } = useVersionDownload(version.id, version.url);
  // ...原有 icon 与信息 JSX 不动...
  return (
    <Flex align="center" gap={4} ...>
      {/* ... */}
      <Button
        size="sm"
        leftIcon={<DownloadIcon />}
        onClick={download}
        isLoading={state.status === "downloading"}
        colorScheme={state.status === "done" ? "grass" : undefined}
        variant={state.status === "done" ? "outline" : "solid"}
      >
        {state.status === "idle" && "下载"}
        {state.status === "downloading" && "下载中"}
        {state.status === "done" && "已下载"}
        {state.status === "error" && "重试"}
      </Button>
      <Tooltip label="启动功能将在后续里程碑实现" placement="top">
        <Box as="span">
          <Button size="sm" isDisabled>
            启动
          </Button>
        </Box>
      </Tooltip>
    </Flex>
  );
}
```

- [ ] **Step 4: 验证并提交**

`npm run build`(项目根)——预期通过。

```bash
git add src/lib/tauri.ts src/hooks/useVersionDownload.ts src/pages/HomePage.tsx
git commit -m "feat(frontend): add per-version download button with 3-state hook"
```

---

### Task 5:手动验收(tauri dev)

- [ ] **Step 1: 清理可能残留的 1420 端口进程**

```powershell
Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

- [ ] **Step 2: 启动**

`npm run tauri dev`(项目根,交互式;窗口打开后:**点任意版本卡上的「下载」**)

- [ ] **Step 3: 预期结果**

1. 按钮短暂变「下载中」→「已下载」
2. 项目根出现 `.bamcl-dev/versions/<id>/<id>.json`(用资源管理器或 `Get-ChildItem -Recurse .bamcl-dev` 确认)
3. 断网重试点「重试」能复现错误状态(可选)

## Self-Review(执行前已核对)

- S2/L1 全部契约均有对应任务(Task 3 + Task 4 覆盖:命令签名、错误、安全校验、共享 client、前端状态机)✓
- 无占位符;所有代码块完整 ✓
- 命名一致:`download_version_json`(Rust)↔ `downloadVersionJson`(TS)↔ `{ versionId, url }`(invoke 参数)↔ `useVersionDownload`(hook)✓