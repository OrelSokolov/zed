use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use reqwest_client::ReqwestClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use http_client::{AsyncBody, HttpClient, HttpRequestExt, Method, Request as HttpRequest};
use futures::{AsyncReadExt, stream::BoxStream};

// Локальная копия KeepAlive чтобы не зависеть от settings
#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(untagged)]
enum KeepAlive {
    Seconds(isize),
    Duration(String),
}

impl KeepAlive {
    fn indefinite() -> Self {
        Self::Seconds(-1)
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::indefinite()
    }
}

// Локальные копии типов из ollama
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "role", rename_all = "lowercase")]
enum ChatMessage {
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
struct OllamaToolCall {
    pub id: Option<String>,
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize, Debug)]
struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub keep_alive: KeepAlive,
    pub options: Option<ChatOptions>,
    pub tools: Vec<OllamaTool>,
    pub think: Option<bool>,
}

#[derive(Serialize, Default, Debug)]
struct ChatOptions {
    pub num_ctx: Option<u64>,
    pub num_predict: Option<isize>,
    pub stop: Option<Vec<String>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum OllamaTool {
    Function { function: OllamaFunctionTool },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct OllamaFunctionTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
}

#[derive(Deserialize, Debug)]
struct ChatResponseDelta {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done_reason: Option<String>,
    pub done: bool,
    pub prompt_eval_count: Option<u64>,
    pub eval_count: Option<u64>,
}

// Копия функции stream_chat_completion из ollama.rs
async fn stream_chat_completion(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    request: ChatRequest,
) -> anyhow::Result<BoxStream<'static, anyhow::Result<ChatResponseDelta>>> {
    let uri = format!("{api_url}/api/chat");
    let request = HttpRequest::builder()
        .method(Method::POST)
        .uri(uri)
        .header("Content-Type", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::from(serde_json::to_string(&request)?))?;

