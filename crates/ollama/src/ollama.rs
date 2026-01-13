use anyhow::{Context as _, Result};
use futures::{AsyncReadExt, StreamExt, stream::BoxStream};
use http_client::{AsyncBody, HttpClient, HttpRequestExt, Method, Request as HttpRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use settings::KeepAlive;

pub const OLLAMA_API_URL: &str = "http://localhost:11434";

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub name: String,
    pub display_name: Option<String>,
    pub max_tokens: u64,
    pub keep_alive: Option<KeepAlive>,
    pub supports_tools: Option<bool>,
    pub supports_vision: Option<bool>,
    pub supports_thinking: Option<bool>,
}

fn get_max_tokens(name: &str) -> u64 {
    /// Default context length for unknown models.
    const DEFAULT_TOKENS: u64 = 4096;
    /// Magic number. Lets many Ollama models work with ~16GB of ram.
    /// Models that support context beyond 16k such as codestral (32k) or devstral (128k) will be clamped down to 16k
    const MAXIMUM_TOKENS: u64 = 16384;

    match name.split(':').next().unwrap() {
        "granite-code" | "phi" | "tinyllama" => 2048,
        "llama2" | "stablelm2" | "vicuna" | "yi" => 4096,
        "aya" | "codegemma" | "gemma" | "gemma2" | "llama3" | "starcoder" => 8192,
        "codellama" | "starcoder2" => 16384,
        "codestral" | "dolphin-mixtral" | "llava" | "magistral" | "mistral" | "mixstral"
        | "qwen2" | "qwen2.5-coder" => 32768,
        "cogito" | "command-r" | "deepseek-coder-v2" | "deepseek-r1" | "deepseek-v3"
        | "devstral" | "gemma3" | "gpt-oss" | "granite3.3" | "llama3.1" | "llama3.2"
        | "llama3.3" | "mistral-nemo" | "phi3" | "phi3.5" | "phi4" | "qwen3" | "yi-coder" => 128000,
        "qwen3-coder" => 256000,
        _ => DEFAULT_TOKENS,
    }
    .clamp(1, MAXIMUM_TOKENS)
}

