# FixAgent 部署文档

## 项目概述

FixAgent 是一个面向 Pull Request 自动修复流程的工程化系统。围绕代码审查结果推进后续动作：接收 ReviewAgent 生成的 review 结果，将 issue、fix、verification 持久化到数据库，以 workflow 方式持续执行 `review -> fix -> verify`，通过前端控制台实时展示运行状态、轮次进度与错误池。

## 架构说明

系统由以下模块组成：

| Module | Role | 技术栈 |
| --- | --- | --- |
| `ReviewAgent/` | review 引擎 submodule，负责生成 PR/MR 审查结果 | Rust |
| `FixAgent/` | 自动修复引擎，负责消费 issue 并生成或应用修复 | Rust |
| `Orchestrator/` | workflow 编排层，负责数据库持久化、任务推进与 HTTP API | Rust |
| `web/` | 前端控制台，负责发起 workflow、查看实时状态和错误池 | React + Vite |
| `PostgreSQL` | 持久化问题池，数据关系按 `project -> pr/mr -> issue/fix` 建模 | PostgreSQL 17 |

### 架构图

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Web UI     │────>│ Orchestrator │────>│  FixAgent    │
│  React+Vite  │     │  Rust/HTTP   │     │  Rust        │
└──────────────┘     └──────┬───────┘     └──────────────┘
                            │
                            v
                     ┌──────────────┐     ┌──────────────┐
                     │  PostgreSQL  │<────│ ReviewAgent  │
                     │    17        │     │  Rust        │
                     └──────────────┘     └──────────────┘
```

## 环境要求

### 最低要求

- **Docker Compose 部署**：Docker 20.10+ / Docker Compose 2.0+
- **本地开发部署**：
  - Rust 1.92+
  - Node.js 22+
  - PostgreSQL 17
  - 内存：建议 4GB+

## 部署方式

### 方式一：Docker Compose 部署（推荐）

适用于快速部署和生产环境。

#### 1. 克隆项目

```bash
git clone <repository-url> fixagent
cd fixagent
```

#### 2. 初始化 Submodule

```bash
git submodule update --init --recursive
```

#### 3. 配置环境变量

复制环境变量模板：

```bash
cp .env.example .env
```

编辑 `.env` 文件：

```bash
# 数据库连接
DATABASE_URL=postgres://fixagent:fixagent@localhost:5432/fixagent

# LLM API 配置（OpenAI 兼容接口）
OPENAI_BASE_URL=https://api.deepseek.com/v1
OPENAI_API_KEY=your-api-key-here
MODEL=deepseek-v4-pro
```

#### 4. 启动服务

```bash
docker compose up -d --build
```

启动后会自动创建以下服务：

| 服务 | 端口 | 说明 |
| --- | --- | --- |
| postgres | 5432 | PostgreSQL 数据库 |
| orchestrator | 3000 | API 服务 |
| web | 5173 | 前端控制台 |

#### 5. 验证部署

```bash
# 检查服务状态
docker compose ps

# 检查健康状态
curl http://localhost:3000/health

# 访问前端
open http://localhost:5173
```

#### 6. 查看日志

```bash
# 查看所有服务日志
docker compose logs -f

# 查看特定服务日志
docker compose logs -f orchestrator
docker compose logs -f web
docker compose logs -f postgres
```

### 方式二：本地开发部署

适用于开发调试。

#### 1. 安装依赖

**Rust 工具链**：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**Node.js**：

```bash
# 使用 nvm 安装
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
source ~/.bashrc
nvm install 22
```

**PostgreSQL**：

```bash
# Ubuntu/Debian
sudo apt-get install -y postgresql

# macOS
brew install postgresql@17
```

#### 2. 初始化数据库

```bash
# 启动 PostgreSQL 服务
sudo systemctl start postgresql

# 创建数据库和用户
sudo -u postgres psql -c "CREATE USER fixagent WITH PASSWORD 'fixagent';"
sudo -u postgres psql -c "CREATE DATABASE fixagent OWNER fixagent;"
sudo -u postgres psql -d fixagent -f ops/postgres/init.sql
```

#### 3. 编译 Rust 服务

```bash
# 编译 Orchestrator（包含 ReviewAgent 和 FixAgent）
cargo build --release --manifest-path Orchestrator/Cargo.toml
```

编译产物位于：`Orchestrator/target/release/orchestrator`

#### 4. 安装前端依赖

```bash
cd web
npm install
```

#### 5. 配置环境变量

```bash
# 根目录 .env 文件
cat > .env << EOF
DATABASE_URL=postgres://fixagent:fixagent@localhost:5432/fixagent
OPENAI_BASE_URL=https://api.deepseek.com/v1
OPENAI_API_KEY=your-api-key-here
MODEL=deepseek-v4-pro
EOF
```

#### 6. 启动服务

**启动 Orchestrator**：

```bash
source .env
export DATABASE_URL OPENAI_BASE_URL OPENAI_API_KEY MODEL RUST_LOG=info
./Orchestrator/target/release/orchestrator serve-http --host 0.0.0.0 --port 3000
```

**启动前端**：

```bash
cd web
npm run dev -- --host 0.0.0.0 --port 5173
```

**注意**：前端开发服务器不要设置 `VITE_API_BASE_URL`，让它使用 Vite 代理转发到后端。代理配置已在 `vite.config.ts` 中设置。

## 环境变量说明

### Orchestrator 环境变量

| 变量名 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `DATABASE_URL` | 是 | - | PostgreSQL 连接字符串 |
| `OPENAI_BASE_URL` | 否 | `https://api.openai.com/v1` | OpenAI 兼容 API 基础 URL |
| `OPENAI_API_KEY` | 是 | - | API 密钥 |
| `MODEL` | 是 | - | 模型名称 |
| `RUST_LOG` | 否 | `info` | 日志级别 |

