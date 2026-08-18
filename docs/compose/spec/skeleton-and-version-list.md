---
feature: skeleton-and-version-list
status: delivered
updated: 2026-08-18
branch: main
commits: a22cc6d..50fd20c
---

# BAMCLaunch 里程碑 1:项目骨架 + Minecraft 版本列表

## Report

**What was built** — M1 交付:Tauri 2 + React 19 + TypeScript 可运行骨架;BA×MC 设计令牌(品牌蓝 `#4C9EEB` / 草方块绿 `#7CBD4B` + 全局样式);Rust 端 `fetch_version_manifest` 命令(Mojang 官方版本清单,reqwest + serde camelCase);前端 `useVersionManifest` 三态 hook(loading/error/success + 重试);无边框窗口 + 自绘标题栏(拖拽/最小化/最大化/关闭)+ 侧边导航(版本/资源下载/账户/设置);主页版本列表(正式版/快照版分组、最新徽标、刷新);README + MIT 许可证。M2 沿用其 IPC 教学模式与目录约定。

**Verification** — `npm run build` ✓ / `cargo check` ✓ / `tauri dev` 手动验收:窗口无边框可拖拽、三窗口按钮可用、版本列表真实加载(2026-08 时最新 release 26.2 / snapshot 26.3-snapshot-9)、路由切换无误。M1 按约定不含自动化测试(自 M2-L1 起引入 `cargo test`)。

**Journey log** — create-tauri-app v4.6.2 拒绝非空目录 → 临时子目录脚手架后 Move-Item 上移;本机会话守卫挡 `git branch -M` → 删空 .git 后 `git init -b main`;Tauri 2 自绘标题栏按钮需显式 capabilities(`core:window:allow-minimize` 等 3 项,缺则 cargo build 报错);npm allow-scripts 机制需 `npm approve-scripts esbuild`;reqwest 0.13 + rustls 编译干净无需 openssl,首次 debug 构建约 414 crates。

## [S1] Problem

用户(编程新手)要创建自己的第一个 GitHub 项目:BAMCLaunch —— 一个 Minecraft 启动器。

- 技术栈:Tauri 2.0 + React + TypeScript(Vite 构建)
- 界面风格:蔚蓝档案(Blue Archive) × 我的世界(Minecraft)融合
- 界面结构参考 NexBox,功能路线参考 SJMCL、HMCL

里程碑 1 的目的:把能跑起来的项目骨架搭好,并实现第一个真实功能「Minecraft 版本列表」(从 Mojang 官方 API 拉取),让用户理解 Tauri 前后端通信用法、项目结构约定,为后续里程碑(下载、启动、账户…)铺路。

## [S2] Design

### 技术栈

- 前端:Tauri 2 + React + TypeScript + Vite(create-tauri-app `react-ts` 模板)
- UI:Chakra UI v2 + @emotion/react + @emotion/styled + framer-motion(与 NexBox/SJMCL 一致)
- 路由:react-router-dom(v7 declarative 模式:`BrowserRouter / Routes / Route`)
- 后端:Rust,`reqwest`(features = ["json"])拉取 Mojang 版本清单,serde 反序列化

### 目录结构(参考 NexBox)

```
bamcl/
├── src/                      # 前端(React)
│   ├── main.tsx              # 入口:ChakraProvider + BrowserRouter
│   ├── App.tsx               # 布局壳:侧边导航 + 内容区
│   ├── theme/                # Chakra 主题(BA×MC 设计令牌)
│   ├── components/           # 复用组件:TitleBar / VersionCard …
│   ├── pages/                # 页面:Home(版本列表)/ Download / Accounts / Settings
│   ├── hooks/                # useVersionManifest(版本清单数据 hook)
│   ├── lib/                  # 工具:invoke 封装、常量
│   └── types/                # TS 类型:VersionManifest 等
└── src-tauri/                # 后端(Rust)
    ├── src/lib.rs            # 注册 tauri commands
    └── src/commands/version.rs  # fetch_version_manifest 命令
```

### 数据流(版本列表)

