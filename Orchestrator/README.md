# Orchestrator

独立的 Rust 编排服务，负责把 `project -> pr/mr -> issue/fix` 数据写入 PostgreSQL。

当前版本提供：

- 直接调用 `ReviewAgent` 拉取 PR/MR diff 并完成 review
- 运行最小 workflow：review 后自动 fix 一个 issue
- 运行多轮 workflow，直到没有重要 issue 可继续处理
- 初始化数据库连接
- 从 `ReviewAgent` 结构化 JSON 导入 issues
- 自动 upsert `project` 和 `pr/mr`
- 记录 review run 和 issue 数据
- 领取指定 PR/MR 下的一个 open issue
- 调用 `FixAgent` 执行单问题修复
- 将修复结果写入 `fix_runs`，并更新 `issues.status`
- 记录 fix 的验证结果与证据
- 根据验证结果更新 `issues.status`
- 查询 project / PR / issue 列表
- 查询单个 PR/MR 的统计摘要
- 查询 workflow 历史与轮次详情
- 提供最小 HTTP API，便于前端接入

## 用法

直接运行 review 并入库：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- run-review \
  --repo-dir /workspace \
  --project-key github.com/acme/demo \
  --project-name demo \
  --pr-url https://github.com/acme/demo/pull/123
```

运行最小 workflow：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- run-workflow \
  --repo-dir /workspace \
  --project-key github.com/acme/demo \
  --project-name demo \
  --pr-url https://github.com/acme/demo/pull/123 \
  --dry-run
```

运行自动收敛 workflow：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- run-until-stable \
  --repo-dir /workspace \
  --project-key github.com/acme/demo \
  --project-name demo \
  --pr-url https://github.com/acme/demo/pull/123 \
  --max-rounds 5 \
  --dry-run
```

启动 HTTP API：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- serve-http \
  --host 0.0.0.0 \
  --port 3000
```

当前自动收敛策略：

- 每轮先执行一次 `run-review`
- 然后从 `open`/`reopened` 的重要 issue 中处理一个 issue
- 修复成功后会再次执行 `run-review`，用“同一 issue 指纹是否仍然出现”作为自动 verification 依据
- 重要 issue 当前定义为 `critical` 和 `warning`
- 当只剩下 `suggestion` 或没有可处理的重要 issue 时停止
- 如果某轮修复结果是 `needs_human`，workflow 提前停止
- 如果使用 `--dry-run`，会记录 `not_verifiable_in_current_env`，并停止等待人工或真实执行

手工导入已有 review JSON：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- ingest-review \
  --project-key github.com/acme/demo \
  --project-name demo \
  --platform github \
  --pr-number 123 \
  --pr-url https://github.com/acme/demo/pull/123 \
  --review-file /workspace/report_result.json
```

需要设置：

```bash
export DATABASE_URL=postgres://fixagent:fixagent@localhost:5432/fixagent
```

执行单个 issue 修复：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- run-fix \
  --repo-dir /workspace \
  --project-key github.com/acme/demo \
  --platform github \
  --pr-number 123
```

记录验证结果：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- verify-fix \
  --issue-id 42 \
  --status verified \
  --summary "Static validation passed and the original issue no longer reproduces" \
  --evidence "lint passed\nunit test passed" \
  --next-actions "Monitor in staging"
```

列出所有项目：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- list-projects
```

列出某个项目的 PR/MR：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- list-prs \
  --project-key github.com/acme/demo
```

列出某个 PR/MR 的 issues：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- list-issues \
  --project-key github.com/acme/demo \
  --platform github \
  --pr-number 123
```

查看某个 PR/MR 的统计：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- pr-stats \
  --project-key github.com/acme/demo \
  --platform github \
  --pr-number 123
```

列出 workflow 历史：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- list-workflows
```

按项目过滤 workflow：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- list-workflows \
  --project-key github.com/acme/demo
```

查看单个 workflow 详情：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- workflow-detail \
  --workflow-run-id 1
```

查看单个 workflow 的轮次列表：

```bash
cargo run --manifest-path Orchestrator/Cargo.toml -- workflow-rounds \
  --workflow-run-id 1
```

## HTTP API

健康检查：

```bash
curl http://localhost:3000/health
```

列出项目：

```bash
curl http://localhost:3000/projects
```

列出 PR/MR：

```bash
curl "http://localhost:3000/prs?project_key=github.com/acme/demo"
```

列出 issues：

```bash
curl "http://localhost:3000/issues?project_key=github.com/acme/demo&platform=github&pr_number=123"
```

查看 PR/MR 统计：

```bash
curl "http://localhost:3000/pr-stats?project_key=github.com/acme/demo&platform=github&pr_number=123"
```

列出 workflow 历史：

```bash
curl http://localhost:3000/workflows
```

查看 workflow 详情：

```bash
curl http://localhost:3000/workflows/1
```

触发自动收敛 workflow：

```bash
curl -X POST http://localhost:3000/workflows/run-until-stable \
  -H "Content-Type: application/json" \
  -d '{
    "repo_dir": "/workspace",
    "project_key": "github.com/acme/demo",
    "project_name": "demo",
    "pr_url": "https://github.com/acme/demo/pull/123",
    "claimed_by": "frontend",
    "max_rounds": 5,
    "dry_run": true
  }'
```
