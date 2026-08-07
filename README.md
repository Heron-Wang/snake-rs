# 🐍 snake-rs

Rust 版贪吃蛇 Web 游戏服务 — 纯标准库手写 HTTP server，无第三方依赖。

## 功能

- 🎮 完整贪吃蛇游戏（键盘 + WASD + 触屏滑动 + 虚拟方向键）
- 📱 响应式画布，移动端自适应
- 🌈 深色渐变主题（cyan → blue → purple）
- 🏆 本地最高分记录（localStorage）
- ⚡ 线程池并发处理

## 路由

| 方法 | 路径 | 说明 |
|------|------|------|
| GET  | `/` | 贪吃蛇游戏 HTML 页面 |
| GET  | `/health` | `{"status":"ok"}` 健康检查 |
| *    | * | 404 Not Found |

## 运行

```bash
cd /home/heron/projects/snake-rs

# 编译
cargo build --release

# 运行
cargo run --release

# 或直接运行编译后的二进制
./target/release/snake-rs
```

服务监听 `0.0.0.0:8081`，通过 Cloudflare Tunnel 暴露到 `snake.heronwang.cn`。

## 项目结构

```
snake-rs/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs       # Rust HTTP 服务（线程池 + 路由）
    └── index.html    # 贪吃蛇游戏前端（编译时 include_str! 引入）
```

## 技术细节

- **HTTP server**: `std::net::TcpListener` 手写，无第三方框架
- **并发**: 固定大小线程池（`std::thread` + `mpsc` channel 分发任务）
- **HTML 引入**: `include_str!("index.html")` 编译时嵌入，零运行时文件 IO
- **Worker 数量**: `std::thread::available_parallelism()` 自动检测 CPU 核心数（至少 4）

## 与 Python 版对比

| 特性 | Python (`server.py`) | Rust (`snake-rs`) |
|------|---------------------|-------------------|
| HTTP server | `http.server.HTTPServer` | `std::net::TcpListener` 手写 |
| 并发 | 单线程 | 线程池（多 worker） |
| HTML 来源 | 运行时字符串 | 编译时 `include_str!` |
| 二进制 | 需 Python 解释器 | 单文件原生二进制 |
| 性能 | ~中 | 高 |

前端 HTML 与 Python 版完全一致，仅 footer 标注由 `Python 3` 改为 `Rust`。
