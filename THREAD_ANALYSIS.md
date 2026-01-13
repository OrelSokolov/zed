# Анализ потоков в Zed

## Основные пулы потоков

### 1. Rayon Thread Pool
**Место**: `crates/zed/src/main.rs:280-285`
```rust
rayon::ThreadPoolBuilder::new()
    .num_threads(std::thread::available_parallelism().map_or(1, |n| n.get().div_ceil(2)))
```
- **Количество**: `available_parallelism() / 2` (округление вверх)
- **Назначение**: Параллельная обработка CPU-интенсивных задач
- **Пример**: На 8-ядерной системе = 4 потока

### 2. GPUI Background Executor Threads (Linux)
**Место**: `crates/gpui/src/platform/linux/dispatcher.rs:38-92`
```rust
let thread_count = std::thread::available_parallelism()
    .map_or(MIN_THREADS, |i| i.get().max(MIN_THREADS));
```
- **Количество**: `available_parallelism().max(2)` (минимум 2)
- **Назначение**: Выполнение background задач (async tasks)
- **Пример**: На 8-ядерной системе = 8 потоков

### 3. Timer Thread
**Место**: `crates/gpui/src/platform/linux/dispatcher.rs:95-97`
```rust
let timer_thread = std::thread::Builder::new()
    .name("Timer".to_owned())
    .spawn(|| { ... });
```
- **Количество**: 1 поток
- **Назначение**: Обработка таймеров через calloop event loop

## Дополнительные потоки (создаются по требованию)

### 4. Open Listener Thread
**Место**: `crates/zed/src/zed/open_listener.rs`
- **Количество**: 1 поток (если используется)
- **Назначение**: Обработка CLI запросов через IPC

### 5. Clipboard Thread (Linux X11)
**Место**: `crates/gpui/src/platform/linux/x11/clipboard.rs:961`
- **Количество**: 1 поток (только на X11)
- **Назначение**: Обработка clipboard операций

### 6. VSync Thread (Windows)
**Место**: `crates/gpui/src/platform/windows/platform.rs:257`
- **Количество**: 1 поток (только на Windows)
- **Назначение**: Ожидание VSync событий

### 7. Screen Capture Threads (macOS)
**Место**: `crates/gpui/src/platform/scap_screen_capture.rs`
- **Количество**: 1-3 потока (по требованию)
- **Назначение**: Захват экрана

### 8. Ollama Reader Thread
**Место**: `crates/ollama/src/ollama.rs:280`
- **Количество**: 1 поток (при активном запросе к Ollama)
- **Назначение**: Чтение стрима от Ollama

### 9. Agent Stream Thread
**Место**: `crates/agent/src/thread.rs:1383`
- **Количество**: 1 поток (при активном agent запросе)
- **Назначение**: Обработка стрима от language model

### 10. Realtime Threads
**Место**: `crates/gpui/src/platform/linux/dispatcher.rs:220`
- **Количество**: По требованию (для audio и других realtime задач)
- **Назначение**: Задачи с высоким приоритетом (SCHED_FIFO)

## Итого на типичной системе

**На 8-ядерной системе (Linux):**
- Rayon: 4 потока
- GPUI Background: 8 потоков
- Timer: 1 поток
- Clipboard (X11): 1 поток
- **Базовое количество: ~14 потоков**

**Дополнительно при активной работе:**
- Ollama reader: +1 поток
- Agent stream: +1 поток
- Open listener: +1 поток (если используется)
- **Максимум: ~17 потоков**

## Проблема с планировщиком

При большом количестве потоков планировщик CFS (Completely Fair Scheduler) в Linux может:
1. **Распределять CPU время** между всеми потоками
2. **Откладывать пробуждение** потоков из-за конкуренции
3. **Создавать задержки** в 40-50мс для потоков с низким приоритетом

Это объясняет, почему `read()` в `ollama-stream-reader` блокируется на 40-45мс, в то время как в standalone `test_ollama.rs` (1 поток) время чтения составляет 5-7мс.

## Рекомендации

1. **Установить более высокий приоритет** для `ollama-stream-reader` потока
2. **Использовать SCHED_FIFO** для realtime планирования (требует root)
3. **Уменьшить количество background потоков** GPUI (если возможно)
4. **Использовать CPU affinity** для привязки потока к конкретному ядру
