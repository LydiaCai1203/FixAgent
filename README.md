# FixAgent

问题终结者。

当前仓库包含：

- `ReviewAgent/`: review 引擎 submodule
- `FixAgent/`: 自动修复引擎
- `Orchestrator/`: workflow 编排、数据库持久化与 HTTP API
- `web/`: 面向产品形态的前端控制台

本地开发入口：

```bash
docker compose up
```

启动后默认端口：

- 前端：`http://localhost:5173`
- Orchestrator API：`http://localhost:3000`
