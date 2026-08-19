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

**L3:assets 资源下载(2026-08-18 完成)**

- 后端:`download_version_assets(version_id)` 单命令全链路——读本地说明书 → 取 assetIndex{id,sha1,url} → 下载 index(sha1 校验)→ 解析 objects 清单 → classify 缺失/已有 → **并发 8** 下载(`tokio::task::JoinSet` 分块)→ 每文件 sha1 校验,失败不写盘 → 返回 `AssetsSummary{total, downloaded, skipped}`
- 内容寻址:`objects/<sha1 前两位>/<完整 sha1>` + CDN `https://resources.download.minecraft.net/<前两位>/<hash>`;纯函数 `asset_object_path` / `asset_download_url` / `classify_objects` 均 TDD
- 前端:每卡「资源」按钮(useVersionAssets 同款状态机),完成 Tooltip 显示统计"新增 N/共 M, 跳过 K",错误 Tooltip 显示详情

**Verification(L3)** — `cargo test` 8 passed(新增 4:assetIndex 解析 / 内容寻址路径 / CDN URL / 跳过已有分类)/ `cargo check` ok / `npm run build` ✓ / `tauri dev` 手动:点 26.2「资源」→ `assets/indexes/32.json` 586,366 B 落盘 + `assets/objects/` 5057 文件、总计 479,185,985 B —— 与 index 声明 5057 及官方 totalSize 完全一致(全量通过 sha1 校验才落盘)

**Journey log(L3)** — serde 字段映射:JSON 的 `assetIndex`(camelCase)不会自动对应 Rust 的 `asset_index`,需 `#[serde(rename = "assetIndex")]`(缺省会静默 None,测试首次失败即此因);`JoinSet::spawn` 要求 'static,分块迭代时强闭包引用 chunk 会报 E0597 → 任务内 clone 所有捕获值;真实 26.2 index 声明 5057 对象 / 479,185,985 B(教学预估 6000+ 是错的,以真实数据为准)。

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

### L3:assets 资源下载(设计,2026-08-18)

Minecraft 的 assets = 素材库(音效、语言、字体、图标…),同一个仓库被所有版本共享。assets 总量远大于 jar:26.2 全量约 457MB / 6000+ 文件,而 client.jar 仅 39MB。

教学点 1 — **内容寻址**:资产文件"不按名字、按内容指纹"存储——文件名 = sha1,目录 = `objects/<sha1前两位>/<完整sha1>`。天然防损坏(名字即校验值),全版本共享一个仓库,重复文件自动去重。

教学点 2 — **清单 + 逐件购买**:版本说明书里 `assetIndex` 字段指向一个"物料清单" JSON(index),里面列了每个资产的名字/hash/size。下载 = 拿清单 → 逐项比对本地 → 缺失才下载。已存在的直接跳过(增量)。

教学点 3 — **并发**:6000 个文件串行下载不可接受 → 并发池(限 8)批量拉取。

数据流:

```
VersionCard「资源」按钮(useVersionAssets hook)
  → invoke("download_version_assets", { versionId })          # 只传 id,Rust 全包
  → Rust:读本地 <id>.json → 取 assetIndex{id,sha1,url}
         → 下载 index JSON(sha1 校验,复用 verify_sha1)→ 写 assets/indexes/<id>.json
         → serde 解析 objects{<相对路径>: {hash, size}}
         → 遍历清单:objects/<前两位>/<hash> 已存在 → skip;缺失 → 并发 8 下载
         → 每个文件下载后 sha1 比对,不一致 Err 且不写盘(沿用 L2 脏数据不落地)
  → 返回 { total, downloaded, skipped } → 前端按钮状态机 + Tooltip 统计
```

契约:

- `download_version_assets(version_id: String) -> Result<AssetsSummary, String>`,其中 `AssetsSummary { total: usize, downloaded: usize, skipped: usize }`(serde Serialize 直传前端)
- 复用:http_client / game_dir / id 校验 / sha1_hex / verify_sha1
- 本地未先下版本信息 → Err "请先下载该版本的版本信息"
- 资产下载地址规则:Mojang 资源 CDN `https://resources.download.minecraft.net/<hash前两位>/<hash>`
- 并发 8(state 共享计数,进度可观测);任一文件校验失败 → Err(不写盘该文件)

布局(教学对照真实启动器):

```
.bamcl-dev/assets/
├── indexes/<asset_index_id>.json   # 物料清单,如 indices/32.json(26.2)
└── objects/<hash前两位>/<hash>      # 内容寻址对象仓库
```

UI:每卡「资源」按钮与「客户端」并列,同款三态状态机,完成 Tooltip 显示"新增 N/M,跳过 K";错误 Tooltip 显示具体原因。

### L4:libraries 下载 + natives 解压(设计,2026-08-19)

Minecraft 的 libraries = 第三方运行库(压缩 lz4、OpenGL 绑定 LWJGL、原生 .dll…),全部由 Mojang 库托管(`libraries.minecraft.net`)。26.2 实查:131 项、约 114MB。

