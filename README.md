# BAMCLaunch

> 蔚蓝档案(Blue Archive) × 我的世界(Minecraft)风格交融的 Minecraft 启动器,基于 Tauri 2 + React + TypeScript 构建。第一个 GitHub 项目,正在一步一步学习中成长。

> ⚠️ 本项目与 Mojang Studios / Microsoft 无任何关联,并非 Minecraft 官方服务。

---

## ✨ 设计理念

- **界面**参考 [NexBox](https://github.com/MuLiuSaMa/NexBox) 的结构与风格,融合 AC「BLUE ARCHIVE」的明亮浅蓝美学与 MC 的草方块像素点缀
- **功能**路线参考 [SJMCL](https://github.com/UNIkeEN/SJMCL)、[HMCL](https://github.com/HMCL-dev/HMCL)、[PCL](https://github.com/Meloong-Git/PCL)
- **架构**:前端负责界面与交互,后端(Rust)负责一切系统能力(网络、文件、进程),通过 Tauri IPC(`invoke`)通信

## 🗺️ 路线图

| 里程碑 | 内容 | 状态 |
| ------ | ---- | ---- |
| M1 | 项目骨架 + Minecraft 版本列表(Mojang 官方清单) | ✅ 已完成 |
| M2 | 版本下载与游戏启动 | ⏳ 规划中 |
| M3 | 账户系统(微软登录 / 离线模式) | ⏳ 规划中 |
| M4 | 资源下载(Mod / 整合包 / 光影) | ⏳ 规划中 |

## 🛠️ 技术栈

- **桌面框架**:[Tauri 2](https://tauri.app)(Rust)
- **前端**:React 19 + TypeScript + Vite
- **UI**:Chakra UI v2 + Emotion + Framer Motion
- **路由**:react-router-dom
- **后端**:reqwest(HTTP)、serde(序列化)

## 🚀 快速开始

前置依赖:

- [Node.js](https://nodejs.org) 20+
- [Rust](https://rustup.rs) 1.77+ + Visual Studio Build Tools(C++ 桌面开发工作负载,Windows 需要)

```bash
# 1. 安装前端依赖
npm install

# 2. 启动开发模式(带热重载,会打开启动器窗口)
npm run tauri dev
```

构建生产版本:

```bash
npm run tauri build
```

## 📁 项目结构

```
bamcl/
├── src/                      # 前端(React)
│   ├── main.tsx              # 入口:ChakraProvider + Router
│   ├── App.tsx               # 布局壳:标题栏 + 侧边导航 + 内容区
│   ├── theme/                # BA×MC 设计令牌(Chakra 主题)
│   ├── components/           # 复用组件(TitleBar / Sidebar)
│   ├── pages/                # 页面(版本列表 / 占位页)
│   ├── hooks/                # 自定义 Hooks(useVersionManifest)
│   ├── lib/                  # 工具(invoke 封装)
│   └── types/                # TS 类型定义
└── src-tauri/                # 后端(Rust)
    └── src/
        ├── lib.rs            # 注册 tauri commands
        └── commands/         # 命令实现(version.rs)
```

## 🔗 参考与致谢

- [NexBox](https://github.com/MuLiuSaMa/NexBox) — UI 结构与风格参考
- [SJMCL](https://github.com/UNIkeEN/SJMCL) — 功能路线参考
- [HMCL](https://github.com/HMCL-dev/HMCL)、[PCL](https://github.com/Meloong-Git/PCL) — Minecraft 启动器参考
- [Tauri](https://tauri.app)、[Chakra UI](https://chakra-ui.com) — 底层框架

## 📄 License

[MIT](./LICENSE) © 2026 TSS-Small-sunshine