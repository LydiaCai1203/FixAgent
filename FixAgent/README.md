# FixAgent

独立的 Rust 修复代理，用于消费 `ReviewAgent` 生成的结构化 review JSON，并对单个 issue 执行最小修改。

## 用法

```bash
cargo run --manifest-path FixAgent/Cargo.toml -- run \
  --repo-dir /workspace \
  --review-file /workspace/report_result.json \
  --issue-index 1 \
  --output /workspace/fix_result.json
```

默认会直接写回目标文件。如只想预览结果，可添加 `--dry-run`。

作为库使用时，`FixAgent` 也支持直接接收单个结构化 issue，由 `Orchestrator` 调用执行修复。

## 配置

FixAgent 会优先读取以下配置：

1. `<repo-dir>/.fixagent.toml`
2. `<repo-dir>/fixagent.toml`

示例：

```toml
[fix]
context_lines = 20
max_replacement_lines = 120
```

LLM 配置直接复用 `ReviewAgent` 的 `.reviewagent.toml` / `reviewagent.toml`。