impl Model {
    pub fn new(
        name: &str,
        display_name: Option<&str>,
        max_tokens: Option<u64>,
        supports_tools: Option<bool>,
        supports_vision: Option<bool>,
        supports_thinking: Option<bool>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            display_name: display_name
                .map(ToString::to_string)
                .or_else(|| name.strip_suffix(":latest").map(ToString::to_string)),
            max_tokens: max_tokens.unwrap_or_else(|| get_max_tokens(name)),
            keep_alive: Some(KeepAlive::indefinite()),
            supports_tools,
            supports_vision,
            supports_thinking,
        }
    }

    pub fn id(&self) -> &str {
        &self.name
    }

    pub fn display_name(&self) -> &str {
        self.display_name.as_ref().unwrap_or(&self.name)
    }

    pub fn max_token_count(&self) -> u64 {
        self.max_tokens
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    Assistant {
        content: String,
        tool_calls: Option<Vec<OllamaToolCall>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<String>>,
        thinking: Option<String>,
    },
    User {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<String>>,
    },
    System {
        content: String,
    },
    Tool {
        tool_name: String,
        content: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaToolCall {
    // TODO: Remove `Option` after most users have updated to Ollama v0.12.10,
    // which was released on the 4th of November 2025
    pub id: Option<String>,
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct OllamaFunctionTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OllamaTool {
    Function { function: OllamaFunctionTool },
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub keep_alive: KeepAlive,
    pub options: Option<ChatOptions>,
    pub tools: Vec<OllamaTool>,
    pub think: Option<bool>,
}

// https://github.com/ollama/ollama/blob/main/docs/modelfile.md#valid-parameters-and-values
#[derive(Serialize, Default, Debug)]
pub struct ChatOptions {
    pub num_ctx: Option<u64>,
    pub num_predict: Option<isize>,
    pub stop: Option<Vec<String>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponseDelta {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done_reason: Option<String>,
    pub done: bool,
    pub prompt_eval_count: Option<u64>,
    pub eval_count: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModelsResponse {
    pub models: Vec<LocalModelListing>,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModelListing {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    pub details: ModelDetails,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModel {
    pub modelfile: String,
    pub parameters: String,
    pub template: String,
    pub details: ModelDetails,
}

#[derive(Serialize, Deserialize)]
pub struct ModelDetails {
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>,
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Debug)]
pub struct ModelShow {
    pub capabilities: Vec<String>,
    pub context_length: Option<u64>,
    pub architecture: Option<String>,
}

impl<'de> Deserialize<'de> for ModelShow {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ModelShowVisitor;

        impl<'de> Visitor<'de> for ModelShowVisitor {
            type Value = ModelShow;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a ModelShow object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut capabilities: Vec<String> = Vec::new();
                let mut architecture: Option<String> = None;
                let mut context_length: Option<u64> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "capabilities" => {
                            capabilities = map.next_value()?;
                        }
                        "model_info" => {
                            let model_info: Value = map.next_value()?;
                            if let Value::Object(obj) = model_info {
                                architecture = obj
                                    .get("general.architecture")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);

                                if let Some(arch) = &architecture {
                                    context_length = obj
                                        .get(&format!("{}.context_length", arch))
                                        .and_then(|v| v.as_u64());
                                }
                            }
                        }
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                Ok(ModelShow {
                    capabilities,
                    context_length,
                    architecture,
                })
            }
        }

        deserializer.deserialize_map(ModelShowVisitor)
    }
}

impl ModelShow {
    pub fn supports_tools(&self) -> bool {
        // .contains expects &String, which would require an additional allocation
        self.capabilities.iter().any(|v| v == "tools")
    }

    pub fn supports_vision(&self) -> bool {
        self.capabilities.iter().any(|v| v == "vision")
    }

    pub fn supports_thinking(&self) -> bool {
        self.capabilities.iter().any(|v| v == "thinking")
    }
}

// Синхронная функция для создания потока вне async контекста
fn spawn_ollama_reader_thread(addr: String, host: String, request_json: String) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("ollama-stream-reader".to_string())
        .spawn(move || {
                #[cfg(target_os = "linux")]
                {
                    // Логируем TID потока для perf trace
                    let tid = unsafe { libc::syscall(libc::SYS_gettid) };
                    eprintln!("[OLLAMA CONSOLE] Thread started (TID={}), connecting to {}", tid, &addr);
                }
                #[cfg(not(target_os = "linux"))]
                {
                    eprintln!("[OLLAMA CONSOLE] Thread started, connecting to {}", &addr);
                }
                use std::io::{Read, Write};
                use std::net::TcpStream as StdTcpStream;
            
                // Создаем синхронное TCP соединение с теми же настройками, что в test_ollama.rs
                eprintln!("[OLLAMA CONSOLE] Attempting to connect to {}", &addr);
                let mut tcp_stream = match StdTcpStream::connect(&addr) {
                    Ok(stream) => {
                        #[cfg(target_os = "linux")]
                        {
                            use std::os::unix::io::AsRawFd;
                            let fd = stream.as_raw_fd();
                            eprintln!("[OLLAMA CONSOLE] Connected successfully, TCP socket fd={}", fd);
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            eprintln!("[OLLAMA CONSOLE] Connected successfully");
                        }
                        stream.set_nodelay(true).unwrap();
                        
                        // Устанавливаем размеры TCP буферов для более быстрого чтения
                        #[cfg(target_os = "linux")]
                        {
                            use std::os::unix::io::AsRawFd;
                            unsafe {
                                let fd = stream.as_raw_fd();
                                // Увеличиваем размер приемного буфера до 64KB
                                let rcvbuf: libc::c_int = 64 * 1024;
                                let result = libc::setsockopt(
                                    fd,
                                    libc::SOL_SOCKET,
                                    libc::SO_RCVBUF,
                                    &rcvbuf as *const _ as *const libc::c_void,
                                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                                );
                                if result == 0 {
                                    eprintln!("[OLLAMA CONSOLE] Set SO_RCVBUF to {} bytes", rcvbuf);
                                } else {
                                    eprintln!("[OLLAMA CONSOLE] Failed to set SO_RCVBUF: {}", *libc::__errno_location());
                                }
                                
                                // Устанавливаем SO_RCVLOWAT для более быстрого возврата из read()
                                let lowat: libc::c_int = 1; // Минимум 1 байт для возврата
                                let result = libc::setsockopt(
                                    fd,
                                    libc::SOL_SOCKET,
                                    libc::SO_RCVLOWAT,
                                    &lowat as *const _ as *const libc::c_void,
                                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                                );
                                if result == 0 {
                                    eprintln!("[OLLAMA CONSOLE] Set SO_RCVLOWAT to {} bytes", lowat);
                                } else {
                                    eprintln!("[OLLAMA CONSOLE] Failed to set SO_RCVLOWAT: {}", *libc::__errno_location());
                                }
                            }
                        }
                        
                        // Используем блокирующий режим (как в test_ollama.rs) - быстрее чем non-blocking + poll
                        eprintln!("[OLLAMA CONSOLE] Using blocking mode (like test_ollama.rs)");
                        stream
                    }
                    Err(e) => {
                        eprintln!("[OLLAMA CONSOLE] Failed to connect: {}", e);
                        return;
                    }
                };
                
                // Отправляем HTTP запрос синхронно
                eprintln!("[OLLAMA CONSOLE] Sending HTTP request");
                let http_request = format!(
                    "POST /api/chat HTTP/1.1\r\n\
                     Host: {}\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    &host,
                    request_json.len(),
                    request_json
                );
                
                if let Err(e) = tcp_stream.write_all(http_request.as_bytes()) {
                    eprintln!("[OLLAMA CONSOLE] Failed to send request: {}", e);
                    return;
                }
                eprintln!("[OLLAMA CONSOLE] Request sent, flushing");
                if let Err(e) = tcp_stream.flush() {
                    eprintln!("[OLLAMA CONSOLE] Failed to flush: {}", e);
                    return;
                }
                eprintln!("[OLLAMA CONSOLE] Request flushed, reading headers");
                
                // Читаем HTTP заголовки синхронно (блокирующий режим, как в test_ollama.rs)
                let mut response_buffer = String::new();
                let mut buffer = [0u8; 8192];
                loop {
                    match tcp_stream.read(&mut buffer) {
                        Ok(0) => {
                            eprintln!("[OLLAMA CONSOLE] Connection closed before headers");
                            return;
                        }
                        Ok(n) => {
                            response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));
                            if response_buffer.contains("\r\n\r\n") {
                                let parts: Vec<&str> = response_buffer.splitn(2, "\r\n\r\n").collect();
                                response_buffer = parts[1].to_string();
                                eprintln!("[OLLAMA CONSOLE] Headers received, starting to read body");
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("[OLLAMA CONSOLE] Read error: {}", e);
                            return;
                        }
                    }
                }
                
                // Читаем тело ответа построчно синхронно
                let mut buffer = response_buffer;
                let mut count = 0u64;
                let start = std::time::Instant::now();
                let mut read_buffer = [0u8; 256];
                let mut last_read_time = std::time::Instant::now();
                
                // Оптимизация потока без root (Linux)
                #[cfg(target_os = "linux")]
                {
                    unsafe {
                        let thread_id = libc::pthread_self();
                        
                        // 1. Попытка установить CPU affinity - привязываем поток к последнему ядру
                        // Это может помочь избежать конкуренции с другими потоками Zed
                        let cpu_count = libc::sysconf(libc::_SC_NPROCESSORS_ONLN);
                        if cpu_count > 0 {
                            let last_cpu = (cpu_count - 1) as usize;
                            let mut cpu_set = std::mem::zeroed::<libc::cpu_set_t>();
                            libc::CPU_ZERO(&mut cpu_set);
                            libc::CPU_SET(last_cpu, &mut cpu_set);
                            
                            let result = libc::pthread_setaffinity_np(
                                thread_id,
                                std::mem::size_of::<libc::cpu_set_t>(),
                                &cpu_set,
                            );
                            if result == 0 {
                                eprintln!("[OLLAMA CONSOLE] CPU affinity set to CPU {}", last_cpu);
                            } else {
                                eprintln!("[OLLAMA CONSOLE] Failed to set CPU affinity: {} (errno: {})", result, *libc::__errno_location());
                            }
                        }
                        
                        // 2. Попытка установить nice value (может не работать без root для отрицательных значений)
                        // Но попробуем - если не получится, просто продолжим
                        let nice_result = libc::nice(-5);
                        if nice_result >= 0 {
                            eprintln!("[OLLAMA CONSOLE] Nice value set to {}", nice_result);
                        } else {
                            // nice() вернул -1, но это может быть ошибка или успех
                            // Проверяем errno
                            let errno = *libc::__errno_location();
                            if errno == libc::EPERM {
                                eprintln!("[OLLAMA CONSOLE] Cannot set negative nice value without root (expected)");
                            } else {
                                eprintln!("[OLLAMA CONSOLE] Nice value adjustment failed: errno {}", errno);
                            }
                        }
                        
                        // 3. Устанавливаем SCHED_OTHER с приоритетом 0 (по умолчанию, но явно)
                        let mut sched_param = std::mem::zeroed::<libc::sched_param>();
                        sched_param.sched_priority = 0;
                        let result = libc::pthread_setschedparam(thread_id, libc::SCHED_OTHER, &sched_param);
                        if result == 0 {
                            eprintln!("[OLLAMA CONSOLE] Thread scheduling policy set to SCHED_OTHER");
                        } else {
                            eprintln!("[OLLAMA CONSOLE] Failed to set scheduling policy: {}", result);
                        }
                    }
                }
                
                loop {
                // Ищем полную строку в буфере
                if let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();
                    
                    // Пропускаем пустые строки
                    if line.is_empty() {
                        continue;
                    }
                    
                    // Ollama может использовать chunked encoding - пропускаем размер чанка
                    if line.chars().all(|c| c.is_ascii_hexdigit()) {
                        continue;
                    }
                    
                    count += 1;
                    let parse_start = std::time::Instant::now();
                    
                    // Парсим JSON
                    let result: Result<ChatResponseDelta> = match serde_json::from_str(&line) {
                        Ok(delta) => Ok(delta),
                        Err(e) => {
                            eprintln!("[OLLAMA CONSOLE] Failed to parse line #{}: {} (line: {}...)", count, e, line.chars().take(100).collect::<String>());
                            continue;
                        }
                    };
                    let parse_time = parse_start.elapsed();
                    
                    // Выводим в консоль
                    if let Ok(delta) = &result {
                        match &delta.message {
                            crate::ChatMessage::Assistant { content, .. } => {
                                print!("{}", content);
                                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                            }
                            _ => {}
                        }
                    }
                    
                    // Логируем события
                    if count <= 20 || count % 10 == 0 {
                        eprintln!(
                            "[OLLAMA CONSOLE] Chunk #{}: parsed in {}ms (since_start={}ms)",
                            count,
                            parse_time.as_millis(),
                            start.elapsed().as_millis()
                        );
                    }
                    
                    // НЕ отправляем в UI - только консольный вывод
                } else {
                    // Читаем ещё данные из TCP потока синхронно (блокирующий режим, как в test_ollama.rs)
                    let time_since_last_read = last_read_time.elapsed();
                    
                    // Измеряем время до системного вызова
                    let before_syscall = std::time::Instant::now();
                    let syscall_start = std::time::Instant::now();
                    
                    let read_result = tcp_stream.read(&mut read_buffer);
                    
                    let syscall_time = syscall_start.elapsed();
                    let total_time = before_syscall.elapsed();
                    let overhead = (total_time.as_nanos() as i64 - syscall_time.as_nanos() as i64).max(0) as u64;
                    
                    match read_result {
                        Ok(0) => {
                            eprintln!("[OLLAMA CONSOLE] EOF reached after {} chunks", count);
                            break; // EOF
                        }
                        Ok(n) => {
                            last_read_time = std::time::Instant::now();
                            
                            // Логируем детальную информацию о времени чтения
                            if count < 5 || total_time.as_millis() > 5 || time_since_last_read.as_millis() > 10 {
                                eprintln!(
                                    "[OLLAMA CONSOLE] Read {} bytes: total={}ms, syscall={}ms, overhead={}µs (since_start={}ms, waited={}ms since last read)",
                                    n,
                                    total_time.as_millis(),
                                    syscall_time.as_millis(),
                                    overhead / 1000,
                                    start.elapsed().as_millis(),
                                    time_since_last_read.as_millis()
                                );
                            }
                            buffer.push_str(&String::from_utf8_lossy(&read_buffer[..n]));
                        }
                        Err(e) => {
                            eprintln!("[OLLAMA CONSOLE] Read error: {}", e);
                            break;
                        }
                    }
                }
            }
            
                eprintln!("[OLLAMA CONSOLE] Stream finished, total chunks: {}", count);
        })
        .expect("Failed to spawn ollama reader thread")
}

