# Простой способ доказать влияние GPUI

## Самый простой тест

### Шаг 1: Запустить Zed в headless режиме

```bash
# Запустить Zed без GUI
ZED_HEADLESS=1 zed
```

### Шаг 2: Запустить Ollama запрос

В Zed:
1. Нажать `Cmd+K` (или `Ctrl+K` на Linux)
2. Ввести запрос, например: "Tell me about wolf"
3. Нажать Enter

### Шаг 3: Посмотреть логи

Логи `[OLLAMA CONSOLE]` будут выводиться в stderr или в файл логов:
```bash
# Смотреть логи в реальном времени
tail -f ~/.local/share/zed/logs/*.log | grep "OLLAMA CONSOLE"
```

Ищите строки вида:
```
[OLLAMA CONSOLE] Read 148 bytes: total=5ms, syscall=5ms, overhead=0µs
```

### Шаг 4: Сравнить с GUI режимом

```bash
# Запустить Zed в обычном режиме
zed

# Повторить запрос
# Снова посмотреть логи
```

## Ожидаемый результат

- **Headless режим**: `total=5-7ms` (быстро, как test_ollama.rs)
- **GUI режим**: `total=40-45ms` (медленно)

## Вывод

Если headless быстрый, а GUI медленный → **проблема точно в GUI компонентах GPUI** (Wayland/X11 event loop, рендеринг, обработка событий).

Если оба медленные → проблема в базовом GPUI или планировщике ОС.
