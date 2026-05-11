# 用户指令记忆

本文件记录长期有效的用户偏好与项目级约束，避免保留过多过程性细节。

## 条目

[执行与提交偏好]
- Date: 2026-05-09
- Context: 用户对协作方式与提交行为的明确要求
- Instructions:
  - 如果下一步明确，就继续执行，不要停在中间等待确认；只有需求不清楚时才提问。
  - 当用户说"提交"时，默认提交本次工作区里与当前任务相关的改动。
  - 仍应排除明显的本地产物或构建缓存，例如 `node_modules/.vite` 一类自动生成文件。
  - 如果已有公共封装或公共函数，优先复用公共实现，不要在多个文件里重复写同一套逻辑。

[分支管理偏好]
- Date: 2026-05-09
- Context: 用户明确要求分支管理策略
- Instructions:
  - 始终在一个分支上开发，不要创建多个分支。
  - 如果当前已在某个功能分支上，继续在该分支上提交新改动，不要切到新分支。
  - 只有在当前分支已合并到 main 且需要开始全新的大功能时，才考虑创建新分支。

[分支管理偏好补充]
- Date: 2026-05-11
- Context: 本次修复中再次确认用户希望持续在同一分支上开发
- Instructions:
  - 后续同一任务链路内继续沿用当前分支，不要重复创建新分支。

[前端状态知识]
- Date: 2026-05-11
- Context: Agent 在执行 Fix/Fix All 按钮 loading 修复时发现
- Category: 代码模式
- Instructions:
  - 单个 issue 的 Fix 在后端执行期间会先把 issue 状态置为 `claimed`，前端需要把 `claimed` 视为仍在修复中。
  - Fix All 的按钮 loading 不能在请求返回后立即清除，应跟随后端 workflow 的 `running` 状态持续显示。

[FixAgent 执行方式偏好]
- Date: 2026-05-11
- Context: 用户明确要求 Fix 不要走 run_simple
- Instructions:
  - FixAgent 执行修复时统一使用 Agent 模式，不要走 `run_simple()` 直提取路径。

[编译检查偏好]
- Date: 2026-05-10
- Context: 用户明确指示跳过编译检查
- Instructions:
  - 对于 Rust 项目（ReviewAgent、FixAgent、Orchestrator），修改代码后不需要运行 cargo check 或 cargo build 进行编译检查。
  - 代码修改完成后直接提交并推送，不要等待编译验证。