pub async fn stream_chat_completion(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    request: ChatRequest,
) -> Result<BoxStream<'static, Result<ChatResponseDelta>>> {
    // Для локальных запросов используем прямой TCP через smol в отдельном runtime
    // Это обходит проблему с Tokio runtime и планировщиком
    let is_local = api_url.starts_with("http://localhost") 
        || api_url.starts_with("http://127.0.0.1")
        || (api_url.starts_with("http://") && api_url.contains("localhost"));
    
    log::info!("[OLLAMA STREAM] Checking connection: is_local={}, api_key={:?}, api_url={}", is_local, api_key.is_some(), api_url);
    
    if is_local && api_key.is_none() {
        log::info!("[OLLAMA STREAM] Using direct TCP connection in separate smol runtime");
        eprintln!("[OLLAMA CONSOLE] Using direct TCP connection path");
        
        // Парсим URL для получения хоста и порта (синхронно, до async контекста)
        let url = url::Url::parse(api_url)?;
        let host = url.host_str().unwrap_or("localhost").to_string();
        let port = url.port().unwrap_or(11434);
        let addr = format!("{}:{}", host, port);
        
        let request_json = serde_json::to_string(&request)?;
        
        // Создаем поток ВНЕ async контекста - в синхронной функции
        // Это должно помочь избежать влияния планировщика async runtime на поток
        let _thread_handle = spawn_ollama_reader_thread(addr, host, request_json);
        
        // Сохраняем cancel_tx для возможности отмены потока
        // TODO: нужно добавить механизм отмены через возвращаемый stream
        // Пока поток работает независимо и завершится сам при EOF или ошибке
        
        // Возвращаем пустой stream - данные НЕ идут в UI, только в консоль
        // Это полностью отвязывает от UI
        Ok(futures::stream::empty().boxed())
    } else {
        log::info!("[OLLAMA STREAM] Using remote HTTP client path (is_local={}, has_api_key={})", is_local, api_key.is_some());
        eprintln!("[OLLAMA CONSOLE] Using remote HTTP client path");
        // Для удаленных запросов используем обычный HTTP client
        let uri = format!("{api_url}/api/chat");
        let http_request = HttpRequest::builder()
            .method(Method::POST)
            .uri(uri)
            .header("Content-Type", "application/json")
            .when_some(api_key, |builder, api_key| {
                builder.header("Authorization", format!("Bearer {api_key}"))
            })
            .body(AsyncBody::from(serde_json::to_string(&request)?))?;
        
        let mut response = client.send(http_request).await?;
        if response.status().is_success() {
            log::info!("[OLLAMA STREAM] Starting remote stream request");
            let body = response.into_body();
            
            // Используем отдельный smol runtime в отдельном потоке для чтения стрима
            let (tx, rx) = futures::channel::mpsc::unbounded::<Result<ChatResponseDelta>>();
            let mut body = body;
            
            std::thread::spawn(move || {
                smol::block_on(async move {
                    let mut buffer = String::new();
                    let mut _count = 0u64;
                    
                    loop {
                        if let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer = buffer[newline_pos + 1..].to_string();
                            
                            if line.is_empty() {
                                continue;
                            }
                            
                            if line.chars().all(|c| c.is_ascii_hexdigit()) {
                                continue;
                            }
                            
                            _count += 1;
                            let result: Result<ChatResponseDelta> = match serde_json::from_str(&line) {
                                Ok(delta) => Ok(delta),
                                Err(e) => {
                                    log::debug!("[OLLAMA STREAM] Failed to parse line: {} (line: {}...)", e, line.chars().take(100).collect::<String>());
                                    continue;
                                }
                            };
                            
                            if tx.unbounded_send(result).is_err() {
                                break;
                            }
                        } else {
                            let mut chunk = vec![0u8; 256];
                            match body.read(&mut chunk).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                                }
                                Err(e) => {
                                    let _ = tx.unbounded_send(Err(anyhow::anyhow!(e)));
                                    break;
                                }
                            }
                        }
                    }
                });
            });
            
            let stream = rx.map(|result| result);
            Ok(stream.boxed())
        } else {
            let mut body = String::new();
            response.body_mut().read_to_string(&mut body).await?;
            anyhow::bail!(
                "Failed to connect to Ollama API: {} {}",
                response.status(),
                body,
            );
        }
    }
}

