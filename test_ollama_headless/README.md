# test_ollama_headless

⚠️ **ПРОБЛЕМА С КОМПИЛЯЦИЕЙ**: Из-за проблем с зависимостями `ashpd` и `zbus`, бинарник не компилируется.

## Решение: Использовать встроенный headless режим Zed

### Способ 1: Ручной запуск

```bash
# Запустить Zed в headless режиме
ZED_HEADLESS=1 zed

# Затем в Zed запустить Ollama запрос через UI (Cmd+K или Ctrl+K)
# И смотреть логи [OLLAMA CONSOLE] Read ... bytes: total=Xms
```

### Способ 2: Автоматический скрипт

```bash
# Запустить скрипт для автоматического теста
./run_headless_test.sh gpt-oss:20b 100 "Tell me about wolf"

# Смотреть логи в реальном времени
tail -f /tmp/zed_headless.log | grep "OLLAMA CONSOLE"
```

## Или используйте test_ollama.rs

Для сравнения производительности используйте `test_ollama.rs`, который работает без GPUI:

```bash
cd test_ollama
cargo run --release -- gpt-oss:20b 100 "Tell me about wolf"
```

Это покажет базовую производительность без GPUI.

## Сравнение

1. **test_ollama.rs** (без GPUI): `read_time=5-7ms` - базовая производительность
2. **Zed headless** (`ZED_HEADLESS=1`): `total=5-7ms` или `total=40-45ms`?
3. **Zed GUI**: `total=40-45ms` - текущая проблема

Если headless быстрый, а GUI медленный → проблема в GUI компонентах GPUI.
