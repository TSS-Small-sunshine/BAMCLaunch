# docs/compose/ 写作规范

本目录按"**里程碑规格(spec)** + **子任务计划(plan)**"两层组织 BAMCLaunch 项目的设计与实施文档。任何新增或维护此目录的人,动手前请先通读本 README。

## 1. 目录结构

```
docs/compose/
├── README.md         (本文件 — 写作规范与维护约定)
├── spec/             (里程碑规格:What & Why)
│   ├── skeleton-and-version-list.md   (M1 ✅ delivered)
│   └── m2-download-and-launch.md       (M2 🟡 in-progress)
└── plans/             (子任务计划:How & When)
    └── 2026-08-18-l1-version-json-download.md  (M2-L1 示例)
```

## 2. spec/ 与 plans/ 的角色分工

两类文档**不重复内容**:

- **spec/** — 里程碑级的"做什么 / 为什么"。包含里程碑目标、用户故事、功能拆解(子课 L1~L6)、验收标准、参考资料、Out of Scope。**不**写具体实现代码、**不**写子任务排期、**不**写 commit 粒度。
- **plans/** — 子任务级的"怎么实现 / 谁做 / 何时完"。包含具体步骤、改动文件列表、完整代码块、任务/提交拆分、单元测试点、e2e 验证路径、TDD 节奏。**不**重新解释 spec 的目标、**不**讨论替代方案的高层权衡(这些归 spec)、**不**预测未来里程碑。

一句话:**spec 给"协作者 / 未来的自己"看,plans 给"当前冲刺的执行者"看。**

## 3. 写作风格对比

| 维度 | spec/ | plans/ |
| --- | --- | --- |
| 视角 | 产品 + 架构 | 工程 + 排期 |
| 受众 | 协作者 / 未来的自己 | 当前冲刺的执行者 |
| 时间跨度 | 整个里程碑(数日~数周) | 1-3 天(单子任务) |
| 更新频率 | 里程碑内少改 | 频繁追加 L2 / L3 / ... |
| 引用源 | 需求 + 类似项目(NexBox / SJMCL / HMCL) | 源码 + spec 引用 |
| 必含 frontmatter | 是(feature / status / updated / branch / commits) | 否(标题即定位) |
| 代码示例 | 关键契约 / 数据结构示意 | 完整可粘贴的代码块 |
| 任务清单 | 高层 T1~T7(里程碑级,粗粒度) | 详细 Task 1~N(每任务含 [ ] 勾选 + Step) |

## 4. 互引规则

- **plans 必须引用对应 spec** — 在 plans 文档开篇或"Architecture"小节写明 `本计划对应 spec: ../spec/<milestone-code>-<short-desc>.md`(如 `../spec/m2-download-and-launch.md`)。
- **spec 引用 plans 时只列索引** — 用"详见 `plans/2026-08-XX-lx-xxx.md`"的简短链接,**不**复制 plan 的步骤或代码。
- **引用 commit hash** — 未落地时用 `<commit-hash>` 占位;plan 完成后用真实 hash 回填 spec 的 frontmatter `commits` 字段(形如 `a22cc6d..50fd20c`)。
- **引用源码位置** — 用 `path/to/file.rs:NN` 或 `path/to/file.ts:NN` 格式,行号尽量精确(跨大段改动另起一条引用)。
- **避免循环依赖** — plans 不引用其他 plans;若 L2 plan 需要看 L1 落地状态,改用"L1 plan 已完成 → 当前 `.bamcl-dev/` 目录结构"的事实陈述。

## 5. 命名规范

- **spec 文件**:`<milestone-code>-<short-desc>.md`(全小写,短横线分隔)
  - 例:`m3-account-system.md`、`m4-microsoft-auth.md`
  - `milestone-code` 沿用 M1 / M2 / M3 / ...(`m` 小写)
- **plans 文件**:`<YYYY-MM-DD>-l<n>-<short-desc>.md`
  - 例:`2026-08-18-l1-version-json-download.md`、`2026-08-19-l2-client-jar-download.md`
  - **日期必须是 plan 创建日期,不是完成日期**(便于按时间排序)
  - `l<n>` 是 M2 spec 引入的"教学分层"约定(L1=版本 JSON / L2=jar / L3=assets / L4=libraries / L5=Java / L6=启动)。其他里程碑可参考此粒度
  - 短描述用动名词或名词短语,**不含**状态词(不写 `2026-08-18-l1-done.md`)

## 6. 现有文档的整理建议

读完三篇现有文档后,给出现状判断:

**`skeleton-and-version-list.md`(M1)**:`spec/` 的好榜样 — frontmatter + Report + [S1] / [S2] / [S3] + 里程碑级 Tasks 一应俱全,且无任何 plans 内容混入。**强烈建议作为后续 spec 的模板基准**。

**`m2-download-and-launch.md`(M2)**:因为 M2 跨度大(6 个 L 课),作者把每 L 课完成后的"Report / Verification / Journey log"也内嵌进了 spec 顶部,使该文件兼具 spec + 多个 plan 总结的角色。这种"长 Report 头"在里程碑大、改动多时有可读性收益,但代价是 spec 长度膨胀(421 行)、**不**符合第 2 节的"spec 不含实施细节"原则。建议:**M3 起回归到简洁 spec 风格**,每 L 课只写"设计 + 验收",把"实际产出 / Verification / Journey log"挪到对应 plans 文档中。

**`2026-08-18-l1-version-json-download.md`**:`plans/` 的好榜样 — Goal / Architecture / Tech Stack 开篇,Global Constraints 集中约定,Task 1~5 每任务含勾选框 + 完整代码块 + 验证步骤 + 提交命令,末尾 Self-Review 自检。**建议作为后续 plans 的模板基准**。唯一可改进:开篇补一句 `对应 spec: ../spec/m2-download-and-launch.md` 的明示互引(目前只在 Task 1 间接提及)。

## 7. 新建 spec/plan 的流程

1. **启动新里程碑** → 新建 `spec/<m-code>-<short-desc>.md`,先填三段骨架:`## [S1] Problem`(用户故事 + 里程碑目标)/ `## [S2] Design`(技术栈 + 数据流 + 接口契约)/ `## [S3] Out of Scope`(明确不做什么)。
2. **spec 稳定后拆子任务** → 每个子任务新建 `plans/<YYYY-MM-DD>-l<n>-<short-desc>.md`,开篇明示 `本计划对应 spec: ../spec/<m-code>-<short-desc>.md`。
3. **plan 落地时** → 末尾 Self-Review 勾选完所有 [ ];commit hash 填回 spec 的 frontmatter `commits` 字段。
4. **里程碑交付** → spec 的 `status` 改为 `delivered`,`commits` 字段写完整 `<base-sha>..<head-sha>`,并把所有"Verification / Journey log"从 spec 顶部 Report 迁移到对应 plans 文档。
