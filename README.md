# FixAgent

一个面向 Pull Request 自动修复流程的工程化系统。

FixAgent 不只是"发现问题"，而是围绕代码审查结果继续推进后续动作：

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

## Quick Start

### Docker Compose（推荐）

```bash
# 复制配置文件
cp orchestrator.toml.example orchestrator.toml

# 编辑 orchestrator.toml，填入 LLM API key
# vim orchestrator.toml

# 启动所有服务
docker compose up --build
```

启动后访问：

- 前端控制台：`http://localhost:5173`
- Orchestrator API：`http://localhost:3000`

### 手动启动（开发模式）

**1. 启动 PostgreSQL**

```bash
docker compose up postgres
```

或使用已有的 PostgreSQL 实例，确保执行了 `ops/postgres/init.sql` 初始化 schema。

**2. 启动 Orchestrator 后端**

```bash
# 方式一：使用配置文件（推荐）
cp orchestrator.toml.example orchestrator.toml
cargo run --manifest-path Orchestrator/Cargo.toml -- serve-http

# 方式二：使用环境变量
export DATABASE_URL=postgres://fixagent:fixagent@localhost:5432/fixagent
cargo run --manifest-path Orchestrator/Cargo.toml -- serve-http
```

**3. 启动前端**

```bash
cd web
npm install
npm run dev
```

前端开发服务器运行在 `http://localhost:5173`，通过 Vite 代理将 `/api` 请求转发到后端。

## Configuration

Orchestrator 使用 `orchestrator.toml` 配置文件，搜索顺序：

1. `./orchestrator.toml`（工作目录）
2. `/etc/fixagent/orchestrator.toml`（容器部署）

```toml
[database]
url = "postgres://fixagent:fixagent@localhost:5432/fixagent"

[server]
host = "0.0.0.0"
port = 3000

[llm]
provider = "openai"
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
```

所有配置项均可通过环境变量覆盖（适用于容器部署）：

| 环境变量 | 对应配置 |
| --- | --- |
| `DATABASE_URL` | `database.url` |
| `SERVER_HOST` / `SERVER_PORT` | `server.host` / `server.port` |
| `LLM_PROVIDER` / `LLM_MODEL` | `llm.provider` / `llm.model` |
| `LLM_BASE_URL` / `LLM_API_KEY` | `llm.base_url` / `llm.api_key` |
| `OPENAI_BASE_URL` / `OPENAI_API_KEY` | 兼容旧配置，等同于 `LLM_BASE_URL` / `LLM_API_KEY` |

## Workflow

系统围绕单个 PR/MR 执行收敛式处理：

1. `ReviewAgent` 分析 PR diff，生成结构化问题列表。
2. `Orchestrator` 将项目、PR、issue、fix、verification 写入 PostgreSQL。
3. `FixAgent` 领取当前最需要处理的问题并尝试修复。
4. Verifier 重新 review 或补充验证结果，判断问题是否真正消失。
5. 当没有重要问题，或问题需要人工接手时，workflow 停止。

当前默认将 `critical` 和 `warning` 视为需要继续处理的重要问题，`suggestion` 不阻止 workflow 收敛。

## HTTP API

Orchestrator 提供以下 REST API：

