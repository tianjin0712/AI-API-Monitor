//! HTTP 层 Mock 合约测试基础设施（仅测试构建）。
//!
//! 在 `127.0.0.1` 上启动一个脚本化 HTTP 服务器：逐连接按脚本返回预设响应
//! （状态码 / 响应头 / 响应体 / 延迟 / 直接断连），并记录收到的请求行，
//! 用于验证各 Provider 适配器在真实 `reqwest` 请求下的行为
//! （成功、401/403、429、5xx、非 JSON、超时、网络中断、分页）。
//!
//! 响应可绑定 `match_path`（请求路径包含该子串才匹配），使并发请求
//! （如 Claude/OpenAI 同时请求 usage 与 cost 端点）与分页请求
//! （第二页 URL 含 `page=<cursor>`）可以按路径顺序无关地正确配对。

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// 脚本化响应。
#[derive(Clone, Debug)]
pub struct MockResponse {
    /// HTTP 状态码（`drop_connection` 时忽略）。
    pub status: u16,
    /// 附加响应头（`content-length`/`connection` 自动补充）。
    pub headers: Vec<(String, String)>,
    /// 响应体（按原样字节输出）。
    pub body: String,
    /// 仅当请求路径包含该子串时匹配；`None` = 匹配下一个未绑定的连接。
    /// 用于区分并发端点（usage_report / cost_report）与分页页次（page=<cursor>）。
    pub match_path: Option<String>,
    /// 响应前延迟；超过客户端超时即可触发客户端超时测试。
    pub delay: Option<Duration>,
    /// `true` = 收到请求后立即断开（模拟网络中断/服务器提前关闭）。
    pub drop_connection: bool,
}

impl MockResponse {
    /// JSON 响应（默认 `content-type: application/json`）。
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.into(),
            match_path: None,
            delay: None,
            drop_connection: false,
        }
    }

    /// 绑定到请求路径包含 `path` 的请求。
    pub fn for_path(mut self, path: impl Into<String>) -> Self {
        self.match_path = Some(path.into());
        self
    }

    /// 延迟 `delay` 后再响应（用于客户端超时测试）。
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// 收到请求后立即断开连接，模拟网络中断。
    pub fn drop_connection() -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: String::new(),
            match_path: None,
            delay: None,
            drop_connection: true,
        }
    }

    fn reason(&self) -> &'static str {
        match self.status {
            200 => "OK",
            201 => "Created",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            _ => "",
        }
    }
}

/// 本地脚本化 Mock 服务器。
pub struct MockServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MockServer {
    /// 启动服务器并按脚本顺序消费响应；脚本耗尽后对剩余请求返回 404。
    pub fn start(script: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        listener.set_nonblocking(true).expect("set nonblocking");
        let addr = listener.local_addr().expect("local addr");
        let script = Arc::new(Mutex::new(VecDeque::from(script)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let script = Arc::clone(&script);
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let script = Arc::clone(&script);
                            let requests = Arc::clone(&requests);
                            std::thread::spawn(move || handle_connection(stream, script, requests));
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            })
        };
        Self {
            addr,
            stop,
            handle: Some(handle),
            requests,
        }
    }

    /// Base URL，例如 `http://127.0.0.1:52341`（无尾斜杠）。
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// 已收到的请求行（如 `GET /user/balance HTTP/1.1`），按连接到达顺序。
    pub fn request_lines(&self) -> Vec<String> {
        self.requests
            .lock()
            .expect("requests lock")
            .iter()
            .map(|head| head.lines().next().unwrap_or_default().to_string())
            .collect()
    }

    /// 已收到的完整请求头（请求行 + headers），按连接到达顺序。
    pub fn request_heads(&self) -> Vec<String> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    script: Arc<Mutex<VecDeque<MockResponse>>>,
    requests: Arc<Mutex<Vec<String>>>,
) {
    let request_head = read_request_head(&mut stream);
    if let Some(head) = &request_head {
        requests.lock().expect("requests lock").push(head.clone());
    }
    let path = request_head
        .as_deref()
        .and_then(|head| {
            head.lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
        })
        .unwrap_or("")
        .to_string();

    let response = {
        let mut queue = script.lock().expect("script lock");
        // 断连脚本按 match_path 匹配（None = 匹配任意连接）；普通响应同理。
        // 并发/分页场景中应给断连脚本绑定 match_path，避免它抢走其它端点的连接。
        let index = queue.iter().position(|r| {
            r.drop_connection && r.match_path.as_deref().is_none_or(|m| path.contains(m))
        });
        let index = index.or_else(|| {
            queue.iter().position(|r| {
                !r.drop_connection && r.match_path.as_deref().is_none_or(|m| path.contains(m))
            })
        });
        match index {
            Some(index) => queue.remove(index).expect("indexed response"),
            None => MockResponse::json(404, "{}"),
        }
    };

    if response.drop_connection {
        drop(stream);
        return;
    }

    if let Some(delay) = response.delay {
        std::thread::sleep(delay);
    }

    let body = response.body.as_bytes();
    let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, response.reason());
    head.push_str(&format!("content-length: {}\r\n", body.len()));
    head.push_str("connection: close\r\n");
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    drop(stream);
}

/// 读取请求起始行与头（到 `\r\n\r\n` 或超时/EOF），返回请求行。
fn read_request_head(stream: &mut TcpStream) -> Option<String> {
    // 显式恢复阻塞模式：非阻塞监听 accept 出的连接可能继承非阻塞标志，
    // 导致首次 read 立即返回 WouldBlock 而误判为“无请求”。
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buffer = [0u8; 2048];
    let mut head = Vec::new();
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                head.extend_from_slice(&buffer[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            // 阻塞模式下 WouldBlock 不应出现；出现则短暂让出后继续等待。
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
    if head.is_empty() {
        return None;
    }
    String::from_utf8(head)
        .ok()
        .map(|text| text.trim_end_matches("\r\n").to_string())
}
