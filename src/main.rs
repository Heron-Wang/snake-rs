// =============================================================================
// 贪吃蛇游戏 — Rust 版 Web 服务
// =============================================================================
// 使用纯标准库手写 HTTP server，监听 0.0.0.0:8081
// 通过 Cloudflare Tunnel 暴露到 snake.heronwang.cn
//
// 路由:
//   GET /        → 返回贪吃蛇 HTML 页面 (src/index.html)
//   GET /health  → 返回 {"status":"ok"} JSON
//   其他          → 404
//
// 并发模型: 线程池 (std::thread + Arc<Mutex<Receiver>>)
// =============================================================================

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// HTML 页面内容 — 编译时从 src/index.html 引入，无需运行时文件 IO
static INDEX_HTML: &str = include_str!("index.html");

/// 监听地址
const HOST: &str = "0.0.0.0";
const PORT: u16 = 8081;

// =============================================================================
// 线程池 — 简单固定大小线程池，用 mpsc channel 分发任务
// =============================================================================

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// 创建拥有 `size` 个 worker 线程的线程池
    fn new(size: usize) -> ThreadPool {
        assert!(size > 0, "线程池大小必须 > 0");

        let (sender, receiver) = mpsc::channel::<Job>();
        // 用 Arc<Mutex> 包装 receiver，让多个 worker 能共享并互斥访问
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Self::spawn_worker(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// 启动单个 worker 线程 — 循环从 channel 取任务执行
    fn spawn_worker(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            loop {
                // lock receiver 取出下一个任务；channel 关闭时退出
                let job = receiver.lock().unwrap().recv();
                match job {
                    Ok(job) => {
                        // 执行任务
                        job();
                    }
                    Err(_) => {
                        // sender 已断开（线程池正在 drop），worker 退出
                        break;
                    }
                }
            }
            // worker id 在日志中可识别（此处静默退出）
            let _ = id;
        })
    }

    /// 提交一个任务到线程池
    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(ref sender) = self.sender {
            let job = Box::new(f);
            let _ = sender.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // 关闭 sender，通知所有 worker 退出
        drop(self.sender.take());
        // 等待所有 worker 完成
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

// =============================================================================
// HTTP 请求处理
// =============================================================================

/// HTTP 请求方法 + 路径
struct HttpRequest {
    method: String,
    path: String,
}

/// 解析 HTTP 请求的第一行，例如: GET /health HTTP/1.1\r\n
fn parse_request(stream: &TcpStream) -> Option<HttpRequest> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();

    // 读取第一行；读取失败或 EOF 则返回 None
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }

    // 请求行格式: METHOD PATH HTTP/1.1
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    Some(HttpRequest {
        method: parts[0].to_string(),
        path: parts[1].to_string(),
    })
}

/// 构建一个完整的 HTTP 响应 (状态行 + 头 + 空行 + body)
fn build_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {length}\r\n\
         Connection: close\r\n\
         \r\n",
        status = status,
        content_type = content_type,
        length = body.len(),
    );

    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}

/// 处理单个 HTTP 请求 — 路由分发
fn handle_request(stream: &mut TcpStream) {
    // 读取并解析请求
    let request = match parse_request(stream) {
        Some(req) => req,
        None => {
            let body = b"400 Bad Request";
            let resp = build_response("400 Bad Request", "text/plain; charset=utf-8", body);
            let _ = stream.write_all(&resp);
            return;
        }
    };

    // 路由匹配
    let response = match (request.method.as_str(), request.path.as_str()) {
        // 首页 — 返回贪吃蛇 HTML
        ("GET", "/") => {
            let body = INDEX_HTML.as_bytes();
            build_response("200 OK", "text/html; charset=utf-8", body)
        }
        // 健康检查 — 返回 JSON
        ("GET", "/health") => {
            let body = br#"{"status":"ok"}"#;
            build_response("200 OK", "application/json", body)
        }
        // 其他路径 — 404
        _ => {
            let body = b"404 Not Found";
            build_response("404 Not Found", "text/plain; charset=utf-8", body)
        }
    };

    // 发送响应
    if let Err(e) = stream.write_all(&response) {
        eprintln!("⚠️  发送响应失败: {}", e);
    }
}

/// 处理连接 — 消费 stream，在当前线程执行
fn handle_connection(mut stream: TcpStream) {
    // 设置读取超时，防止恶意慢连接占用 worker
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));

    handle_request(&mut stream);

    // 确保所有缓冲数据已写出
    let _ = stream.flush();
    // stream drop 时自动关闭连接
}

// =============================================================================
// 主函数 — 启动 TCP 监听 + 线程池
// =============================================================================

fn main() {
    let addr = format!("{}:{}", HOST, PORT);

    // 绑定 TCP 监听
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("❌ 无法绑定 {addr}: {e}");
        std::process::exit(1);
    });

    // 创建线程池 — worker 数量 = CPU 核心数，至少 4
    let num_workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(4);
    let pool = ThreadPool::new(num_workers);

    println!("🐍 贪吃蛇服务已启动 (Rust): http://localhost:{PORT}");
    println!("   线程池: {num_workers} workers");
    println!("   路由: GET / | GET /health");
    println!("   按 Ctrl+C 停止");

    // 接受连接循环
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // 提交到线程池异步处理
                pool.execute(move || {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                eprintln!("⚠️  接受连接失败: {}", e);
            }
        }
    }

    // listener.incoming() 正常结束（不会发生，除非出错），pool drop 时关闭
    println!("服务已停止");
}