### Web 环境变量

| 变量名 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `VITE_API_BASE_URL` | 否 | `${window.location.origin}/api` | API 基础 URL（开发时留空使用代理） |

### 重要提示

1. **`OPENAI_BASE_URL` vs `BASE_URL`**：ReviewAgent 使用 OpenAI SDK，需要 `OPENAI_BASE_URL` 而不是 `BASE_URL`
2. **前端代理**：开发模式下，前端通过 Vite 代理 `/api` 请求到后端，不要设置 `VITE_API_BASE_URL`
3. **生产部署**：生产环境应使用独立的 Web 服务器（如 Nginx）代理前端和后端

## 数据库说明

### 数据表结构

系统使用以下核心表：

| 表名 | 说明 |
| --- | --- |
| `projects` | 项目信息 |
| `pull_requests` | PR/MR 信息 |
| `review_runs` | 审查运行记录 |
| `issues` | 审查发现的问题 |
| `fix_runs` | 修复运行记录 |
| `verifications` | 验证记录 |
| `workflow_runs` | Workflow 运行记录 |
| `workflow_rounds` | Workflow 轮次记录 |

### 数据库初始化

首次启动时，PostgreSQL 容器会自动执行：

- `ops/postgres/init.sql` - 创建所有表和索引
- `ops/postgres/sync-schema.sh` - 同步数据库 schema（确保列存在）

### 数据持久化

PostgreSQL 数据存储在 Docker 卷 `postgres-data` 中：

```bash
# 查看卷信息
docker volume ls | grep postgres-data

# 备份数据
docker run --rm -v postgres-data:/data -v $(pwd):/backup alpine tar czf /backup/postgres-backup.tar.gz -C /data .

# 恢复数据
docker run --rm -v postgres-data:/data -v $(pwd):/backup alpine tar xzf /backup/postgres-backup.tar.gz -C /data
```

## 常见问题排查

### 1. 前端无数据

**问题**：访问前端控制台，页面显示但无数据。

**原因**：`VITE_API_BASE_URL` 被设置为 `http://localhost:3000/api`，浏览器端请求的是用户本机的 localhost。

**解决方案**：

```bash
# 开发模式下不要设置 VITE_API_BASE_URL
cd web
npm run dev -- --host 0.0.0.0 --port 5173
```

前端会使用 `window.location.origin/api` 作为 API 地址，通过 Vite 代理转发到后端。

### 2. ReviewAgent 报连接错误

**问题**：日志显示 `tls handshake eof` 或连接到 `api.openai.com`。

**原因**：环境变量使用了 `BASE_URL` 而不是 `OPENAI_BASE_URL`。

**解决方案**：

```bash
# 正确配置
OPENAI_BASE_URL=https://api.deepseek.com/v1
OPENAI_API_KEY=your-api-key-here

# 错误配置（不会被识别）
BASE_URL=https://api.deepseek.com/v1
```

### 3. 数据库连接失败

**问题**：Orchestrator 启动失败，显示数据库连接错误。

**检查步骤**：

```bash
# Docker 模式：检查 postgres 是否健康
docker compose ps

# 本地模式：检查 PostgreSQL 是否运行
pg_isready -h localhost -p 5432

# 测试连接
PGPASSWORD=fixagent psql -h localhost -U fixagent -d fixagent -c "SELECT 1"
```

### 4. 编译失败

**问题**：`cargo build` 失败，提示找不到符号。

**解决方案**：

```bash
# 清理并重新编译
cargo clean
cargo build --release --manifest-path Orchestrator/Cargo.toml
```

### 5. 端口冲突

**问题**：端口 5173 或 3000 已被占用。

**解决方案**：

```bash
# 查看占用端口的进程
lsof -i :5173
lsof -i :3000

# 终止进程
kill -9 <PID>

# 或使用其他端口
npm run dev -- --host 0.0.0.0 --port 5174
```

## 生产部署建议

### 1. 使用反向代理

使用 Nginx 统一代理前端和后端：

```nginx
server {
    listen 80;
    server_name fixagent.example.com;

    location /api/ {
        proxy_pass http://localhost:3000/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location / {
        proxy_pass http://localhost:5173/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 2. 数据库安全

- 修改默认密码
- 限制数据库访问 IP
- 启用 SSL 连接
- 定期备份数据

### 3. 日志管理

```bash
# 配置日志轮转
# /etc/logrotate.d/fixagent
/var/log/fixagent/*.log {
    daily
    rotate 30
    compress
    missingok
    notifempty
}
```

### 4. 监控告警

- 使用 Prometheus 监控服务健康
- 配置 Grafana 仪表盘
- 设置关键错误告警

## 更新与升级

### 更新代码

```bash
git pull
git submodule update --init --recursive
```

### 重新构建

```bash
# Docker 模式
docker compose down
docker compose up -d --build

# 本地模式
cargo build --release --manifest-path Orchestrator/Cargo.toml
cd web && npm install && npm run build
```

### 数据库迁移

```bash
# 应用新迁移
./ops/postgres/sync-schema.sh
```
