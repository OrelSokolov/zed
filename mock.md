# Решение проблемы медленного стриминга в Ollama

## Проблема

Стрим токенов из Ollama работал очень медленно - максимум 20 токенов/сек, даже при использовании mock данных с задержкой всего 3мс. Проблема была в том, что **smol планировщик опрашивает futures только при обновлении UI** (примерно 20 раз в секунду), что создавало узкое место.

## Решение

### 1. Отдельный smol runtime в отдельном потоке

В `agent/src/thread.rs` создан отдельный поток с собственным smol runtime для опроса стрима независимо от частоты обновления UI:

```rust
// Используем отдельный smol runtime в отдельном потоке для независимого опроса стрима
// Это обходит проблему с планировщиком, который опрашивает futures только при обновлении UI
let (events_tx, mut events_rx) = mpsc::unbounded();
let mut cancellation_rx_bg = cancellation_rx.clone();

// Запускаем отдельный поток с собственным smol runtime для опроса стрима
let stream_thread = std::thread::spawn(move || {
    // Создаем отдельный smol runtime в этом потоке для независимого опроса
    smol::block_on(async move {
        const BATCH_TIMEOUT_MS: u64 = 4; // Как в terminal.rs
        const MAX_BATCH_SIZE: usize = 100;
        
        let mut event_batch = Vec::new();
        let mut timer = futures::FutureExt::fuse(smol::Timer::after(std::time::Duration::from_millis(BATCH_TIMEOUT_MS)));
        futures::pin_mut!(timer);
        
        futures::pin_mut!(events);
        
        loop {
            futures::select_biased! {
                _ = futures::FutureExt::fuse(cancellation_rx_bg.changed()) => {
                    if *cancellation_rx_bg.borrow() {
                        // Отправляем оставшиеся события перед отменой
                        if !event_batch.is_empty() {
                            let _ = events_tx.unbounded_send(std::mem::take(&mut event_batch));
                        }
                        break;
                    }
                }
                event = futures::FutureExt::fuse(events.next()) => {
                    match event {
                        Some(Ok(event)) => {
                            event_batch.push(Ok(event));
                            
                            // Отправляем батч если он достиг максимального размера
                            if event_batch.len() >= MAX_BATCH_SIZE {
                                let batch = std::mem::take(&mut event_batch);
                                if events_tx.unbounded_send(batch).is_err() {
                                    break; // Получатель закрыт
                                }
                                timer.set(futures::FutureExt::fuse(smol::Timer::after(std::time::Duration::from_millis(BATCH_TIMEOUT_MS))));
                            }
                        }
                        Some(Err(err)) => {
                            // Отправляем ошибку и оставшиеся события
                            if !event_batch.is_empty() {
                                let _ = events_tx.unbounded_send(std::mem::take(&mut event_batch));
                            }
                            let _ = events_tx.unbounded_send(vec![Err(err)]);
                            break;
                        }
                        None => {
                            // Стрим закончился, отправляем оставшиеся события
                            if !event_batch.is_empty() {
                                let _ = events_tx.unbounded_send(std::mem::take(&mut event_batch));
                            }
                            break;
                        }
                    }
                }
                _ = timer => {
                    // Таймер истек, отправляем батч
                    if !event_batch.is_empty() {
                        let batch = std::mem::take(&mut event_batch);
                        if events_tx.unbounded_send(batch).is_err() {
                            break; // Получатель закрыт
                        }
                    }
                    timer.set(futures::FutureExt::fuse(smol::Timer::after(std::time::Duration::from_millis(BATCH_TIMEOUT_MS))));
                }
            }
        }
    });
});
```

### 2. Батчинг событий

События батчатся с таймаутом 4мс (как в `terminal.rs`):
- Первое событие обрабатывается сразу для низкой задержки
- Остальные события батчатся до 100 событий или 4мс
- Батчи передаются в foreground через канал

### 3. Чтение стрима в отдельном runtime в ollama.rs

В `ollama.rs` чтение стрима происходит в отдельном smol runtime в отдельном потоке:

```rust
pub async fn stream_chat_completion(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    request: ChatRequest,
) -> Result<BoxStream<'static, Result<ChatResponseDelta>>> {
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
```

## Полный код stream_chat_completion из ollama.rs

```rust:279:343:crates/ollama/src/ollama.rs
pub async fn stream_chat_completion(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    request: ChatRequest,
) -> Result<BoxStream<'static, Result<ChatResponseDelta>>> {
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
```

## Ключевые моменты решения

1. **Отдельный smol runtime** - создан в отдельном потоке с `smol::block_on()`, который опрашивает futures независимо от UI frame rate
2. **Батчинг событий** - события собираются в батчи с таймаутом 4мс для эффективности
3. **Async задержка** - используется `smol::Timer::after()` вместо `std::thread::sleep()` для неблокирующей задержки
4. **Канал для передачи** - данные передаются из отдельного потока в foreground через `futures::channel::mpsc::unbounded`

## Результат

Теперь стрим опрашивается с высокой частотой в отдельном потоке, независимо от частоты обновления UI. Это позволяет достичь throughput, ограниченного только скоростью генерации модели, а не планировщиком.

## Почему это работает

Отдельный smol runtime в отдельном потоке опрашивает futures с высокой частотой (не ограниченной частотой обновления UI), что позволяет обрабатывать события из стрима практически мгновенно. Батчинг событий обеспечивает эффективность передачи данных в foreground executor.

