# FixAgent

一个面向 Pull Request 自动修复流程的工程化系统。

FixAgent 不只是“发现问题”，而是围绕代码审查结果继续推进后续动作：

- 接收 `ReviewAgent` 生成的 review 结果
- 将 issue、fix、verification 持久化到数据库
- 以 workflow 方式持续执行 `review -> fix -> verify`
- 通过前端控制台实时展示运行状态、轮次进度与错误池

## Architecture

仓库当前由四个主要部分组成：

| Module | Role |
| --- | --- |
| `ReviewAgent/` | review 引擎 submodule，负责生成 PR/MR 审查结果 |
| `FixAgent/` | 自动修复引擎，负责消费 issue 并生成或应用修复 |
| `Orchestrator/` | workflow 编排层，负责数据库持久化、任务推进与 HTTP API |
| `web/` | 前端控制台，负责发起 workflow、查看实时状态和错误池 |

## Workflow

系统围绕单个 PR/MR 执行收敛式处理：

1. `ReviewAgent` 分析 PR diff，生成结构化问题列表。
2. `Orchestrator` 将项目、PR、issue、fix、verification 写入 PostgreSQL。
3. `FixAgent` 领取当前最需要处理的问题并尝试修复。
4. Verifier 重新 review 或补充验证结果，判断问题是否真正消失。
5. 当没有重要问题，或问题需要人工接手时，workflow 停止。

当前默认将 `critical` 和 `warning` 视为需要继续处理的重要问题，`suggestion` 不阻止 workflow 收敛。

## Highlights

- 独立的 `FixAgent` Rust 工程，而不是把修复逻辑塞进 review 代码里
- 独立的 `Orchestrator` Rust 服务，统一负责编排、持久化和查询
- PostgreSQL 持久化问题池，数据关系按 `project -> pr/mr -> issue/fix` 建模
- 支持异步 workflow 启动，前端可轮询查看运行中的 round progress
- 提供错误池视图，可区分 `open`、`reopened`、`needs_human`、`resolved`

## Local Development

推荐直接通过 `docker compose` 启动：

```bash
docker compose up
```

默认端口：

- 前端控制台：`http://localhost:5173`
- Orchestrator API：`http://localhost:3000`

如果你在当前开发环境中手动启动服务，也可以分别进入对应目录运行前后端。

## Repository Layout

```text
.
├── ReviewAgent/        # review 引擎 submodule
├── FixAgent/           # 自动修复引擎
├── Orchestrator/       # workflow 编排、数据库持久化、HTTP API
├── web/                # 前端控制台
├── ops/postgres/       # PostgreSQL 初始化脚本
└── docker-compose.yml  # 本地整体启动入口
```

## Current Direction

这个项目当前的目标不是单次 CLI 工具，而是一个更接近正式产品形态的自动修复平台：

- 用户输入一个 PR/MR 地址
- 系统自动发起 workflow
- 前端持续展示运行过程
- 问题池、修复结果和验证结论都可以被追踪与统计

如果你想了解具体实现细节，可以继续查看：

- `FixAgent/README.md`
- `Orchestrator/README.md`
