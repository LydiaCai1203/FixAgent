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

### Docker Compose（推荐）

一键启动所有服务：

```bash
docker compose up
```

默认端口：

- 前端控制台：`http://localhost:5173`
- Orchestrator API：`http://localhost:3000`

### 手动启动

如果你在当前开发环境中手动启动服务，需要分别启动数据库、后端和前端。

**1. 启动 PostgreSQL**

确保数据库已初始化（`ops/postgres/init.sql` 会在容器首次启动时自动执行）。如果数据库已存在但缺少新列，Orchestrator 会在连接时自动运行轻量级迁移修复 schema。

**2. 启动 Orchestrator 后端**

```bash
export DATABASE_URL=postgres://fixagent:fixagent@localhost:5432/fixagent
cargo run --manifest-path Orchestrator/Cargo.toml -- serve-http --host 0.0.0.0 --port 3000
```

**3. 启动前端**

```bash
cd web
npm install
npm run dev
```

前端开发服务器默认运行在 `http://localhost:5173`，并通过 Vite 代理将 `/api` 请求转发到后端。

## Repository Layout

```text
.
├── ReviewAgent/        # review 引擎 submodule
├── FixAgent/           # 自动修复引擎
├── Orchestrator/       # workflow 编排、数据库持久化、HTTP API
├── web/                # 前端控制台 (React + Vite + TypeScript)
├── ops/postgres/       # PostgreSQL 初始化脚本与迁移
│   ├── init.sql        # 初始 schema
│   ├── migrations/     # 增量迁移文件
│   └── sync-schema.sh  # schema 同步脚本
├── docker-compose.yml  # 本地整体启动入口
└── .monkeycode/        # 项目级文档与规格
    ├── MEMORY.md       # 用户偏好与项目知识
    └── specs/          # 功能规格说明书
```

## Technology Stack

| 层级 | 技术 |
| --- | --- |
| 后端服务 | Rust (axum + sqlx + tokio) |
| 数据库 | PostgreSQL 17 |
| 前端 | React 19 + Vite 7 + TypeScript 5 |
| 容器化 | Docker + Docker Compose |

## Database Schema

核心表结构：

- `projects` — 项目信息
- `pull_requests` — PR/MR 信息
- `review_runs` — 每次 review 的执行记录
- `issues` — 审查发现的问题（含 `suggestion_code` 建议修复代码）
- `fix_runs` — 每次 fix 的执行记录
- `verifications` — 修复验证结果
- `workflow_runs` / `workflow_rounds` — workflow 执行历史与轮次

Orchestrator 启动时会自动检查并修复缺失的列（如 `suggestion_code`），无需手动执行迁移。

## Current Direction

这个项目当前的目标不是单次 CLI 工具，而是一个更接近正式产品形态的自动修复平台：

- 用户输入一个 PR/MR 地址
- 系统自动发起 workflow
- 前端持续展示运行过程
- 问题池、修复结果和验证结论都可以被追踪与统计

## 详细文档

- `FixAgent/README.md` — 自动修复引擎用法与配置
- `Orchestrator/README.md` — 编排服务 CLI 命令与 HTTP API 参考
