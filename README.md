# 🐍 snake-rs — 用 Rust 标准库手写的贪吃蛇 Web 游戏

> 零第三方依赖，纯 `std` 实现 HTTP 服务器、线程池与游戏页面，一条命令编译部署。

---

## 项目背景

`snake-rs` 是一个学习与实践项目，目标是用 **Rust 标准库且仅用标准库** 从零搭建一个可玩的贪吃蛇 Web 服务——不依赖 `axum`、`tokio`、`hyper` 等任何第三方 crate。

整个项目只包含一个 Rust 源文件 `src/main.rs`，其中手写实现了：

- **HTTP/1.1 请求解析与响应构建**（解析请求行、组装状态行+头部+正文）
- **固定大小线程池**（`std::thread` + `mpsc::channel` + `Arc<Mutex<Receiver>>` 分发任务）
- **路由分发**（`GET /` 返回游戏页面，`GET /health` 返回健康检查 JSON）
- **编译时资源嵌入**（通过 `include_str!` 将 HTML 打包进二进制，运行时无文件 IO）

游戏前端用原生 HTML5 Canvas 绘制，单文件内嵌 CSS/JS，随服务器一并编译进二进制。部署时只需一个可执行文件 + 一个 systemd unit，通过 Cloudflare Tunnel 暴露到公网域名 `snake.heronwang.cn`。

---

## 项目结构

```
snake-rs/
├── Cargo.toml              # 包配置（edition 2021，无任何依赖）
├── Cargo.lock
├── README.md               # 本文件
├── snake-rs.service        # systemd 服务单元文件
├── src/
│   ├── main.rs             # 全部后端逻辑：HTTP server + 线程池 + 路由（~245 行）
│   ├── index.html          # 游戏前端：Canvas 贪吃蛇 + 响应式布局 + 触摸控制
│   └── favicon.svg         # 站点图标：手绘风格鹭鸟 SVG
└── target/                 # 构建产物（release 二进制）
    └── release/
        └── snake-rs        # 编译后的单一可执行文件
```

**核心文件说明**

| 文件 | 职责 |
|------|------|
| `src/main.rs` | TCP 监听 (`0.0.0.0:8081`)、线程池实现、HTTP 请求解析、路由匹配、响应构建 |
| `src/index.html` | 游戏页面：20×20 网格 Canvas、渐变蛇身、发光食物、键盘/滑动/虚拟方向键控制、高分本地存储 |
| `snake-rs.service` | systemd 服务定义，`Restart=always`，开机自启 |

---

## 快速启动

### 前置要求

- Rust 工具链（`rustc` + `cargo`，edition 2021）
- Linux 环境（部署使用 systemd）

### 本地开发

```bash
# 进入项目目录
cd snake-rs

# Debug 构建 + 运行
cargo run

# Release 构建（opt-level = 2）
cargo build --release

# 直接运行 release 二进制
./target/release/snake-rs
```

启动后控制台输出：

```
🐍 贪吃蛇服务已启动 (Rust): http://localhost:8081
   线程池: N workers          # N = CPU 核心数（至少 4）
   路由: GET / | GET /health
   按 Ctrl+C 停止
```

浏览器访问 **http://localhost:8081** 即可开始游戏。

### 生产部署（systemd）

```bash
# 1. Release 构建
cargo build --release

# 2. 安装 systemd 服务（需 root/sudo）
sudo cp snake-rs.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now snake-rs

# 3. 查看状态
sudo systemctl status snake-rs
```

服务配置要点（见 `snake-rs.service`）：

- 以 `heron` 用户运行，工作目录 `/home/heron/workspace/snake-rs`
- `ExecStart` 指向 release 二进制
- `Restart=always` + `RestartSec=5`：崩溃后自动重启
- 通过 Cloudflare Tunnel 暴露到 **https://snake.heronwang.cn**

### 路由一览

| 方法 | 路径 | 响应 |
|------|------|------|
| `GET` | `/` | 贪吃蛇 HTML 页面（`text/html`） |
| `GET` | `/health` | `{"status":"ok"}`（`application/json`） |
| * | * | `404 Not Found` |

---

## 演示效果

访问 **https://snake.heronwang.cn**（或本地 `http://localhost:8081`），页面加载后显示带霓虹渐变标题的贪吃蛇游戏界面，覆盖一层半透明启动遮罩，点击 **Start Game** 即可开始。

### 游戏玩法

- **目标**：控制蛇移动，吃掉地图上随机生成的粉色发光食物，每吃一个得 **10 分**，蛇身增长一节。
- **操作方式**：
  - 🖥️ **桌面端**：方向键 `↑↓←→` 或 `WASD` 控制方向；`空格键` 暂停/继续。
  - 📱 **移动端**：在画布上**滑动**转向，或使用屏幕底部的**虚拟方向键**（D-pad）。
- **规则**：
  - 蛇头撞墙或撞到自身 → 游戏结束。
  - 不能 180° 反向掉头（防止直接撞自己）。
  - 高分自动保存在浏览器 `localStorage`，刷新不丢失；破纪录时显示 🏆 New Record。
- **视觉细节**：
  - 20×20 网格，蛇身从蛇头的青色（`#00f5d4`）到尾部渐变为紫色，头部带辉光。
  - 食物为粉色（`#f15bb5`）发光圆点。
  - 深色渐变背景（`#0f0f23 → #1a1a3e → #2d1b4e`），整体霓虹科技风。
  - 画布尺寸响应式自适应，移动端自动显示虚拟方向键并隐藏键盘提示。

### 并发架构

服务器采用经典的 **线程池 + mpsc channel** 并发模型：

```
TcpListener.incoming()
       │
       ▼
  ThreadPool.execute(job)  ──►  mpsc::Sender<Job>
                                    │
                   ┌────────────────┴────────────────┐
                   ▼                  ▼              ▼
              Worker 0            Worker 1       Worker N
          (lock receiver)     (lock receiver)  (lock receiver)
                   │                  │              │
                   ▼                  ▼              ▼
            handle_connection   handle_connection  ...
```

- `Arc<Mutex<mpsc::Receiver<Job>>>` 让多个 worker 互斥地从同一个 channel 取任务。
- Worker 数量 = `available_parallelism()`（CPU 核心数），最少 4 个。
- 每个 worker 循环 `recv()`，channel 关闭（`Drop` 时 `drop(sender)`）时自动退出。
- 每个 TCP 连接设置 5 秒读取超时，防止慢连接占用 worker。

---

**技术栈**：Rust 标准库 · HTML5 Canvas · systemd · Cloudflare Tunnel
