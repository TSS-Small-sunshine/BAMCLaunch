---
feature: m2-download-and-launch
status: in-progress
updated: 2026-08-19
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

**L4:libraries 下载 + natives 解压(2026-08-19 完成)**

- 后端:`download_version_libraries(version_id)` 单命令全链路——读本地说明书 → rules 按当前 OS 过滤(windows)→ classify 缺失/已有 → **并发 8** 下载(`libraries.minecraft.net/<path>`)→ 每 jar sha1 校验,失败不写盘 → natives 条目(zip)解压到 `versions/<id>/natives/` → 返回 `LibrariesSummary{total, downloaded, skipped, natives}`
- rules 语义与官方启动器一致:无 rules 默认允许,有 rules 时最后一条匹配的规则定夺;26.2 实测 131 库中 Windows 需要 88 个(~86MB)
- 教学点:**胖 jar 裁剪**——LWJGL 3.4.1 实测一个 natives jar 同时装 x64/x86/arm64 三套 dll + META-INF 元数据,`entry_allowed_for_arch` 只解本机架构(arch 名映射 `lwjgl_arch()`),META-INF 整体跳过、平铺 jar 原样保留
- 前端:每卡「库」按钮(useVersionLibraries 同款状态机),完成 Tooltip 显示"库 N/M,跳过 K,原生库 X"

**Verification(L4)** — `cargo test` 16 passed(新增 8:rules 过滤×3 / native 识别 / 安全路径 / 架构裁剪×3)/ `cargo check` ok / `npm run build` ✓ / `tauri dev` 手动:点 26.2「库」→ `libraries/` 88 个 jar、85.8MB 落盘 + `versions/26.2/natives/` 只含 x64 架构 11 个 dll + 平铺 jtracy(无 arm64/x86/META-INF);修复前实测三套 dll 全解 + 57 个 META-INF 文件,删目录重下后验证只 x64

**Journey log(L4)** — `ZipFile` 实现了非 Send 的 `dyn Read`,直接放进 async 任务会编译期报错(E0277 future 不 Send)→ 正确姿势是 `spawn_blocking` 丢进阻塞线程池(zip 解压是 CPU 密集工作,也不该占异步运行时);LWJGL 3.4 的 natives jar 是"胖 jar"(一包三架构),只在真实 26.2 数据上才暴露——教学预估"1 套 dll"是错的,以实测定 3 套为准。

**L5:Java 发现(2026-08-28 完成)**