```
React hook (useVersionManifest)
  → invoke("fetch_version_manifest")
  → Rust command:reqwest GET https://launchermeta.mojang.com/mc/game/version_manifest_v2.json
  → serde 解析为 VersionManifest JSON
  → 前端渲染:分区展示 release / snapshot,最新正式版带「最新」徽标
```

返回结构(经验证的真实 API 形状):

```
latest:   { release: string, snapshot: string }
versions: [{ id, type: "release"|"snapshot", url, time, releaseTime, sha1, complianceLevel }]
```

错误处理(系统边界):网络失败时 Rust 返回错误字符串 → 前端展示错误卡片 + 重试按钮;loading / success / error 三态齐全。

### 界面设计令牌(BA × MC)

- 主色:`#4C9EEB`(BA 蓝);强调色:`#7CBD4B`(MC 草方块绿)
- 背景:浅色系,白 → 浅蓝渐变(`#F5F9FF`),卡片纯白 + 柔和阴影 + 16px 圆角
- 自定义标题栏:无边框窗口(`decorations: false`),`data-tauri-drag-region` 拖拽区,自绘最小化/最大化/关闭按钮
- 侧边导航:左侧竖排(图标 + 文字),选中态为 BA 蓝胶囊底 + 白色文字
- MC 像素点缀:版本徽标/边框使用像素化细节(如 `image-rendering: pixelated`、绿宝石色标签)

### 窗口

- 1280 × 800,最小 900 × 600,`decorations: false`(无系统边框,自定义标题栏)

### 工程化

- 本地 git 仓库 + GitHub 远程 `BAMCLaunch`(public),默认分支 `main`
- 每个任务一个 commit,conventional commits 风格,信息用中文注释说明
- README:项目简介、技术栈、运行方式、里程碑路线图
- LICENSE:MIT
- `.gitignore` 追加 `.mimocode/`(本地 agent 工具目录,不入库)

## [S3] Out of Scope

- 账户系统(微软登录 / 第三方认证)、游戏下载安装、启动游戏、mod / 资源包管理、服务器列表、设置持久化、多实例管理 —— 后续里程碑
- 深浅色主题切换、多语言 i18n
- 自动化测试(本里程碑验证以 typecheck + build + cargo check + `tauri dev` 手动验收为准;自动化测试从包含纯逻辑模块的里程碑开始引入)

## Tasks

- [x] T1: 初始化 Tauri 2 + React + TS 项目;安装 Chakra UI v2 全家桶与 react-router-dom;删除模板演示代码;`.gitignore` 追加 `.mimocode/` — acceptance: `npm run build` 与 `cargo check` 均通过 (covers: S2)
- [x] T2: 编写 BA×MC Chakra 主题(`src/theme/`)+ 全局样式 — acceptance: 主题导出 colors / shadows / radii / 组件样式,`npm run build` 通过 (covers: S2)
- [x] T3: Rust 后端 `fetch_version_manifest` command(reqwest + serde 解析 Mojang 版本清单)并在 lib.rs 注册 — acceptance: `cargo check` 通过,`tauri dev` 中调用可返回真实数据 (covers: S2)
- [x] T4: 前端类型定义(`src/types/version.ts`)+ `useVersionManifest` hook(loading / error / data 状态机,支持重试)— acceptance: `npm run build` 通过 (covers: S2)
- [x] T5: 窗口配置(decorations: false)+ 自定义 TitleBar 组件(拖拽区、最小化/最大化/关闭)+ 侧边导航布局 — acceptance: `tauri dev` 窗口可拖动、三个窗口按钮可用 (covers: S2)
- [x] T6: 路由与页面:主页(版本列表三态 + release/snapshot 分组 + 最新徽标)、下载 / 账户 / 设置占位页 — acceptance: `tauri dev` 完整界面可见,切换无误;`npm run build` 通过 (covers: S2)
- [x] T7: README(简介 / 技术栈 / 运行方式 / 路线图)+ LICENSE(MIT)— acceptance: 文件存在且内容准确 (covers: S2)