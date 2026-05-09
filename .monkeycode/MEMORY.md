# 用户指令记忆

本文件记录了用户的指令、偏好和教导，用于在未来的交互中提供参考。

## 格式

### 用户指令条目
用户指令条目应遵循以下格式：

[用户指令摘要]
- Date: [YYYY-MM-DD]
- Context: [提及的场景或时间]
- Instructions:
  - [用户教导或指示的内容，逐行描述]

### 项目知识条目
Agent 在任务执行过程中发现的条目应遵循以下格式：

[项目知识摘要]
- Date: [YYYY-MM-DD]
- Context: Agent 在执行 [具体任务描述] 时发现
- Category: [代码结构|代码模式|代码生成|构建方法|测试方法|依赖关系|环境配置]
- Instructions:
  - [具体的知识点，逐行描述]

## 去重策略
- 添加新条目前，检查是否存在相似或相同的指令
- 若发现重复，跳过新条目或与已有条目合并
- 合并时，更新上下文或日期信息
- 这有助于避免冗余条目，保持记忆文件整洁

## 条目

[ReviewAgent 当前编排结构]
- Date: 2026-05-08
- Context: Agent 在执行 fix agent 和 orchestrator 设计分析时发现
- Category: 代码结构
- Instructions:
  - 主流程当前围绕 `ReviewOrchestrator`，负责 `prepare -> review -> merge/report`。
  - ACP 入口位于 `ReviewAgent/src/acp/agent.rs`，CLI 入口位于 `ReviewAgent/src/main.rs`。
  - 当前系统已有 review 结果合并、issue 校验和置信度过滤能力，但还没有独立的问题池、fix agent 和 verifier 状态流转。

[FixAgent 目录边界要求]
- Date: 2026-05-08
- Context: 用户在实现 fix agent 和 orchestrator 时明确要求
- Instructions:
  - `ReviewAgent` 作为 submodule 仅用于生成 review 数据。
  - `FixAgent` 应作为与 `ReviewAgent` 平级的独立目录实现，使用 Rust。
  - `Orchestrator` 也应作为与 `ReviewAgent`、`FixAgent` 平级的独立目录实现。
  - 项目需要提供根目录 `docker-compose.yml` 以便部署。

[问题数据建模要求]
- Date: 2026-05-08
- Context: 用户在讨论数据库和问题追踪关系时明确要求
- Instructions:
  - `docker-compose.yml` 需要包含数据库服务。
  - 问题与已解决问题不应继续存储在 `.json` 文件中，应改为数据库持久化，便于统计分析。
  - 数据关系应按 `project -> pr/mr -> issue/fix` 建模。
  - 问题与解决方案都需要跟随对应的 PR/MR 归属，而 PR/MR 需要归属于 project。

[默认持续执行偏好]
- Date: 2026-05-08
- Context: 用户在当前实现阶段要求我自行继续下一步，除非不确定才停下来确认
- Instructions:
  - 如果我有明确的下一步，就继续执行，不要停在中间等待确认。
  - 只有在需求不清楚或存在关键歧义时，才停下来向用户询问。

[目标产品形态]
- Date: 2026-05-08
- Context: 用户描述未来希望的产品使用方式
- Instructions:
  - 需要有一个前端界面，用户进入项目后输入 PR，即可自动触发完整流程。
  - 系统应持续自动执行 review、fix、verify，而不是只跑单轮 CLI 命令。
  - workflow 的停止条件应以 verifier 结果为准，直到没有需要继续修复的重要 bug 为止。

[前端风格要求]
- Date: 2026-05-08
- Context: 用户要求开始做前端，并指定界面风格
- Instructions:
  - 前端视觉风格应与 monkeycode-ai 保持一致。
  - 优先实现可用的产品界面，而不是仅补演示级页面。

[Rust 工具链要求]
- Date: 2026-05-08
- Context: Agent 在启动 Orchestrator HTTP API 并编译工程时发现
- Category: 环境配置
- Instructions:
  - 系统自带 `/usr/bin/cargo` 版本过旧，仅支持到 Rust 2021 edition，无法编译当前 `edition = "2024"` 的 Rust 工程。
  - 需要使用 `rustup` 安装的新工具链，即 `/root/.cargo/bin/cargo` 与 `/root/.cargo/bin/rustc`。

