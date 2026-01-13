# Анализ perf trace для Ollama stream

## Проблема

В выводе `perf trace` **НЕТ вызовов `read()` на TCP сокете** от Ollama. Все вызовы `read()` - это `eventfd` для синхронизации async runtime.

## Что видно в perf trace

1. **Все `read()` - это eventfd:**
   - `fd: 5<anon_inode:[eventfd]>` - главный поток
   - `fd: 8<anon_inode:[eventfd]>` - другой поток
   - `fd: 11<anon_inode:[eventfd]>` - Timer поток
   - `fd: 32<anon_inode:[eventfd]>` - async-io поток

2. **`ret: -11` = EAGAIN** - неблокирующий read на eventfd

3. **Нет TCP read!** - поток `ollama-stream-reader` либо:
   - Не запустился
   - Его `read()` не видны в perf trace
   - Данные уже в буфере и `read()` не вызывается

## Улучшенные команды perf

### 1. Фильтр по потоку `ollama-stream-reader`
```bash
# Найти PID потока
ps aux | grep ollama-stream-reader

# Трассировать только этот поток
sudo perf trace -e 'syscalls:*read*' -t <TID> --duration 1000
```

### 2. Фильтр по TCP сокету
```bash
# Трассировать все read с фильтром по TCP (fd > 10 обычно)
sudo perf trace -e 'syscalls:sys_enter_read' --filter 'fd > 10' -p $(pgrep zed) --duration 1000
```

### 3. Трассировать все системные вызовы потока
```bash
# Найти TID потока ollama-stream-reader
ps -eLf | grep ollama-stream-reader

# Трассировать все syscalls этого потока
sudo perf trace -t <TID> --duration 1000
```

### 4. Трассировать connect и read вместе
```bash
sudo perf trace -e 'syscalls:*connect*,syscalls:*read*' -p $(pgrep zed) --duration 1000
```

## Что проверить

1. **Запустился ли поток?**
   ```bash
   ps -eLf | grep ollama-stream-reader
   ```

2. **Какой файловый дескриптор у TCP сокета?**
   - В логах должно быть: `[OLLAMA CONSOLE] Connected successfully, TCP socket fd=X`

3. **Есть ли вызовы `read()` на этом fd?**
   - После получения fd, фильтровать perf trace по этому fd

## Возможные причины

1. **Поток не запустился** - проверить логи `[OLLAMA CONSOLE] Thread started`
2. **read() не вызывается** - данные уже в буфере, или поток заблокирован
3. **perf не видит поток** - поток создан в другом namespace или perf не может его отследить

## Следующие шаги

1. Добавить логирование fd TCP сокета (уже добавлено)
2. Запустить perf trace с фильтром по конкретному fd
3. Проверить, запустился ли поток `ollama-stream-reader`