    let mut response = client.send(request).await?;
    if response.status().is_success() {
        let body = response.into_body();
        
        // Оптимизированное чтение стрима: читаем чанками и обрабатываем построчно
        // Используем тот же подход, что и LMStudio - читаем body напрямую без Pin<Box<>>
        let start_time = std::time::Instant::now();
        let chunk_count = std::sync::atomic::AtomicU64::new(0);
        let chunk_count = std::sync::Arc::new(chunk_count);
        let chunk_count_clone = chunk_count.clone();
        Ok(futures::stream::unfold(
            (body, String::new(), start_time, chunk_count_clone),
            |(mut body, mut buffer, start_time, chunk_count)| async move {
                use futures::AsyncReadExt;
                
                loop {
                    // Обрабатываем все полные строки в буфере
                    if let Some(newline_pos) = buffer.find('\n') {
                        let parse_start = std::time::Instant::now();
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
                        
                        let current_count = chunk_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        let parse_time = parse_start.elapsed();
                        
                        // Парсим JSON
                        let json_start = std::time::Instant::now();
                        match serde_json::from_str::<ChatResponseDelta>(&line) {
                            Ok(delta) => {
                                let json_time = json_start.elapsed();
                                let total_time = parse_start.elapsed();
                                if current_count <= 5 || total_time.as_millis() > 10 || json_time.as_millis() > 10 {
                                    log::info!(
                                        "[OLLAMA STREAM] Chunk #{}: parse={}ms json={}ms total={}ms since_start={}ms",
                                        current_count,
                                        parse_time.as_millis(),
                                        json_time.as_millis(),
                                        total_time.as_millis(),
                                        start_time.elapsed().as_millis()
                                    );
                                }
                                return Some((Ok(delta), (body, buffer, start_time, chunk_count)));
                            }
                            Err(e) => {
                                // Пропускаем некорректные строки вместо возврата ошибки
                                // чтобы не прерывать стрим из-за одного плохого чанка
                                log::debug!("Failed to parse Ollama response line: {} (line: {}...)", e, line.chars().take(100).collect::<String>());
                                continue;
                            }
                        }
                    }
                    
                    // Читаем новые данные в буфер - используем тот же размер буфера, что и LMStudio
                    let read_start = std::time::Instant::now();
                    let mut chunk = [0u8; 256];
                    match body.read(&mut chunk).await {
                        Ok(0) => return None, // EOF
                        Ok(n) => {
                            let read_time = read_start.elapsed();
                            if read_time.as_millis() > 10 {
                                log::info!(
                                    "[OLLAMA STREAM] Read {} bytes in {}ms (since_start={}ms)",
                                    n,
                                    read_time.as_millis(),
                                    start_time.elapsed().as_millis()
                                );
                            }
                            buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                        }
                        Err(e) => return Some((Err(anyhow::anyhow!(e).into()), (body, buffer, start_time, chunk_count))),
                    }
                }
            },
        )
        .boxed())
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let model = std::env::args().nth(1).unwrap_or_else(|| "gpt-oss:20b".to_string());
    let prompt = "Count from 1 to 200";

    println!("Запрос к модели: {}", model);
    println!("Промпт: {}", prompt);
    println!("{}", "-".repeat(60));

    let start_time = Instant::now();
    let mut first_token_time = None;
    let mut tokens_received = 0;
    let mut response_text = String::new();
    let mut previous_content = String::new();
    let mut chunk_count = 0u64;

    let client: Arc<dyn HttpClient> = Arc::new(ReqwestClient::new());
    let api_url = "http://localhost:11434";

    let request = ChatRequest {
        model: model.clone(),
        messages: vec![ChatMessage::User {
            content: prompt.to_string(),
            images: None,
        }],
        stream: true,
        keep_alive: KeepAlive::indefinite(),
        options: None,
        tools: vec![],
        think: None,
    };

    println!("Отправка запроса...");
    let request_start = Instant::now();
    let mut stream = stream_chat_completion(client.as_ref(), api_url, None, request).await?;
    println!("Запрос отправлен за {}ms", request_start.elapsed().as_millis());

    println!("Начало чтения стрима...");
    while let Some(response) = stream.next().await {
        chunk_count += 1;
        let delta = match response {
            Ok(delta) => delta,
            Err(e) => {
                eprintln!("Ошибка в стриме: {}", e);
                continue;
            }
        };

        match delta.message {
            ChatMessage::Assistant { content, .. } => {
                if !content.is_empty() {
                    if first_token_time.is_none() {
                        first_token_time = Some(Instant::now());
                        let ttft = first_token_time.unwrap().duration_since(start_time);
                        println!("Время до первого токена (TTFT): {:.3} сек", ttft.as_secs_f64());
                    }

                    if content != previous_content {
                        if content.starts_with(&previous_content) {
                            let delta_text = &content[previous_content.len()..];
                            if !delta_text.is_empty() {
                                response_text.push_str(delta_text);
                                print!("{}", delta_text);
                                std::io::stdout().flush()?;
                            }
                        } else {
                            response_text = content.clone();
                            print!("{}", content);
                            std::io::stdout().flush()?;
                        }
                        previous_content = content.clone();
                        tokens_received += 1;
                    }
                }
            }
            _ => {}
        }

        if delta.done {
            println!();
            break;
        }
    }

    let end_time = Instant::now();
    let total_time = end_time.duration_since(start_time);
    let generation_time = first_token_time
        .map(|ftt| end_time.duration_since(ftt))
        .unwrap_or(total_time);

    println!("\n{}", "=".repeat(60));
    println!("РЕЗУЛЬТАТЫ БЕНЧМАРКА:");
    println!("{}", "=".repeat(60));
    if let Some(ttft) = first_token_time {
        let ttft_duration = ttft.duration_since(start_time);
        println!("  Время до первого токена (TTFT): {:.3} сек", ttft_duration.as_secs_f64());
    }
    println!("  Время генерации: {:.3} сек", generation_time.as_secs_f64());
    println!("  Всего времени: {:.3} сек", total_time.as_secs_f64());
    println!("  Всего чанков получено: {}", chunk_count);
    println!("  Чанков с токенами обработано: {}", tokens_received);
    if generation_time.as_secs_f64() > 0.0 {
        println!(
            "  Токенов в секунду (расчетное): {:.2}",
            tokens_received as f64 / generation_time.as_secs_f64()
        );
    }
    println!("  Символов сгенерировано: {}", response_text.len());

    Ok(())
}
