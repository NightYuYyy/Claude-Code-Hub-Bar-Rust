# Claude Code Hub Bar (Rust)

跨平台的 **Claude Code Hub** 菜单栏 / 托盘监控工具。本项目是原 macOS 专属 SwiftUI 应用的 **Rust 移植版**，使用 [Tauri v2](https://tauri.app) 实现，可在 **Windows 与 macOS** 上原生运行。

> A cross-platform tray monitor for Claude Code Hub. All business logic is a faithful Rust port of the original macOS SwiftUI app; the UI is reproduced 1:1 in a Tauri webview.

## 功能 Features

- **概览 Dashboard** — 今日花费 / 请求数 / 并发会话 / 平均响应 / 错误率，与昨日同期对比，进行中请求实时展示。
- **排行 Leaderboard** — 用户 / 供应商 / 模型维度，支持今日 / 本周 / 本月，含缓存命中率加权汇总。
- **日志 Logs** — 分页请求日志，模型 / 状态码过滤，TPS、缓存命中、耗时，快速通道 (Fast Tier) 标记。
- **供应商 Providers** — 启用切换、熔断状态与重置、分组、限额、成本倍率。
- **缓存重建检测** — 复刻原版的大额缓存掉落识别，托盘呼吸指示。
- **两套主题** — Liquid Glass / Endless Dark。
- **托盘标题与提示** — macOS 显示标题文本，Windows/Linux 折叠进 tooltip。

## 架构 Architecture

```
cch-bar-rust/
├── crates/cch-core/      # 纯 Rust 业务逻辑（无 UI 依赖，单元测试覆盖）
│   ├── models.rs         #   领域模型
│   ├── jsonx.rs          #   宽松 JSON 取值
│   ├── parse.rs          #   行->模型解析、排行聚合、缓存检测
│   ├── format.rs         #   金额/数字/时长/语义化版本格式化
│   ├── api.rs            #   reqwest API 客户端（v1 + legacy actions 回退）
│   └── state.rs          #   MonitorState 状态机 / 托盘快照
├── src-tauri/            # Tauri 外壳（托盘、窗口、命令、后台刷新）
└── frontend/             # 原生 HTML/CSS/JS 还原原版界面
```

`cch-core` 不依赖任何平台 API，已通过 `x86_64-pc-windows-msvc` 交叉编译校验，保证 Windows 可运行。

## 开发 Development

前置：[Rust](https://rustup.rs)、[Tauri CLI](https://tauri.app/start/prerequisites/)（`cargo install tauri-cli --version "^2"` 或 `npm i -g @tauri-apps/cli`）。

```bash
cd cch-bar-rust
cargo test -p cch-core          # 运行核心逻辑单元测试
cargo tauri dev                 # 本地开发运行
cargo tauri build               # 打包当前平台
```

## 打包产物 Build artifacts

GitHub Actions（`.github/workflows/build.yml`）在每次 push / tag 时自动构建：

| 平台 | 产物 |
| --- | --- |
| Windows x64 | NSIS 安装包 `.exe` + 便携版 `CCHBar-portable-x64.exe` |
| macOS arm64 | `.dmg` + `.app.zip` |
| macOS x64 | `.dmg` + `.app.zip` |

推送 `v*` 标签会自动创建 GitHub Release 并附带全部产物。

## 配置 Configuration

首次启动后在 **设置** 窗口填写：

- **CCH 地址** — 例如 `http://localhost:3000`
- **API Key** — 留空则从 `.env` 文件读取（支持 `CCH_API_KEY` / `CCH_TOKEN` / `TOKEN` 等键）
- **刷新间隔**、**主题**、**菜单栏详情**、**检查更新**

## License

MIT
