// Прямое чтение из TCP сокета для избежания межрантаймовой задержки
// Используется только для локальных запросов к Ollama
// Используем синхронный std::net::TcpStream в отдельном потоке с каналом
// чтобы избежать задержки от async планировщика (как в test_ollama2)

use anyhow::Result;
use futures::{StreamExt, stream::BoxStream};
use smol::channel;
use std::io::{Read, Write};
use std::net::TcpStream as StdTcpStream;
use std::thread;

use crate::{ChatRequest, ChatResponseDelta};

pub async fn stream_chat_completion_direct(
    api_url: &str,
    _api_key: Option<&str>,
    request: ChatRequest,
) -> Result<BoxStream<'static, Result<ChatResponseDelta>>> {
    log::info!("[OLLAMA DIRECT] Using direct TCP connection to {}", api_url);
    
    // Парсим URL для получения хоста и порта
    let url = url::Url::parse(api_url)?;
    let host = url.host_str().unwrap_or("localhost").to_string();
    let port = url.port().unwrap_or(11434);
    let addr = format!("{}:{}", host, port);

    let request_json = serde_json::to_string(&request)?;
    
    // Создаем синхронное соединение в отдельном потоке
    let connect_start = std::time::Instant::now();
    let mut tcp_stream = StdTcpStream::connect(&addr)?;
    tcp_stream.set_nodelay(true)?;
    log::info!("[OLLAMA DIRECT] Connected in {}ms", connect_start.elapsed().as_millis());
    
    // Отправляем HTTP запрос синхронно
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

    let write_start = std::time::Instant::now();
    tcp_stream.write_all(http_request.as_bytes())?;
    tcp_stream.flush()?;
    log::info!("[OLLAMA DIRECT] Request sent in {}ms", write_start.elapsed().as_millis());

    // Читаем HTTP заголовки синхронно
    let mut response_buffer = String::new();
    let mut buffer = [0u8; 8192];
    let headers_start = std::time::Instant::now();
    loop {
        let n = tcp_stream.read(&mut buffer)?;
        if n == 0 {
            anyhow::bail!("Connection closed before headers");
        }
        
        response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));
        if response_buffer.contains("\r\n\r\n") {
            let parts: Vec<&str> = response_buffer.splitn(2, "\r\n\r\n").collect();
            response_buffer = parts[1].to_string();
            log::info!("[OLLAMA DIRECT] Headers received in {}ms", headers_start.elapsed().as_millis());
            break;
        }
    }
    
    // Создаем канал для передачи данных из отдельного потока
    let (tx, rx) = channel::unbounded::<Result<Vec<u8>>>();
    
    // Запускаем отдельный поток для чтения (как в test_ollama2)
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match tcp_stream.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.try_send(Ok(vec![])); // EOF
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    if tx.try_send(Ok(data)).is_err() {
                        break; // Получатель закрыт
                    }
                }
                Err(e) => {
                    let _ = tx.try_send(Err(anyhow::anyhow!(e)));
                    break;
                }
            }
        }
    });

    let start_time = std::time::Instant::now();
    let chunk_count = std::sync::atomic::AtomicU64::new(0);
    let chunk_count = std::sync::Arc::new(chunk_count);
    let chunk_count_clone = chunk_count.clone();
    
    // Используем stream::unfold для чтения из канала (данные приходят из отдельного потока)
    Ok(futures::stream::unfold(
        (rx, response_buffer, start_time, chunk_count_clone),
        |(rx, mut buffer, start_time, chunk_count)| async move {
            loop {
                // Обрабатываем все полные строки в буфере
                if let Some(newline_pos) = buffer.find('\n') {
                    let parse_start = std::time::Instant::now();
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    // Ollama может использовать chunked encoding - пропускаем размер чанка
                    if line.chars().all(|c| c.is_ascii_hexdigit()) {
                        continue;
                    }

                    let current_count = chunk_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    
                    // Парсим JSON
                    let json_start = std::time::Instant::now();
                    match serde_json::from_str::<ChatResponseDelta>(&line) {
                        Ok(delta) => {
                            let parse_time = parse_start.elapsed();
                            let json_time = json_start.elapsed();
                            if current_count <= 5 || parse_time.as_millis() > 10 || json_time.as_millis() > 10 {
                                log::info!(
                                    "[OLLAMA DIRECT] Chunk #{}: parse={}ms json={}ms total={}ms (since_start={}ms)",
                                    current_count,
                                    parse_time.as_millis(),
                                    json_time.as_millis(),
                                    parse_time.as_millis(),
                                    start_time.elapsed().as_millis()
                                );
                            }
                            return Some((Ok(delta), (rx, buffer, start_time, chunk_count)));
                        }
                        Err(e) => {
                            log::debug!("[OLLAMA DIRECT] Failed to parse line: {} (line: {}...)", e, line.chars().take(100).collect::<String>());
                            continue;
                        }
                    }
                }
                
                // Читаем новые данные из канала (приходят из отдельного потока)
                let read_start = std::time::Instant::now();
                match rx.recv().await {
                    Ok(Ok(data)) => {
                        if data.is_empty() {
                            log::info!("[OLLAMA DIRECT] EOF reached after {}ms", start_time.elapsed().as_millis());
                            return None; // EOF
                        }
                        let read_time = read_start.elapsed();
                        if read_time.as_millis() > 5 {
                            log::info!(
                                "[OLLAMA DIRECT] Read {} bytes in {}ms (since_start={}ms)",
                                data.len(),
                                read_time.as_millis(),
                                start_time.elapsed().as_millis()
                            );
                        }
                        buffer.push_str(&String::from_utf8_lossy(&data));
                    }
                    Ok(Err(e)) => {
                        log::error!("[OLLAMA DIRECT] Read error: {} (since_start={}ms)", e, start_time.elapsed().as_millis());
                        return Some((Err(anyhow::anyhow!(e).into()), (rx, buffer, start_time, chunk_count)));
                    }
                    Err(_) => {
                        log::info!("[OLLAMA DIRECT] Channel closed after {}ms", start_time.elapsed().as_millis());
                        return None; // Канал закрыт
                    }
                }
            }
        },
    )
    .boxed())
}