- 后端:新增 `scan_java_installations(version_id)` 命令(#[tauri::command]),内部从4 个来源(JAVA_HOME / PATH / Windows 常见路径 / Windows 注册表)收候选 → 同 path 多源去重(优先级 JAVA_HOME > PATH > CommonDir > Registry,Windows 路径 ASCII 小写归一化)→ 逐个 spawn `java -version` 探活取真实主版本(支持 JDK 9+ `version "25"` 现代格式 + JDK 8 `version "1.8.0_412"` 旧格式特判)→ 标记 `meets_requirement = (candidate.version >= required_major)`
- 纯逻辑 TDD 共 18 个测试(已全部 passed,1 ignored 为注册表 smoke test):
  - `parse_java_version`: modern / legacy(1.8.0_412 → 8)/ garbage(None,空字符串、空 version、非数字)
  - `meets_requirement`: 6 个边界(等号、超配、低配、JDK 1 起点)
  - `dedupe_candidates`: 同 path 优先级保留、不同 path 不去重、Windows 大小写归一化
  - `parse_env_paths`: JAVA_HOME 优先 / 无 JAVA_HOME / 全空 / 空 JAVA_HOME 字符串
  - `looks_like_jdk_dir`: jdk- / zulu / temurin- / openjdk- 前缀识别
  - `discover_from_common_dirs`: temp_dir fixture 真实 IO + 不存在目录跳过
  - `format_registry_java_home`: 纯函数单元测
  - `read_required_major_from_version_json`: 路径穿越防护(拒 `../` `/` `\` `..`)
- 新增依赖:`winreg = "0.52"`(Windows 注册表读取,`cfg(windows)` 隔离);`tokio` 加 `process` feature;`download::game_dir` 改 `pub(crate)` 跨模块共用
- 前端:新增 `useVersionJava` hook(状态机 idle / scanning / done / error,带 reset) + VersionCard 新增 [Java] 按钮 + Chakra Modal 渲染(按 `meets_requirement` 分两组:满足项置顶 + 绿色 v 标 + 来源 Badge;不满足项灰显 + 来源 Badge;无候选时显示提示文案)

**Verification(L5)** — `cargo test --lib` = **34 passed / 0 failed / 2 ignored**;`npm run build` ✓;e2e smoke test(`cargo test --lib -- --ignored e2e_scan_26_2`)真实跑通:本机发现 4 个 Java(JDK 17/21/25 来自注册表 + JDK 25 来自 PATH `Oracle\Java\javapath\`),其中 2 个满足 26.2 的 Java 25+ 要求。

**Journey log(L5)** — `std::env::path_separator()` 不存在,正确做法是 `cfg!(windows)` 字面常量 `;` / `:`;Windows 上 `PathBuf::eq` **不**自动大小写不敏感(NTFS 大小写不敏感但 std path 比较走精确字节),必须显式 `to_ascii_lowercase()` 作归一化 key,否则 dedupe 失效(教学点:不要假设 OS 文件系统大小写规则 = Rust API 行为);`tokio::process::Command` 默认 feature 不含,必须 `features = ["process"]`;cargo test 下 `current_exe()` 锚到 `target/debug/deps/`,不是 `target/debug/`,集成测试若依赖 `game_dir()` 找版本说明书要绕开(本课 e2e 测试改用 `current_exe().parent().parent()` 显式回退一层);「尽力扫描」语义体现在每来源错误 swallow 继续(注册表无权限、目录不存在、java 不可执行)→ 不阻断整体。

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

教学点 2 — **natives 解压**:名字含 `natives-windows` 的 jar 是 zip 格式,里面装的是 `.dll`。启动时 Java 要从目录加载这些原生库 → 解压到 `versions/<id>/natives/`。教学点:**zip 路径穿越防护**——解压每个条目前校验目标路径必须仍位于 natives 目录内(拒绝 `..` / 绝对路径),这是真实启动器也会防的任意文件写漏洞。教学点 3:**胖 jar 裁剪**——LWJGL 3.4.1 实测一个 natives jar 同时装 x64/x86/arm64 三套 dll(另有 META-INF 的 .sha1/.git 元数据),只解本机架构那套(`entry_allowed_for_arch`,arch 名映射 `lwjgl_arch()`,Rust `x86_64`→LWJGL `x64`);平铺 jar(如 jtracy)原样保留。META-INF 一律跳过。

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
  - `fn entry_allowed_for_arch(entry: &str, arch: &str) -> bool` — 胖 jar 裁剪:META-INF 跳过 / 只解本机架构
  - `fn lwjgl_arch() -> &'static str` — Rust 架构名 → LWJGL fat-jar 目录名
- 新增依赖:`zip` crate(解压 natives jar)
- 本地未先下版本信息 → Err "请先下载该版本的版本信息"

布局(教学对照真实启动器):

```
.bamcl-dev/libraries/<path>            # path 即说明书里的 artifact.path(含三级坐标)
.bamcl-dev/versions/<id>/natives/      # 解压后的 .dll 等原生库
```

UI:每卡「库」按钮与「资源」并列,同款三态状态机,完成 Tooltip 显示"库 N/M,跳过 K,原生库 X";错误 Tooltip 显示具体原因。

### L5:Java 发现(设计,2026-08-27)

Minecraft 是 Java 程序,启动器必须自己找一个**匹配说明书要求版本**的 java.exe。原因有三:
1. PATH 里那个 java 不一定够新 —— 26.2 要求 Java 25(2025-09 才 GA),玩家可能只装了 17 给别的游戏
2. 用户可能装了多个 Java(老游戏用 8,新游戏用 21,MC 26.2 要 25)—— 启动器需要识别并展示候选
3. 缺模块会爆 `UnsupportedClassVersionError` —— 26.2 还用 `java-runtime-epsilon` 这种 Java 9+ 才有的模块化 JRE 概念

**教学点 1 — 读说明书拿需求**:从 `<id>.json` 顶层 `javaVersion: {component: "java-runtime-epsilon", majorVersion: 25}` 读出最小主版本号(整数 25)。这就是**适配判定**的输入。

**教学点 2 — 多源扫描,合成候选列表**(任一来源都可能有,也都没有——给玩家一个列表让 ta 自己挑):

| 源 | 怎么找 | 适用平台 |
|---|---|---|
| `JAVA_HOME` 环境变量 | 读 env → `<JAVA_HOME>/bin/java.exe` | 全平台 |
| `PATH` 解析 | split `PathSep` → 每个目录看有没有 `java.exe` | 全平台 |
| Windows 常见安装路径 | glob `C:\Program Files\Java\jdk-*`、`C:\Program Files\Eclipse Adoptium\jdk-*`、`C:\Program Files\Microsoft\jdk-*`、`C:\Program Files\Zulu\zulu-*` | 仅 Windows |
| Windows 注册表 `HKLM\SOFTWARE\JavaSoft\JDK\<version>` | `winreg` crate 读 `JavaHome` 值 → 拼 `bin\java.exe` | 仅 Windows |

**教学点 3 — 探活取真实版本**:对每个候选路径 spawn `java -version`(走 stdout/stderr,JDK 11+ 走 stderr,旧版走 stdout,**都要收**)。解析首行:
```
openjdk version "25" 2025-09-16          → 25
openjdk version "1.8.0_412" 2025-08-19   → 8 (旧版「1.x」格式要特判)
```
正则 `version "(\d+)(?:\.(\d+))?(?:\.(\d+))?"` —— 第一个捕获组即主版本。旧版 `1.8.0` 第二组才是真主版本(`1.8` → 8),特判。

**教学点 4 — 版本适配判定**:每个候选打标 `meets_requirement = candidate.version >= required.major_version`。前端按是否适配分组呈现(适配的优先显示)。

数据流:

```
[扫Java]按钮 (新) → invoke("scan_java_installations")
  → Rust:遍历四个来源 → 去重(同 path 只算一次)→ spawn java -version 取版本号
     → 返回 Vec<JavaCandidate { path, version, source, meets_requirement }>

[版本卡]读取 javaVersion.majorVersion → 调 scan → 把结果按适配/不适配分组渲染
  (L5 仅展示,不启动;L6 才用选定 candidate 的 path 真去 spawn 游戏)
```

契约:

- `scan_java_installations() -> Result<JavaScanResult, String>`,其中 `JavaScanResult { required_major: u32, candidates: Vec<JavaCandidate> }`(serde Serialize)
- `JavaCandidate { path: String, version: u32, source: JavaSource, meets_requirement: bool }`
- `JavaSource` 枚举:`JAVA_HOME | PATH | CommonDir | Registry`,Serialize as snake_case
- 任何来源抛错不阻断整体(如注册表无权限 → 该来源返空,继续下一个);所有来源收集完合并去重
- 同 path 多个来源命中只保留先到的(优先级 `JAVA_HOME > PATH > CommonDir > Registry`)
- 候选文件不存在或 spawn 失败 → 跳过该候选,不报错(教学点:扫描天然是「尽力」语义)
- 路径规范化为绝对路径后比较,避免 `C:\...\bin\java.exe` 与 `./bin/java.exe` 重复
- 已知 Java 路径 glob 用 `std::fs::read_dir` 递归遍历 + 文件名 pattern 匹配,不引入额外依赖(教学:能 std 解决就别加 crate)

UI:版本卡新增「Java」按钮(与「下载」「客户端」「资源」「库」并列),同款三态状态机。点击 → 调 scan → 弹 Chakra Modal 列候选(适配项置顶 + 绿色✓图标,源标 JAVA_HOME/PATH/CommonDir/Registry);选中项 Tooltip 显示「✓ Java 25 · JAVA_HOME」之类的精简信息;未找到任何 Java → Modal 显示「未检测到 Java 安装,请安装 Java 25+ 或在设置中手动指定路径」(设置持久化在 M3)

布局:L5 不写新文件 —— 只读取系统。Java 安装本身不受 BAMCLaunch 管理(玩家自己装)。

Out of scope:

- **手动下载 Java**(Java 自下载是另一课,L5 不做)
- **设置页手动指定 Java 路径**(M3)
- **真启动游戏**(L6 才把 JavaCandidate.path 喂给 tokio::process::Command)
- **macOS/Linux 特定扫描路径**(L5 仅做 Windows;跨平台细化等真上 macOS 再补)

纯逻辑 TDD(预计新增 ~8 个测试):

- `fn parse_java_version(stdout_stderr: &str) -> Option<u32>` —— 正则解析"version \"N\""
- `fn parse_java_version("version \"1.8.0_412\"") == Some(8)` —— 旧版「1.x」特判
- `fn meets_requirement(candidate_version: u32, required_major: u32) -> bool` —— 简单比较
- `fn dedupe_candidates(candidates: Vec<JavaCandidate>) -> Vec<JavaCandidate>` —— 同 path 只留一个,优先级决定保留谁
- `fn discover_from_env() -> Vec<PathBuf>` —— 读 JAVA_HOME + PATH
- `fn discover_from_common_dirs() -> Vec<PathBuf>` —— 扫常见安装路径
- `fn discover_from_registry() -> Vec<PathBuf>` —— 读注册表(Windows only,单元测试可用 mock)

新增依赖:`winreg = "0.52"`(Windows 注册表读写,Linux/macOS 上此模块不可用,L5 用 cfg(windows) 隔离,其他平台该函数返回空 Vec)

### L6:启动参数拼接 + 进程拉起(设计,2026-08-29)

L5 给出候选 Java 路径,L6 用它 + 说明书 + 物料拼出**完整 java 命令**并 spawn 游戏进程。M2 的最后一课 — 至此**读说明书 → 买齐物料 → 组装 → 启动**全链路打通。

**教学点 1 — Classpath 拼装**:把所有 .jar 文件路径用平台分隔符串起来,作为 `-cp` 参数值:
```
# Windows 用 `;` 分隔,Linux/macOS 用 `:`
-cp "versions/<id>/client.jar;libraries/at/yawk/lz4/lz4-java/1.10.1/lz4-java-1.10.1.jar;..."
```
顺序无所谓(Minecraft 自己从 manifest 找主类),但必须**全部** — 缺一个 native 就崩 `NoClassDefFoundError`。L4 已按当前平台 rules 下载,这里**遍历 `libraries/` 目录拿到所有 jar** 即可(rules 过滤已经在下载阶段做完)。

**教学点 2 — `arguments.jvm[]` / `arguments.game[]` 是混合数组**:每项要么是纯字符串(`"-Xmx4G"`、`"--username"`),要么是带 `rules` 的对象(只有当前平台/feature 满足才加)。语义:
- **无 rules → 永远加入**
- **有 rules → 最后一条匹配的规则定夺**(同 libraries.rules)

rules 过滤维度有三类:
- `os.name` / `os.arch` / `os.version`(`"windows" / "linux" / "osx"`,arch `"x86" / "x86_64"` 等)
- `features.{name}`(`is_demo_user` / `has_custom_resolution` / `has_quick_plays_support` / `is_quick_play_*`)
- 真实 26.2 实测 rules 只用 `os.name`(无 arch、无 disallow、无 features)— 简化实现

**教学点 3 — 占位符替换**:`arguments.jvm[]` 和 `arguments.game[]` 里的 `${name}` 必须替换为具体值。L6 离线模式占位符全集:

| 占位符 | 离线值 |
|---|---|
| `${natives_directory}` | `<game_dir>/versions/<id>/natives`(注意:`-Djava.library.path=${natives_directory}/java`,需要拼 `/java` 子目录) |
| `${classpath}` | 拼好的 cp 字符串 |
| `${version_name}` | `<id>` |
| `${game_directory}` | `<game_dir>`(exe 旁 `.bamcl-dev/`) |
| `${assets_root}` | `<game_dir>/assets` |
| `${assets_index_name}` | `<id>.json` 的 `assets` 字段(26.2 为 `"32"`) |
| `${auth_player_name}` | `"Player"` |
| `${auth_uuid}` | 离线生成的 UUID(`UUID.nameUUIDFromBytes("OfflinePlayer:Player".bytes)`) |
| `${auth_access_token}` | `""`(离线模式) |
| `${auth_xuid}` | `""`(离线模式) |
| `${version_type}` | `<id>.json` 的 `type` 字段 |
| `${launcher_name}` / `${launcher_version}` | `"BAMCLaunch"` / `"0.1.0"` |
| `${clientid}` / `${user_type}` / `${resolution_width}` 等 | `""`(未涉及的 features 占位符按需给空串) |
| `${quickPlay*}` | `""`(M3 才接 quick play) |

**教学点 4 — 进程拉起参数**:
- `tokio::process::Command::new(java_path)`
- `.args(jvm_args_with_placeholders_replaced)`
- `.arg("<main_class>")`  例如 `net.minecraft.client.main.Main`
- `.args(game_args_with_placeholders_replaced)`
- `.current_dir(game_dir)` — **关键**: Minecraft 启动时用相对路径找 `versions/`、`assets/` 等
- `.stdout(Stdio::piped()).stderr(Stdio::piped())` — 留 stdout/stderr 读取通道(L6 暂时不消费,M3 console panel 用)
- `.spawn()` → `Child { id, ... }` → **返回 PID + Java path**(给前端显示「已启动」)

**教学点 5 — L6 的边界**:
- **不消费 stdout/stderr** — 进程后台跑,启动器不阻塞。读日志留到 M3 做 console panel
- **不持久化进程**(不存 PID) — 关窗口不杀进程,玩家自己管;杀进程/查进程留到 M3
- **不做微软登录** — 离线模式固定占位符。微软 OAuth 是 M4+
- **不做 mods / 整合包** — 纯 vanilla 启动
- **不做内存设置** — 用默认 JVM 参数(`arguments.jvm[]` 里硬编码的 `-Xmx4G` 等),玩家可在设置里覆盖(M3)

**数据流**:

```
[启动]按钮 (新, 取代当前 disabled 状态)
  → invoke("launch_version", { versionId, javaPath })
  → Rust:
     1. 读 <id>.json → mainClass + arguments.jvm/game/libraries/assets
     2. 拼 classpath:遍历 libraries/*.jar + versions/<id>/client.jar + <id>/<id>.json
     3. 拼 JVM args:遍历 arguments.jvm[], rules 过滤, 替换 ${...}
     4. 拼 game args:遍历 arguments.game[], rules 过滤, 替换 ${...}
     5. spawn java process
  → 返回 { pid, java_path } → 前端按钮变「启动中」(其实游戏已跑)
```

**契约**:

- `launch_version(version_id: String, java_path: String) -> Result<LaunchResult, String>`,其中 `LaunchResult { pid: u32, java_path: String }`(serde Serialize)
- `current_os() -> OsKind`(`"windows" | "linux" | "osx"`)— 内部用,不在契约
- `arg_rule_applies(rule: &ArgRule, os: OsKind, features: &HashSet<String>) -> bool` — 教学:rules 语义"最后一条匹配的定夺"
- `expand_placeholders(text: &str, vars: &HashMap<String, String>) -> String` — 教学:`${xxx}` 替换,未匹配保留原样(或报错)
- `build_classpath(version_id: &str, libraries_dir: &Path) -> Result<String, String>` — 平台分隔符拼 jar 路径
- `spawn_game_process(java_path: &Path, args: &[String], cwd: &Path) -> Result<u32, String>` — tokio::process::Command

**纯逻辑 TDD**(预计新增 ~6 个测试):

- `fn arg_rule_applies_simple_os_match` — os.name="windows", rule 也是 "windows" → 通过
- `fn arg_rule_applies_no_match` — os.name="windows", rule 是 "linux" → 不通过
- `fn arg_rule_applies_arch` — os.arch="x86_64", rule arch "x86" → 不通过
- `fn arg_rule_picks_last_match` — 多条 rule, 最后一条 "allow" 胜出
- `fn expand_placeholders_basic` — `${foo}` → 替换值
- `fn expand_placeholders_no_match_leaves_intact` — 未匹配保留 `${...}`
- `fn expand_placeholders_multiple` — 多个占位符同字符串
- `fn build_classpath_joins_jars_with_platform_separator` — classpath 拼接(用临时 fixture 测,只测 jar 列表遍历逻辑,不真 spawn)

**新增依赖**:`uuid = "1"`(离线 UUID 生成;`UUID.nameUUIDFromBytes` 是 JDK 标准但我们是 Rust 侧算,因为 Java 还在外面没启动)

**前端**: VersionCard 取代当前 disabled 「启动」按钮。新按钮状态机:`idle → launching → launched(pid)` / `error`。launched 状态显示 PID + 「再次启动」。

**UI**: 启动按钮跟 [Java] 紧邻,launched 后绿色 outline + 「已启动 (pid 1234)」Tooltip。

布局: 不写新文件 — 启动不产生任何磁盘产物(进程在内存)。游戏目录仍是 `.bamcl-dev/`(L1 已锚定)。

**Out of scope**:
- 读游戏 stdout/stderr 推到前端(M3 console panel)
- 进程管理:列出运行中的 MC、杀进程、kill(M3)
- 设置:Java 路径选择 / 内存 / JVM 参数覆盖(M3)
- 微软账户登录(M4+)
- mods / 整合包(M5+)
- 跨进程通信:游戏关 → 自动清理(M3+)
- 自动化测试 spawn 测试(`#[ignore]` e2e smoke 类似 L5,默认不跑)

具体细化到此。

## [S3] Out of Scope

- 除 L1 之外的 M2 内容(见后续课)
- 微软账户登录、Mod/整合包、设置持久化(M3/M4)
- L5 仅做 Windows 扫描路径;macOS/Linux 路径细化等真上跨平台时补
- **Java 自下载**(玩家自己装系统 Java;L5 只发现不下载)
- **手动指定 Java 路径**的设置持久化(M3)
- L6:读游戏 stdout/stderr 推到前端(M3 console panel)
- L6:进程管理 — 列出 / 杀 / 自动清理(M3)
- L6:设置 — Java 路径选择 / 内存 / JVM 参数覆盖(M3)
- L6:微软账户登录(M4+)、mods/整合包(M5+)
- 自动化测试(沿用 M1 约定:验证 = `npm run build` + `cargo check` + `tauri dev` 手动;L5/L6 加 TDD 是因为有纯函数 + 正则/规则过滤值得测)

## Tasks

- [x] T-L1: 下载版本 JSON 全链路(设计见 [S2]/L1)—— acceptance: `npm run build` 与 `cargo check` 通过;`tauri dev` 中点「下载」后 `.bamcl-dev/versions/<id>/<id>.json` 真实落盘,按钮状态正确切换 (covers: S2)
- [x] T-L2: client.jar 下载 + sha1 校验 —— acceptance: `cargo test` 全绿(sha1/verify/解析 3 测试);`npm run build` 通过;`tauri dev` 点「客户端」后 `versions/<id>/client.jar` 落盘(≈37MB 真实数据),按钮状态正确;篡改说明书中的 sha1 后点下载应报"校验失败" (covers: S2)
- [x] T-L3: assets 资源下载 —— acceptance: `cargo test` 全绿(assetIndex 解析 / 内容寻址路径 / verify 复用);`npm run build` 通过;`tauri dev` 点「资源」后 `assets/indexes/32.json` 落盘 + 首次全量下载 5057 文件(479,185,985 B),再点一次 skipped 全量(增量验证);篡改 index 中某文件 hash 后应报错 (covers: S2)
- [x] T-L4: libraries 下载 + natives 解压(rules 过滤 / sha1 校验 / 并发 8 / 路径穿越防护 / 胖 jar 架构裁剪)—— acceptance: `cargo test` 全绿(rules 过滤 / native 识别 / 安全路径 / 架构裁剪 共 6 测试);`npm run build` 通过;`tauri dev` 点「库」后 `libraries/` 落盘 Windows 所需 jar(~86MB)+ `versions/26.2/natives/` 只含 x64 架构 dll(无 arm64/x86/META-INF);再点一次 skipped 全量;篡改某 jar sha1 后应报错 (covers: S2)
- [x] T-L5: Java 发现(扫描 + 版本适配,见 [S2]/L5)—— acceptance: `cargo test` 全绿(parse_java_version 新旧格式 / meets_requirement / dedupe_candidates / discover_from_env / discover_from_common_dirs / discover_from_registry mock 共 ≥6 测试);`npm run build` 通过;`tauri dev` 点 26.2「Java」→ 弹 Modal 列出本机所有 Java 候选(适配 Java 25 的置顶,标 source),未检测到 Java 时显示提示文案;注册表无访问权限时不阻断整体(降级继续) (covers: S2)
- [ ] T-L6: 启动参数 + 进程拉起(离线,见 [S2]/L6)—— acceptance: `cargo test` 全绿(arg_rule_applies os/arch/last-match / expand_placeholders basic/no-match/multiple / build_classpath 平台分隔符 / rules 过滤实战 26.2.json 共 ≥6 测试);`npm run build` 通过;`tauri dev` 点 26.2「启动」+ Java 25 → spawn 成功, 按钮变「已启动 (pid X)」;`tauri dev` 终端应能看到 MC 启动日志刷屏(代表 stdout 走通);若 26.2 已下完整物料(json + jar + assets + libraries + natives),首次启动应能进 MC 主菜单(不要求登录微软,离线玩家名 Player) (covers: S2)