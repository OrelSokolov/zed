use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = std::env::args().nth(1).unwrap_or_else(|| "gpt-oss:20b".to_string());
    let max_tokens = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let prompt = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "Tell me about wolf".to_string());

    println!("Запуск бенчмарка для модели: {}", model);
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

    // Подключаемся через TCP
    let mut stream = TcpStream::connect("localhost:11434")?;
    stream.set_nodelay(true)?;

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

    stream.write_all(http_request.as_bytes())?;
    stream.flush()?;

    // Читаем ответ
    let mut buffer = [0u8; 8192];
    let mut response_buffer = String::new();

    // Пропускаем HTTP headers
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));
        if response_buffer.contains("\r\n\r\n") {
            let parts: Vec<&str> = response_buffer.splitn(2, "\r\n\r\n").collect();
            response_buffer = parts[1].to_string();
            break;
        }
    }

    println!("HTTP headers получены, начинаем читать body...");

    // Читаем body построчно
    let start_time = Instant::now();
    loop {
        // Читаем данные
        let read_start = Instant::now();
        let n = stream.read(&mut buffer)?;
        let read_time = read_start.elapsed();

        if n == 0 {
            break;
        }

        // Логируем все чтения для сравнения с ollama.rs
        if chunk_count < 5 || read_time.as_millis() > 5 {
            println!(
                "[RAW SOCKET] Read {} bytes in {}ms (since_start={}ms)",
                n,
                read_time.as_millis(),
                start_time.elapsed().as_millis()
            );
        }

        response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));

        // Обрабатываем все полные строки
        while let Some(newline_pos) = response_buffer.find('\n') {
            chunk_count += 1;
            let line = response_buffer[..newline_pos].trim().to_string();
            response_buffer = response_buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            // Ollama может использовать chunked encoding - пропускаем размер чанка
            if line.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }

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
                println!(
                    "[RAW SOCKET] Chunk #{}: parsed in {}ms (since_start={}ms)",
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
    println!("РЕЗУЛЬТАТЫ БЕНЧМАРКА:");
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

    Ok(())
}

