#!/bin/bash
# Скрипт для запуска headless теста Ollama в Zed

MODEL="${1:-gpt-oss:20b}"
MAX_TOKENS="${2:-100}"
PROMPT="${3:-Tell me about wolf}"

echo "========================================"
echo "Запуск headless теста Ollama в Zed"
echo "========================================"
echo "Модель: $MODEL"
echo "Промпт: $PROMPT"
echo "Максимум токенов: $MAX_TOKENS"
echo "========================================"
echo ""

# Запускаем Zed в headless режиме в фоне
echo "Запускаем Zed в headless режиме..."
ZED_HEADLESS=1 zed > /tmp/zed_headless.log 2>&1 &
ZED_PID=$!

# Ждем, пока Zed запустится
sleep 3

echo "Zed запущен (PID: $ZED_PID)"
echo "Смотрите логи: tail -f /tmp/zed_headless.log | grep 'OLLAMA CONSOLE'"
echo ""
echo "Теперь запустите Ollama запрос в Zed через UI (Cmd+K или Ctrl+K)"
echo "Или используйте Zed CLI для автоматического запроса"
echo ""
echo "Для остановки: kill $ZED_PID"

# Ждем завершения
wait $ZED_PID