/* ЗАКОММЕНТИРОВАННЫЙ MOCK:
        // ЗАГЛУШКА: Используем mock данные с задержкой 3мс через smol timer
        log::info!("[OLLAMA STREAM FAKE] Using mock data with 3ms smol delay");
        
        // Используем stream::unfold как в LMStudio
        let stream = futures::stream::unfold(
            (0u64, std::time::Instant::now()),
            |(mut count, start)| async move {
                if count >= 1000 {
                    log::info!("[OLLAMA STREAM FAKE] Reached 1000 chunks, ending stream");
                    return None;
                }
                
                count += 1;
                let chunk_start = std::time::Instant::now();
                
                // Async задержка 3мс через smol timer
                smol::Timer::after(std::time::Duration::from_millis(3)).await;
                
                // Генерируем мок-данные: JSON строка с символом "A"
                let fake_json = format!(
                    r#"{{"model":"test","created_at":"2024-01-01T00:00:00Z","message":{{"role":"assistant","content":"A"}},"done":false}}"#
                );
                
                let parse_start = std::time::Instant::now();
                let result: Result<ChatResponseDelta> = match serde_json::from_str(&fake_json) {
                    Ok(delta) => Ok(delta),
                    Err(e) => Err(anyhow::anyhow!(e)),
                };
                let parse_time = parse_start.elapsed();
                let chunk_time = chunk_start.elapsed();
                
                // Логируем все события для анализа
                if count <= 20 || count % 10 == 0 {
                    log::info!(
                        "[OLLAMA STREAM FAKE] Chunk #{}: generated in {}ms (parse={}ms, since_start={}ms)",
                        count,
                        chunk_time.as_millis(),
                        parse_time.as_millis(),
                        start.elapsed().as_millis()
                    );
                }
                
                Some((result, (count, start)))
            },
        );
        
        Ok(stream.boxed())
        */
        
        /* ЗАКОММЕНТИРОВАННЫЙ MOCK:
        // ЗАГЛУШКА: Используем архитектуру как в LMStudio, но с mock данными
        log::info!("[OLLAMA STREAM FAKE] Using mock data with LMStudio architecture");
        
        // Используем stream::unfold как в LMStudio
        let stream = futures::stream::unfold(
            (0u64, std::time::Instant::now()),
            |(mut count, start)| async move {
                if count >= 1000 {
                    log::info!("[OLLAMA STREAM FAKE] Reached 1000 chunks, ending stream");
                    return None;
                }
                
                count += 1;
                let chunk_start = std::time::Instant::now();
                
                // Без задержки - генерируем данные мгновенно
                
                // Генерируем мок-данные: JSON строка с символом "A"
                let fake_json = format!(
                    r#"{{"model":"test","created_at":"2024-01-01T00:00:00Z","message":{{"role":"assistant","content":"A"}},"done":false}}"#
                );
                
                let parse_start = std::time::Instant::now();
                let result: Result<ChatResponseDelta> = match serde_json::from_str(&fake_json) {
                    Ok(delta) => Ok(delta),
                    Err(e) => Err(anyhow::anyhow!(e)),
                };
                let parse_time = parse_start.elapsed();
                let chunk_time = chunk_start.elapsed();
                
                // Логируем все события для анализа
                if count <= 20 || count % 10 == 0 {
                    log::info!(
                        "[OLLAMA STREAM FAKE] Chunk #{}: generated in {}ms (parse={}ms, since_start={}ms)",
                        count,
                        chunk_time.as_millis(),
                        parse_time.as_millis(),
                        start.elapsed().as_millis()
                    );
                }
                
                Some((result, (count, start)))
            },
        );
        
        Ok(stream.boxed())
        */

