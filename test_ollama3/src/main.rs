use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
#[cfg(target_os = "linux")]
use libc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let model = std::env::args().nth(1).unwrap_or_else(|| "gpt-oss:20b".to_string());
    let max_tokens = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let prompt = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "Tell me about wolf".to_string());

    println!("Запуск бенчмарка для модели: {} (с smol runtime)", model);
    println!("Промпт: {}...", prompt.chars().take(50).collect::<String>());
    println!("Максимум токенов: {}", max_tokens);
    println!("{}", "-".repeat(60));

    let start_time = Instant::now();
    let mut first_token_time = None;
    let mut tokens_received = 0;
    let mut response_text = String::new();
    let mut previous_content = String::new();
    let mut previous_thinking = String::new();
    let mut token_times = Vec::new();
    let mut eval_count = 0;
    let mut eval_duration = 0.0;
    let mut prompt_eval_count = 0;
    let mut prompt_eval_duration = 0.0;

    let mut chunk_count = 0;
    let mut message_chunks = 0;
    let mut assistant_chunks = 0;
    let mut thinking_chunks = 0;
    let mut content_chunks = 0;

    // Используем smol::block_on для создания отдельного рантайма, как в Zed
    println!("[DEBUG] Starting smol::block_on...");
    smol::block_on(async {
        println!("[DEBUG] Inside async block, connecting...");
        // Подключаемся через TCP синхронно, но в async контексте
        let stream = smol::unblock(move || {
            println!("[DEBUG] Inside unblock, connecting to localhost:11434...");
            let stream = TcpStream::connect("localhost:11434")?;
            stream.set_nodelay(true)?;
            
            // Устанавливаем неблокирующий режим (как в ollama.rs)
            #[cfg(target_os = "linux")]
            {
                unsafe {
                    let flags = libc::fcntl(stream.as_raw_fd(), libc::F_GETFL);
                    if flags >= 0 {
                        libc::fcntl(stream.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
                        println!("[DEBUG] Set non-blocking mode");
                    }
                }
            }
            
            println!("[DEBUG] Connected successfully");
            Ok::<TcpStream, Box<dyn std::error::Error + Send + Sync>>(stream)
        })
        .await?;
        
        println!("[DEBUG] Wrapping stream in Arc<Mutex<>>...");
        // Обертываем в Arc<Mutex<>> для разделения владения между async блоками
        let stream = Arc::new(Mutex::new(stream));

        // Формируем HTTP запрос
        let request_body = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "stream": true,
            "options": {
                "num_predict": max_tokens,
                "temperature": 0.7
            }
        });

        let body_str = serde_json::to_string(&request_body)?;
        let http_request = format!(
            "POST /api/chat HTTP/1.1\r\n\
             Host: localhost:11434\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body_str.len(),
            body_str
        );

        // Отправляем запрос синхронно через unblock
        println!("[DEBUG] Sending HTTP request...");
        {
            let stream = stream.clone();
            smol::unblock(move || {
                let mut stream = stream.lock().unwrap();
                stream.write_all(http_request.as_bytes())?;
                stream.flush()?;
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })
            .await?;
        }
        println!("[DEBUG] Request sent, reading headers...");

        // Читаем ответ
        let mut response_buffer = String::new();

        // Пропускаем HTTP headers
        println!("[DEBUG] Reading headers...");
        let mut header_read_count = 0;
        loop {
            header_read_count += 1;
            println!("[DEBUG] Reading header chunk #{}...", header_read_count);
            // Читаем данные через unblock с обработкой неблокирующего режима
            let data = {
                let stream = stream.clone();
                smol::unblock(move || {
                    let mut stream = stream.lock().unwrap();
                    let mut buffer = vec![0u8; 8192];
                    let read_result = stream.read(&mut buffer);
                    
                    // Обрабатываем WouldBlock для неблокирующего режима
                    #[cfg(target_os = "linux")]
                    let read_result = match read_result {
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Данных нет, ждем через poll
                            let fd = stream.as_raw_fd();
                            let mut pollfd = libc::pollfd {
                                fd,
                                events: libc::POLLIN,
                                revents: 0,
                            };
                            let poll_result = unsafe { libc::poll(&mut pollfd, 1, -1) }; // -1 = ждать бесконечно
                            if poll_result > 0 && (pollfd.revents & libc::POLLIN) != 0 {
                                stream.read(&mut buffer)
                            } else {
                                read_result
                            }
                        }
                        _ => read_result,
                    };
                    
                    match read_result {
                        Ok(n) => {
                            buffer.truncate(n);
                            Ok(buffer)
                        }
                        Err(e) => Err(e),
                    }
                })
                .await?
            };
            
            let n = data.len();
            println!("[DEBUG] Read {} bytes for headers (total buffer size: {})", n, response_buffer.len());
            if n == 0 {
                println!("[DEBUG] EOF while reading headers");
                break;
            }
            
            // Проверяем, что мы действительно прочитали данные (не нули)
            let non_zero_count = data.iter().filter(|&&b| b != 0).count();
            println!("[DEBUG] Non-zero bytes in chunk: {}/{}", non_zero_count, n);
            if non_zero_count == 0 {
                println!("[DEBUG] WARNING: All bytes are zeros! Something is wrong.");
                println!("[DEBUG] First 50 bytes as hex: {:?}", &data[..n.min(50)]);
                // Пропускаем этот чанк и продолжаем
                continue;
            }
            
            // Показываем первые байты для диагностики
            if header_read_count <= 3 {
                println!("[DEBUG] First 100 bytes as hex: {:?}", &data[..n.min(100)]);
                println!("[DEBUG] First 100 bytes as string: {}", String::from_utf8_lossy(&data[..n.min(100)]));
            }
            
            // Добавляем сырые байты в буфер
            response_buffer.push_str(&String::from_utf8_lossy(&data));
            
            // Проверяем наличие конца заголовков в сырых байтах
            if let Some(pos) = response_buffer.as_bytes().windows(4).position(|w| w == b"\r\n\r\n") {
                println!("[DEBUG] Found end of headers at position {}!", pos);
                let headers = &response_buffer[..pos];
                let body_start = pos + 4;
                println!("[DEBUG] Headers (first 500 chars): {}", headers.chars().take(500).collect::<String>());
                response_buffer = response_buffer[body_start..].to_string();
                println!("[DEBUG] Headers complete, body starts with {} bytes", response_buffer.len());
                if !response_buffer.is_empty() {
                    println!("[DEBUG] Body start (first 200 chars): {}", response_buffer.chars().take(200).collect::<String>());
                }
                break;
            } else {
                // Если прочитали слишком много и не нашли конец заголовков - возможно, заголовки уже прочитаны
                if response_buffer.len() > 10000 {
                    println!("[DEBUG] WARNING: Read more than 10KB and still no end of headers!");
                    println!("[DEBUG] First 500 bytes as hex: {:?}", &response_buffer.as_bytes()[..500.min(response_buffer.len())]);
                    println!("[DEBUG] First 500 chars as string: {}", response_buffer.chars().take(500).collect::<String>());
                    // Попробуем найти начало JSON (обычно это {)
                    if let Some(json_start) = response_buffer.find('{') {
                        println!("[DEBUG] Found JSON start at position {}, assuming headers already read", json_start);
                        response_buffer = response_buffer[json_start..].to_string();
                        break;
                    } else {
                        // Просто продолжаем - возможно, заголовки уже были прочитаны
                        println!("[DEBUG] No JSON start found, continuing anyway...");
                        break;
                    }
                }
            }
        }

        println!("\nHTTP headers получены, начинаем читать body...");

        // Читаем body построчно с логированием, как в test_ollama.rs
        let start_time = Instant::now();
        let mut last_read_time = Instant::now();
        let mut read_count = 0u64;
        let mut line_count = 0u64;
        let mut last_line_time = Instant::now();

        loop {
            // Измеряем время с последнего read()
            let waited_since_last_read = last_read_time.elapsed();

            // Читаем данные через smol::unblock с поддержкой неблокирующего режима
            if read_count < 3 {
                println!("[DEBUG] Starting read #{}...", read_count + 1);
            }
            let read_start = Instant::now();
            
            // Проверяем наличие данных через poll (для неблокирующего режима)
            #[cfg(target_os = "linux")]
            let poll_start = Instant::now();
            #[cfg(target_os = "linux")]
            let has_data = {
                let stream = stream.clone();
                smol::unblock(move || {
                    let stream = stream.lock().unwrap();
                    let fd = stream.as_raw_fd();
                    let mut pollfd = libc::pollfd {
                        fd,
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
                    result > 0 && (pollfd.revents & libc::POLLIN) != 0
                })
                .await
            };
            #[cfg(target_os = "linux")]
            let poll_time = poll_start.elapsed();
            #[cfg(not(target_os = "linux"))]
            let has_data = true;
            #[cfg(not(target_os = "linux"))]
            let poll_time = std::time::Duration::ZERO;
            
            let data = {
                let stream = stream.clone();
                smol::unblock(move || {
                    let mut stream = stream.lock().unwrap();
                    let mut local_buffer = vec![0u8; 8192];
                    let read_result = stream.read(&mut local_buffer);
                    
                    // Обрабатываем WouldBlock для неблокирующего режима
                    #[cfg(target_os = "linux")]
                    let read_result = match read_result {
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Данных нет, ждем через poll
                            let fd = stream.as_raw_fd();
                            let mut pollfd = libc::pollfd {
                                fd,
                                events: libc::POLLIN,
                                revents: 0,
                            };
                            let poll_result = unsafe { libc::poll(&mut pollfd, 1, 100) }; // 100ms timeout
                            if poll_result > 0 && (pollfd.revents & libc::POLLIN) != 0 {
                                stream.read(&mut local_buffer)
                            } else {
                                read_result
                            }
                        }
                        _ => read_result,
                    };
                    
                    match read_result {
                        Ok(n) => {
                            local_buffer.truncate(n);
                            Ok(local_buffer)
                        }
                        Err(e) => Err(e),
                    }
                })
                .await?
            };
            let read_time = read_start.elapsed();
            last_read_time = Instant::now();
            read_count += 1;
            
            let n = data.len();

            if read_count < 3 {
                println!("[DEBUG] Read #{} completed: {} bytes in {:?}", read_count, n, read_time);
            }

            if n == 0 {
                println!("[DEBUG] EOF reached, breaking loop");
                break;
            }

            // Логируем все чтения для сравнения с ollama.rs (без пропусков для первых 100)
            if read_count <= 100 || read_count % 50 == 0 || read_time.as_millis() > 10 {
                eprintln!(
                    "\n[RAW SOCKET] #{} {} bytes: waited_since_last={:?}, read_time={:?}, poll={:?}, has_data={}",
                    read_count, n, waited_since_last_read, read_time, poll_time, has_data
                );
            }

            response_buffer.push_str(&String::from_utf8_lossy(&data));

            // Обрабатываем все полные строки
            while let Some(newline_pos) = response_buffer.find('\n') {
                line_count += 1;
                let waited_since_last_line = last_line_time.elapsed();
                last_line_time = Instant::now();

                let line = response_buffer[..newline_pos].trim().to_string();
                response_buffer = response_buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // Ollama может использовать chunked encoding - пропускаем размер чанка
                if line.chars().all(|c| c.is_ascii_hexdigit()) {
                    continue;
                }

                if line_count <= 5 || line_count % 50 == 0 {
                    eprintln!(
                        "\n[TEST LINE] #{} waited_since_last_line={:?}, len={}",
                        line_count, waited_since_last_line, line.len()
                    );
                }

                chunk_count += 1;
                if chunk_count <= 3 {
                    println!("DEBUG: Чанк {}: {}...", chunk_count, &line.chars().take(200).collect::<String>());
                }

                // Парсим JSON
                let parse_start = Instant::now();
                let chunk: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Ошибка парсинга JSON: {} (строка: {})", e, &line.chars().take(100).collect::<String>());
                        continue;
                    }
                };
                let parse_time = parse_start.elapsed();
                
                // Логируем парсинг для сравнения с ollama.rs
                if chunk_count <= 20 || chunk_count % 10 == 0 {
                    eprintln!(
                        "\n[RAW SOCKET] Chunk #{}: parsed in {}ms (since_start={}ms)",
                        chunk_count,
                        parse_time.as_millis(),
                        start_time.elapsed().as_millis()
                    );
                }

                // Обработка сообщения от assistant
                if let Some(message) = chunk.get("message") {
                    message_chunks += 1;
                    let role = message.get("role").and_then(|r| r.as_str()).unwrap_or("");

                    if role == "assistant" {
                        assistant_chunks += 1;
                        let current_content = message
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        let current_thinking = message
                            .get("thinking")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");

                        // Обрабатываем content
                        if current_content != previous_content {
                            content_chunks += 1;
                            if first_token_time.is_none() && !current_content.is_empty() {
                                first_token_time = Some(Instant::now());
                                let ttft = first_token_time.unwrap().duration_since(start_time);
                                println!("Время до первого токена content (TTFT): {:.3} сек", ttft.as_secs_f64());
                            }

                            if current_content.starts_with(&previous_content) {
                                let delta = &current_content[previous_content.len()..];
                                if !delta.is_empty() {
                                    response_text.push_str(delta);
                                }
                            } else {
                                response_text = current_content.to_string();
                            }

                            previous_content = current_content.to_string();
                            tokens_received += 1;
                            token_times.push(Instant::now());
                        }

                        // Обрабатываем thinking
                        if current_thinking != previous_thinking {
                            thinking_chunks += 1;
                            if current_content.is_empty() {
                                if first_token_time.is_none() && !current_thinking.is_empty() {
                                    first_token_time = Some(Instant::now());
                                    let ttft = first_token_time.unwrap().duration_since(start_time);
                                    println!("Время до первого токена thinking (TTFT): {:.3} сек", ttft.as_secs_f64());
                                }

                                if current_thinking.starts_with(&previous_thinking) {
                                    let delta = &current_thinking[previous_thinking.len()..];
                                    if !delta.is_empty() {
                                        response_text.push_str(delta);
                                    }
                                } else {
                                    response_text = current_thinking.to_string();
                                }

                                tokens_received += 1;
                                token_times.push(Instant::now());
                            }
                            previous_thinking = current_thinking.to_string();
                        }
                    }
                }

                // Получаем метрики из последнего чанка
                if chunk.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                    eval_count = chunk
                        .get("eval_count")
                        .and_then(|c| c.as_u64())
                        .unwrap_or(0);
                    eval_duration = chunk
                        .get("eval_duration")
                        .and_then(|d| d.as_u64())
                        .map(|d| d as f64 / 1e9)
                        .unwrap_or(0.0);
                    prompt_eval_count = chunk
                        .get("prompt_eval_count")
                        .and_then(|c| c.as_u64())
                        .unwrap_or(0);
                    prompt_eval_duration = chunk
                        .get("prompt_eval_duration")
                        .and_then(|d| d.as_u64())
                        .map(|d| d as f64 / 1e9)
                        .unwrap_or(0.0);
                    break;
                }
            }
        }

        let end_time = Instant::now();
        let total_time = end_time.duration_since(start_time);
        let generation_time = first_token_time
            .map(|ftt| end_time.duration_since(ftt))
            .unwrap_or(total_time);

        // Вычисляем среднюю скорость генерации
        let tokens_per_sec_calculated = if token_times.len() > 1 {
            let intervals: Vec<_> = token_times
                .windows(2)
                .map(|w| w[1].duration_since(w[0]).as_secs_f64())
                .collect();
            let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
            if avg_interval > 0.0 {
                1.0 / avg_interval
            } else {
                0.0
            }
        } else {
            tokens_received as f64 / generation_time.as_secs_f64()
        };

        println!("\n{}", "=".repeat(60));
        println!("РЕЗУЛЬТАТЫ БЕНЧМАРКА (с smol runtime):");
        println!("{}", "=".repeat(60));
        println!(
            "  Время обработки промпта: {:.3} сек ({} токенов)",
            prompt_eval_duration, prompt_eval_count
        );
        if let Some(ttft) = first_token_time {
            let ttft_duration = ttft.duration_since(start_time);
            println!("  Время до первого токена (TTFT): {:.3} сек", ttft_duration.as_secs_f64());
        } else {
            println!("  Время до первого токена (TTFT): не получен");
        }
        println!("  Время генерации: {:.3} сек", generation_time.as_secs_f64());
        println!("  Всего времени: {:.3} сек", total_time.as_secs_f64());
        println!("  Всего чанков от сервера: {}", chunk_count);
        println!("  Чанков с токенами обработано: {}", tokens_received);
        println!("  Токенов сгенерировано (eval_count): {}", eval_count);
        if tokens_received > 0 {
            println!(
                "  Средний размер чанка: {:.2} чанков на токен",
                chunk_count as f64 / tokens_received as f64
            );
        }
        if eval_duration > 0.0 {
            println!(
                "  Токенов в секунду (из eval_duration): {:.2}",
                eval_count as f64 / eval_duration
            );
        }
        println!(
            "  Чанков в секунду (расчетное): {:.2}",
            tokens_per_sec_calculated
        );
        println!("  Символов сгенерировано: {}", response_text.len());
        println!("\nПервые 300 символов ответа:");
        println!(
            "{}",
            if response_text.len() > 300 {
                format!("{}...", &response_text[..300])
            } else {
                response_text.clone()
            }
        );

        println!("\nDEBUG:");
        println!("  Всего чанков: {}", chunk_count);
        println!("  Чанков с message: {}", message_chunks);
        println!("  Чанков с assistant: {}", assistant_chunks);
        println!("  Чанков с thinking: {}", thinking_chunks);
        println!("  Чанков с content: {}", content_chunks);

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;

    Ok(())
}