教学点 1 — **rules 过滤**:不是所有库里全都要。每项可带 `rules`(数组),26.2 实测全部是 `[{action:"allow", os:{name:"windows"|"linux"|"osx"}}]` 形式(无 arch/disallow 维度)。游戏要跑在哪个平台就只下哪个平台的库,否则把 4 个平台的 native 全拉下来。**现代格式没有老式的 `natives`/`classifiers` 字段** —— natives 就是独立条目 + os 规则过滤(如 `org.lwjgl:lwjgl-glfw:3.4.1:natives-windows`)。

教学点 2 — **natives 解压**:名字含 `natives-windows` 的 jar 是 zip 格式,里面装的是 `.dll`。启动时 Java 要从目录加载这些原生库 → 解压到 `versions/<id>/natives/`。教学点:**zip 路径穿越防护**——解压每个条目前校验目标路径必须仍位于 natives 目录内(拒绝 `..` / 绝对路径),这是真实启动器也会防的任意文件写漏洞。

数据流:

```
VersionCard「库」按钮(useVersionLibraries hook)
  → invoke("download_version_libraries", { versionId })        # 只传 id,Rust 全包
  → Rust:读本地 <id>.json → 解析 libraries[]
         → rules 按当前 OS 过滤(windows)→ 得到该下的库清单
         → 遍历:libraries/<path> 已存在 → skip;缺失 → 并发 8 下载(libraries.minecraft.net/<path>)
         → 每个 jar sha1 校验,不一致 Err 且不写盘(沿用 L2 脏数据不落地)
         → natives 条目额外步骤:解压 zip 到 versions/<id>/natives/(路径穿越防护)
  → 返回 { total, downloaded, skipped, natives } → 前端按钮状态机 + Tooltip 统计
```

契约:

- `download_version_libraries(version_id: String) -> Result<LibrariesSummary, String>`,其中 `LibrariesSummary { total, downloaded, skipped, natives }`(皆 usize,serde Serialize 直传前端)
- 复用:http_client / game_dir / id 校验 / sha1_hex / verify_sha1 / 并发 8 JoinSet 模式
- 纯逻辑 TDD:
  - `fn library_allowed(rules: &[Rule], os_name: &str) -> bool` — 无 rules 默认允许;有 rules 按最后一条匹配规则定夺(`allow`→true);os 不匹配视为不匹配
  - `fn is_native_library(name: &str) -> bool` — 名字含 `natives-<os>` 标记(教学实现用 `natives-windows`,可参数化)
  - `fn safe_entry_path(natives_dir: &Path, entry: &str) -> Option<PathBuf>` — 拒绝绝对路径与 `..`
- 新增依赖:`zip` crate(解压 natives jar)
- 本地未先下版本信息 → Err "请先下载该版本的版本信息"

布局(教学对照真实启动器):

```
.bamcl-dev/libraries/<path>            # path 即说明书里的 artifact.path(含三级坐标)
.bamcl-dev/versions/<id>/natives/      # 解压后的 .dll 等原生库
```

UI:每卡「库」按钮与「资源」并列,同款三态状态机,完成 Tooltip 显示"库 N/M,跳过 K,原生库 X";错误 Tooltip 显示具体原因。

### 后续课(概要,逐课细化)

- L5:Java 自动发现(版本适配 + 路径探测)
- L6:启动参数拼接 + 进程拉起(离线模式)→ 游戏真跑起来

## [S3] Out of Scope

- 除 L1 之外的 M2 内容(见后续课)
- 微软账户登录、Mod/整合包、设置持久化(M3/M4)
- 自动化测试(沿用 M1 约定:验证 = `npm run build` + `cargo check` + `tauri dev` 手动)

## Tasks

- [x] T-L1: 下载版本 JSON 全链路(设计见 [S2]/L1)—— acceptance: `npm run build` 与 `cargo check` 通过;`tauri dev` 中点「下载」后 `.bamcl-dev/versions/<id>/<id>.json` 真实落盘,按钮状态正确切换 (covers: S2)
- [x] T-L2: client.jar 下载 + sha1 校验 —— acceptance: `cargo test` 全绿(sha1/verify/解析 3 测试);`npm run build` 通过;`tauri dev` 点「客户端」后 `versions/<id>/client.jar` 落盘(≈37MB 真实数据),按钮状态正确;篡改说明书中的 sha1 后点下载应报"校验失败" (covers: S2)
- [x] T-L3: assets 资源下载 —— acceptance: `cargo test` 全绿(assetIndex 解析 / 内容寻址路径 / verify 复用);`npm run build` 通过;`tauri dev` 点「资源」后 `assets/indexes/32.json` 落盘 + 首次全量下载 5057 文件(479,185,985 B),再点一次 skipped 全量(增量验证);篡改 index 中某文件 hash 后应报错 (covers: S2)
- [ ] T-L4: libraries 下载 + natives 解压(rules 过滤 / sha1 校验 / 并发 8 / 路径穿越防护)—— acceptance: `cargo test` 全绿(rules 过滤 / native 识别 / 安全路径 3 测试);`npm run build` 通过;`tauri dev` 点「库」后 `libraries/` 落盘 Windows 所需 jar(~110MB)+ `versions/26.2/natives/` 解压出 .dll;再点一次 skipped 全量;篡改某 jar sha1 后应报错 (covers: S2)
- [ ] T-L5: Java 发现 (covers: S2)
- [ ] T-L6: 启动参数 + 进程拉起(离线) (covers: S2)