pub async fn get_models(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<LocalModelListing>> {
    let uri = format!("{api_url}/api/tags");
    let request = HttpRequest::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Accept", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::default())?;

    let mut response = client.send(request).await?;

    let mut body = String::new();
    response.body_mut().read_to_string(&mut body).await?;

    anyhow::ensure!(
        response.status().is_success(),
        "Failed to connect to Ollama API: {} {}",
        response.status(),
        body,
    );
    let response: LocalModelsResponse =
        serde_json::from_str(&body).context("Unable to parse Ollama tag listing")?;
    Ok(response.models)
}

/// Fetch details of a model, used to determine model capabilities
pub async fn show_model(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<ModelShow> {
    let uri = format!("{api_url}/api/show");
    let request = HttpRequest::builder()
        .method(Method::POST)
        .uri(uri)
        .header("Content-Type", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::from(
            serde_json::json!({ "model": model }).to_string(),
        ))?;

    let mut response = client.send(request).await?;
    let mut body = String::new();
    response.body_mut().read_to_string(&mut body).await?;

    anyhow::ensure!(
        response.status().is_success(),
        "Failed to connect to Ollama API: {} {}",
        response.status(),
        body,
    );
    let details: ModelShow = serde_json::from_str(body.as_str())?;
    Ok(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_completion() {
        let response = serde_json::json!({
        "model": "llama3.2",
        "created_at": "2023-12-12T14:13:43.416799Z",
        "message": {
            "role": "assistant",
            "content": "Hello! How are you today?"
        },
        "done": true,
        "total_duration": 5191566416u64,
        "load_duration": 2154458,
        "prompt_eval_count": 26,
        "prompt_eval_duration": 383809000,
        "eval_count": 298,
        "eval_duration": 4799921000u64
        });
        let _: ChatResponseDelta = serde_json::from_value(response).unwrap();
    }

    #[test]
    fn parse_streaming_completion() {
        let partial = serde_json::json!({
        "model": "llama3.2",
        "created_at": "2023-08-04T08:52:19.385406455-07:00",
        "message": {
            "role": "assistant",
            "content": "The",
            "images": null
        },
        "done": false
        });

        let _: ChatResponseDelta = serde_json::from_value(partial).unwrap();

        let last = serde_json::json!({
        "model": "llama3.2",
        "created_at": "2023-08-04T19:22:45.499127Z",
        "message": {
            "role": "assistant",
            "content": ""
        },
        "done": true,
        "total_duration": 4883583458u64,
        "load_duration": 1334875,
        "prompt_eval_count": 26,
        "prompt_eval_duration": 342546000,
        "eval_count": 282,
        "eval_duration": 4535599000u64
        });

        let _: ChatResponseDelta = serde_json::from_value(last).unwrap();
    }

    #[test]
    fn parse_tool_call() {
        let response = serde_json::json!({
            "model": "llama3.2:3b",
            "created_at": "2025-04-28T20:02:02.140489Z",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_llama3.2:3b_145155",
                        "function": {
                            "name": "weather",
                            "arguments": {
                                "city": "london",
                            }
                        }
                    }
                ]
            },
            "done_reason": "stop",
            "done": true,
            "total_duration": 2758629166u64,
            "load_duration": 1770059875,
            "prompt_eval_count": 147,
            "prompt_eval_duration": 684637583,
            "eval_count": 16,
            "eval_duration": 302561917,
        });

        let result: ChatResponseDelta = serde_json::from_value(response).unwrap();
        match result.message {
            ChatMessage::Assistant {
                content,
                tool_calls,
                images: _,
                thinking,
            } => {
                assert!(content.is_empty());
                assert!(tool_calls.is_some_and(|v| !v.is_empty()));
                assert!(thinking.is_none());
            }
            _ => panic!("Deserialized wrong role"),
        }
    }

    // Backwards compatibility with Ollama versions prior to v0.12.10 November 2025
    // This test is a copy of `parse_tool_call()` with the `id` field omitted.
    #[test]
    fn parse_tool_call_pre_0_12_10() {
        let response = serde_json::json!({
            "model": "llama3.2:3b",
            "created_at": "2025-04-28T20:02:02.140489Z",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "weather",
                            "arguments": {
                                "city": "london",
                            }
                        }
                    }
                ]
            },
            "done_reason": "stop",
            "done": true,
            "total_duration": 2758629166u64,
            "load_duration": 1770059875,
            "prompt_eval_count": 147,
            "prompt_eval_duration": 684637583,
            "eval_count": 16,
            "eval_duration": 302561917,
        });

        let result: ChatResponseDelta = serde_json::from_value(response).unwrap();
        match result.message {
            ChatMessage::Assistant {
                content,
                tool_calls: Some(tool_calls),
                images: _,
                thinking,
            } => {
                assert!(content.is_empty());
                assert!(thinking.is_none());

                // When the `Option` around `id` is removed, this test should complain
                // and be subsequently deleted in favor of `parse_tool_call()`
                assert!(tool_calls.first().is_some_and(|call| call.id.is_none()))
            }
            _ => panic!("Deserialized wrong role"),
        }
    }

    #[test]
    fn parse_show_model() {
        let response = serde_json::json!({
            "license": "LLAMA 3.2 COMMUNITY LICENSE AGREEMENT...",
            "details": {
                "parent_model": "",
                "format": "gguf",
                "family": "llama",
                "families": ["llama"],
                "parameter_size": "3.2B",
                "quantization_level": "Q4_K_M"
            },
            "model_info": {
                "general.architecture": "llama",
                "general.basename": "Llama-3.2",
                "general.file_type": 15,
                "general.finetune": "Instruct",
                "general.languages": ["en", "de", "fr", "it", "pt", "hi", "es", "th"],
                "general.parameter_count": 3212749888u64,
                "general.quantization_version": 2,
                "general.size_label": "3B",
                "general.tags": ["facebook", "meta", "pytorch", "llama", "llama-3", "text-generation"],
                "general.type": "model",
                "llama.attention.head_count": 24,
                "llama.attention.head_count_kv": 8,
                "llama.attention.key_length": 128,
                "llama.attention.layer_norm_rms_epsilon": 0.00001,
                "llama.attention.value_length": 128,
                "llama.block_count": 28,
                "llama.context_length": 131072,
                "llama.embedding_length": 3072,
                "llama.feed_forward_length": 8192,
                "llama.rope.dimension_count": 128,
                "llama.rope.freq_base": 500000,
                "llama.vocab_size": 128256,
                "tokenizer.ggml.bos_token_id": 128000,
                "tokenizer.ggml.eos_token_id": 128009,
                "tokenizer.ggml.merges": null,
                "tokenizer.ggml.model": "gpt2",
                "tokenizer.ggml.pre": "llama-bpe",
                "tokenizer.ggml.token_type": null,
                "tokenizer.ggml.tokens": null
            },
            "tensors": [
                { "name": "rope_freqs.weight", "type": "F32", "shape": [64] },
                { "name": "token_embd.weight", "type": "Q4_K_S", "shape": [3072, 128256] }
            ],
            "capabilities": ["completion", "tools"],
            "modified_at": "2025-04-29T21:24:41.445877632+03:00"
        });

        let result: ModelShow = serde_json::from_value(response).unwrap();
        assert!(result.supports_tools());
        assert!(result.capabilities.contains(&"tools".to_string()));
        assert!(result.capabilities.contains(&"completion".to_string()));

        assert_eq!(result.architecture, Some("llama".to_string()));
        assert_eq!(result.context_length, Some(131072));
    }

    #[test]
    fn serialize_chat_request_with_images() {
        let base64_image = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

        let request = ChatRequest {
            model: "llava".to_string(),
            messages: vec![ChatMessage::User {
                content: "What do you see in this image?".to_string(),
                images: Some(vec![base64_image.to_string()]),
            }],
            stream: false,
            keep_alive: KeepAlive::default(),
            options: None,
            think: None,
            tools: vec![],
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("images"));
        assert!(serialized.contains(base64_image));
    }

    #[test]
    fn serialize_chat_request_without_images() {
        let request = ChatRequest {
            model: "llama3.2".to_string(),
            messages: vec![ChatMessage::User {
                content: "Hello, world!".to_string(),
                images: None,
            }],
            stream: false,
            keep_alive: KeepAlive::default(),
            options: None,
            think: None,
            tools: vec![],
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(!serialized.contains("images"));
    }

    #[test]
    fn test_json_format_with_images() {
        let base64_image = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

        let request = ChatRequest {
            model: "llava".to_string(),
            messages: vec![ChatMessage::User {
                content: "What do you see?".to_string(),
                images: Some(vec![base64_image.to_string()]),
            }],
            stream: false,
            keep_alive: KeepAlive::default(),
            options: None,
            think: None,
            tools: vec![],
        };

        let serialized = serde_json::to_string(&request).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        let message_images = parsed["messages"][0]["images"].as_array().unwrap();
        assert_eq!(message_images.len(), 1);
        assert_eq!(message_images[0].as_str().unwrap(), base64_image);
    }
}