| Method | Path | Description |
| --- | --- | --- |
| GET | `/health` | 健康检查 |
| GET | `/projects` | 列出所有项目 |
| POST | `/projects` | 创建项目 |
| DELETE | `/projects` | 删除项目（级联删除所有关联数据） |
| GET | `/prs?project_key=...` | 列出项目下的 PR |
| POST | `/prs` | 添加 PR |
| DELETE | `/prs/{pr_id}` | 删除 PR（级联删除关联 issues/fixes/workflows） |
| PATCH | `/prs/{pr_id}/status` | 更新 PR 状态（open / ready_to_merge） |
| POST | `/prs/{pr_id}/fix-all` | 对 PR 下所有 open issue 执行修复 |
| GET | `/issues?...` | 列出 issues（支持按 project/platform/pr_number/status 过滤） |
| PATCH | `/issues/{issue_id}` | 更新 issue 状态 |
| DELETE | `/issues/{issue_id}` | 删除 issue |
| POST | `/issues/{issue_id}/fix` | 对单个 issue 执行修复 |
| GET | `/pr-stats?...` | PR 统计信息 |
| POST | `/reviews` | 发起 review |
| GET | `/workflows?project_key=...` | 列出 workflow 运行记录 |
| POST | `/workflows` | 启动 workflow（review + fix 循环） |
| POST | `/workflows/run-until-stable` | 同步执行 workflow 直到收敛 |
| GET | `/workflows/{id}` | workflow 详情 |
| GET | `/workflows/{id}/rounds` | workflow 轮次列表 |

## Repository Layout

```text
.
├── ReviewAgent/            # review 引擎 submodule
├── FixAgent/               # 自动修复引擎
├── Orchestrator/           # workflow 编排、数据库持久化、HTTP API
│   └── src/
│       ├── main.rs         # 入口
│       ├── config.rs       # 配置加载（orchestrator.toml + env overrides）
│       ├── service.rs      # 核心业务逻辑
│       ├── web.rs          # Axum HTTP handlers
│       ├── git.rs          # Git 操作（commit、push、squash）
│       ├── models.rs       # 数据模型（FromRow）
│       ├── error.rs        # 错误类型
│       ├── db.rs           # 数据库连接与迁移
│       ├── cli.rs          # CLI 参数定义
│       └── lib.rs          # 模块声明
├── web/                    # 前端控制台 (React + Vite + TypeScript)
│   ├── src/
│   │   ├── App.tsx         # 主应用组件
│   │   ├── types.ts        # 类型定义
│   │   ├── api.ts          # API 常量与错误处理
│   │   ├── utils.ts        # 工具函数
│   │   ├── CodeDiffViewer.tsx  # Diff 查看器组件
│   │   ├── styles/         # CSS 模块（base, pr-card, issue-card, modals, diff）
│   │   └── styles.css      # CSS 入口（imports）
│   ├── Dockerfile          # 生产构建（multi-stage: node build + nginx）
│   └── nginx.conf          # Nginx 配置（SPA fallback + API 反代）
├── ops/postgres/           # PostgreSQL 初始化脚本与迁移
│   ├── init.sql            # 初始 schema
│   ├── migrations/         # 增量迁移文件
│   └── sync-schema.sh      # schema 同步脚本
├── orchestrator.toml.example  # 配置文件模板
├── docker-compose.yml      # 本地整体启动入口
└── .monkeycode/            # 项目级文档与规格
```

## Technology Stack

| 层级 | 技术 |
| --- | --- |
| 后端服务 | Rust (axum + sqlx + tokio) |
| 数据库 | PostgreSQL 17 |
| 前端 | React 19 + Vite 7 + TypeScript 5 |
| 生产部署 | Nginx (静态文件 + API 反代) |
| 容器化 | Docker (multi-stage build) + Docker Compose |

## Database Schema

核心表结构：

- `projects` — 项目信息
- `pull_requests` — PR/MR 信息（含 status: open / ready_to_merge）
- `review_runs` — 每次 review 的执行记录
- `issues` — 审查发现的问题（含 `suggestion_code`、`original_code` 用于 diff 展示）
- `fix_runs` — 每次 fix 的执行记录（含 `replacement_preview`、`commit_sha`）
- `verifications` — 修复验证结果
- `workflow_runs` / `workflow_rounds` — workflow 执行历史与轮次

Orchestrator 启动时会自动检查并修复缺失的列，无需手动执行迁移。

## 详细文档

- `FixAgent/README.md` — 自动修复引擎用法与配置
- `Orchestrator/README.md` — 编排服务 CLI 命令与 HTTP API 参考