[前端界面精修偏好]
- Date: 2026-05-08
- Context: 用户对当前前端视觉效果提出进一步要求
- Instructions:
  - 前端不要做成泛 AI 风格，要更像成熟的专业产品界面。
  - 可参考 `https://deepkb.com.cn/` 的配色气质，采用更克制、更精致的视觉语言。
  - 页面上的按钮不能只是摆设，所有按钮都需要具备真实可点击交互。
  - 交付前要检查主要交互是否可用，避免界面看起来完成但实际不能用。

[README 呈现偏好]
- Date: 2026-05-08
- Context: 用户要求更新根目录 README.md 的表达与视觉层次
- Instructions:
  - README.md 需要写得更精致、美观，呈现效果应更像正式项目首页。
  - 在保证信息完整的前提下，优先提升结构层次、语言质感和可读性。

[Bug 列表交互与视觉偏好]
- Date: 2026-05-08
- Context: 用户在改进 Bugs 页面时进一步明确要求
- Instructions:
  - Bugs 页面不要做成表格视图。
  - 优先保留更有产品感的卡片式信息展示。
  - 问题列表应支持点击后联动跳转到对应 workflow 或 PR 上下文。

[前端信息架构要求]
- Date: 2026-05-08
- Context: 用户重新明确前端页面主结构
- Instructions:
  - 前端信息结构应围绕 `项目 -> PR -> bugs pool / tasks` 展开。
  - 不要继续以 workflow 作为页面的主导航中心。
  - 页面应优先体现项目层、PR 层，以及该 PR 下的问题池和任务视图。

[前端视觉质感要求]
- Date: 2026-05-08
- Context: 用户继续要求提升页面完成度和产品质感
- Instructions:
  - 页面需要更整齐，避免卡片、标题、状态信息出现松散和不对齐的问题。
  - 整体观感要更高级，避免像临时拼装的控制台。
  - 优先通过节奏、留白、层级、一致性来提升质感，而不是堆砌装饰。

[前端层次与对比要求]
- Date: 2026-05-08
- Context: 用户查看深色改版后继续指出视觉问题
- Instructions:
  - 边框之间不要挤在一起，卡片与容器之间要有更清晰的呼吸感。
  - 页面必须有明确的层次，不能所有块都贴在一个平面上。
  - 需要建立明显的深浅浓淡区分，让背景层、面板层、卡片层、文字层彼此分明。

[前端留白与边界要求]
- Date: 2026-05-08
- Context: 用户继续指出卡片和容器边界过于拥挤
- Instructions:
  - 卡片与卡片之间必须有明显空隙，不能边框紧贴边框。
  - 卡片与外层容器之间也要保留清晰留白，避免视觉上挤在一起。
  - 优先通过减弱外层边框、增强内边距和背景分层来解决拥挤问题。

[Project 与 PR 入口要求]
- Date: 2026-05-08
- Context: 用户继续完善 Project 页面和 PR 卡片体验
- Instructions:
  - `project name` 优先从已有 PR 信息中推导，而不是完全依赖后端返回字段。
  - PR 卡片需要继续设计优化，增强辨识度和产品感。

[左侧导航与品牌位要求]
- Date: 2026-05-08
- Context: 用户继续细化左侧菜单视觉和品牌呈现
- Instructions:
  - 左侧菜单应采用一级 `Project` + 二级 project 列表的结构。
  - 左侧视觉要简单、大方，不要做得过满或过重。
  - 最上方品牌位需要使用工作区里的 `monkeycode1.png` 图标。
  - 左侧二级 project 列表应更像目录树，而不是一组按钮。
  - 展开后不需要额外显示 `Project List` 这类重复说明文字。
  - 左侧展开区不需要 `Refresh` 按钮。
  - 顶部不需要 `Project → PRPool`、`Select a project`、`English Mild Ale Palette` 这类无用标题文案。
  - 右上角不需要 `Refresh` 和 `Open PR` 按钮。
