# Руководство по интеграции вкладок в Zed-Agent

## Обзор

Это руководство описывает процесс интеграции системы вкладок для Zed-Agent, позволяющей поддерживать несколько одновременных разговоров. Реализация использует второй подход: кастомная система вкладок с минимальными изменениями в существующей архитектуре.

## Созданные файлы

### 1. Модуль tabs (`zed/crates/agent_ui/src/tabs/mod.rs`)

Полностью реализованный модуль управления вкладками:

- **`AgentTab`** - структура для хранения данных одной вкладки
  - `id`: уникальный идентификатор
  - `title`: отображаемое название
  - `active_view`: содержимое вкладки (поток, история, настройки)
  - `created_at`: время создания
  - `is_active`: активна ли вкладка
  - `session_id`: ID сессии для потоков агента
  - `is_modified`: наличие несохраненных изменений

- **`AgentTabs`** - менеджер коллекции вкладок
  - `add_tab()` - добавление новой вкладки
  - `select_tab()` / `select_tab_by_id()` - выбор вкладки
  - `close_tab()` / `close_tab_by_id()` - закрытие вкладки
  - `next_tab()` / `previous_tab()` - навигация
  - `update_tab_title()` - обновление названия
  - `find_tab_by_session()` - поиск по ID сессии

- **`AgentTabsBar`** - компонент UI для отображения вкладок
  - Автоматическая стилизация активных/неактивных вкладок
  - Кнопки закрытия вкладок
  - Обработка кликов, средней кнопки мыши
  - Поддержка обратных вызов для действий

- **Unit тесты** - полная проверка всех функций

### 2. Методы рендеринга (`zed/crates/agent_ui/examples/tab_rendering_methods.rs`)

Примеры методов для интеграции:

- `render_tab_bar()` - рендер панели вкладок
- `render_tab_item()` - рендер отдельной вкладки
- `render_empty_state()` - состояние при отсутствии вкладок
- `render_active_view()` - рендер активного содержимого

## Шаги интеграции

### Шаг 1: Добавить модуль tabs в agent_ui

**Файл:** `zed/crates/agent_ui/src/agent_ui.rs`

Добавьте модуль tabs в начало файла:

```rust
mod tabs;
```

### Шаг 2: Обновить структуру AgentPanel

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Добавьте поля для управления вкладками в структуру:

```rust
pub struct AgentPanel {
    // ... существующие поля ...
    
    // ДОБАВИТЬ:
    tabs: AgentTabs,
    active_tab_index: usize,
}
```

### Шаг 3: Обновить конструктор AgentPanel::new()

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Инициализируйте вкладки при создании панели:

```rust
impl AgentPanel {
    fn new(...) -> Self {
        // ... существующий код ...
        
        // Инициализация вкладок с начальным видом
        let mut tabs = AgentTabs::new();
        let default_tab = AgentTab::new(
            match &active_view {
                ActiveView::ExternalAgentThread { thread_view } => {
                    thread_view.read(cx).title()
                }
                _ => "New Thread".into(),
            },
            active_view.clone(),
        );
        tabs.add_tab(default_tab);
        
        let mut panel = Self {
            // ... существующие поля ...
            
            // ДОБАВИТЬ:
            tabs: AgentTabs::new(),
            active_tab_index: 0,
        };
        
        // Установка вкладок
        panel.tabs = tabs;
        panel.active_tab_index = 0;
        
        // ... остальной код ...
        panel
    }
}
```

### Шаг 4: Добавить методы управления вкладками

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Добавьте следующие методы после `set_active_view()`:

```rust
// ============================================================================
// ТАБЛИЧНЫЕ МЕТОДЫ
// ============================================================================

/// Добавляет новую вкладку
fn add_new_tab(
    &mut self,
    title: impl Into<SharedString>,
    active_view: ActiveView,
    focus: bool,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    let tab = AgentTab::new(title, active_view);
    
    // Деактивировать все существующие вкладки
    for tab in self.tabs.tabs_mut() {
        tab.is_active = false;
    }
    
    // Добавить новую вкладку
    self.tabs.add_tab(tab);
    
    // Обновить активный вид
    if let Some(active_tab) = self.tabs.active_tab() {
        self.active_view = active_tab.active_view.clone();
        self.active_tab_index = self.tabs.active_index();
    }
    
    if focus {
        self.focus_handle(cx).focus(window, cx);
    }
    
    cx.notify();
    self.serialize(cx);
}

/// Выбирает вкладку по индексу
fn select_tab(
    &mut self,
    index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    if index >= self.tabs.tab_count() {
        return;
    }
    
    // Использовать AgentTabs для выбора
    if let Some(tab) = self.tabs.select_tab(index) {
        self.active_view = tab.active_view.clone();
        self.active_tab_index = index;
        
        match &tab.active_view {
            ActiveView::History { .. } | ActiveView::Configuration => {
                if !matches!(
                    &self.active_view,
                    ActiveView::History { .. } | ActiveView::Configuration
                ) {
                    self.previous_view = Some(std::mem::replace(
                        &mut self.active_view,
                        tab.active_view.clone(),
                    ));
                }
            }
            _ => {
                self.previous_view = None;
            }
        }
        
        cx.notify();
    }
}

/// Закрывает вкладку
fn close_tab(
    &mut self,
    index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    if index >= self.tabs.tab_count() {
        return;
    }
    
    // Не закрывать последнюю вкладку
    if self.tabs.tab_count() == 1 {
        self.new_thread(&NewThread, window, cx);
        return;
    }
    
    if let Some(closed_tab) = self.tabs.close_tab(index) {
        // Обновить активный вид
        if let Some(active_tab) = self.tabs.active_tab() {
            self.active_view = active_tab.active_view.clone();
            self.active_tab_index = self.tabs.active_index();
        } else if !self.tabs.is_empty() {
            self.select_tab(0, window, cx);
        }
        
        cx.notify();
        self.serialize(cx);
    }
}

/// Переход к следующей вкладке
fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.tabs.is_empty() {
        return;
    }
    
    let new_index = self.tabs.active_index() + 1;
    if new_index >= self.tabs.tab_count() {
        self.select_tab(0, window, cx);
    } else {
        self.select_tab(new_index, window, cx);
    }
}

/// Переход к предыдущей вкладке
fn previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.tabs.is_empty() {
        return;
    }
    
    let current_index = self.tabs.active_index();
    if current_index == 0 {
        self.select_tab(self.tabs.tab_count() - 1, window, cx);
    } else {
        self.select_tab(current_index - 1, window, cx);
    }
}

// Вспомогательные методы
fn active_tab(&self) -> Option<&AgentTab> {
    self.tabs.active_tab()
}

fn tabs(&self) -> &[AgentTab] {
    self.tabs.tabs()
}

fn tab_count(&self) -> usize {
    self.tabs.tab_count()
}
```

### Шаг 5: Обновить создание потоков для использования вкладок

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Измените методы создания потоков:

```rust
// Обновить new_thread()
fn new_thread(&mut self, _action: &NewThread, window: &mut Window, cx: &mut Context<Self>) {
    self.new_agent_thread(AgentType::NativeAgent, window, cx);
    // Поток будет добавлен как вкладка через обновленный external_thread
}

// Обновить _external_thread()
fn _external_thread(
    &mut self,
    server: Rc<dyn AgentServer>,
    resume_thread: Option<AgentSessionInfo>,
    summarize_thread: Option<AgentSessionInfo>,
    // ... существующие параметры ...
) {
    // ... существующий код создания thread_view ...
    
    // ОПРЕДЕЛИТЬ название вкладки
    let tab_title = if let Some(resume_info) = &resume_thread {
        resume_info.title.as_ref()
            .filter(|title| !title.is_empty())
            .cloned()
    } else if let Some(summary_info) = &summarize_thread {
        summary_info.title.as_ref()
            .filter(|title| !title.is_empty())
            .cloned()
    } else {
        None
    }
    .unwrap_or_else(|| {
        thread_view.read(cx).title()
            .as_ref()
            .filter(|title| !title.is_empty())
            .cloned()
            .unwrap_or_else(|| SharedString::new_static("New Thread"))
    });
    
    // Добавить как новую вкладку вместо замены текущего вида
    self.add_new_tab(
        tab_title,
        ActiveView::ExternalAgentThread { thread_view },
        !loading,
        window,
        cx,
    );
}

// Обновить new_text_thread()
fn new_text_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    // ... существующий код создания text_thread_editor ...
    
    let text_thread_view = ActiveView::text_thread(
        text_thread_editor.clone(),
        self.language_registry.clone(),
        window,
        cx,
    );
    
    // Добавить как новую вкладку вместо замены текущего вида
    self.add_new_tab("New Text Thread", text_thread_view, true, window, cx);
    
    text_thread_editor.focus_handle(cx).focus(window, cx);
}
```

### Шаг 6: Обновить сериализацию для сохранения вкладок

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Добавьте структуры для сериализации:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializedAgentPanel {
    width: Option<Pixels>,
    selected_agent: Option<AgentType>,
    tabs: Vec<SerializedTab>,
    active_tab_index: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializedTab {
    id: uuid::Uuid,
    title: SharedString,
    tab_type: SerializedTabType,
    created_at: u64,
    session_id: Option<acp_thread::SessionId>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum SerializedTabType {
    ExternalAgentThread,
    TextThread,
    History,
    Configuration,
}
```

Обновите метод `serialize()`:

```rust
fn serialize(&mut self, cx: &mut Context<Self>) {
    let width = self.width;
    let selected_agent = self.selected_agent.clone();
    
    // Сериализовать вкладки
    let tabs = self.tabs.tabs().iter().map(|tab| {
        let tab_type = match &tab.active_view {
            ActiveView::ExternalAgentThread { .. } => SerializedTabType::ExternalAgentThread,
            ActiveView::TextThread { .. } => SerializedTabType::TextThread,
            ActiveView::History { .. } => SerializedTabType::History,
            ActiveView::Configuration => SerializedTabType::Configuration,
        };
        
        SerializedTab {
            id: tab.id,
            title: tab.title.clone(),
            tab_type,
            created_at: tab.created_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            session_id: tab.session_id.clone(),
        }
    }).collect();
    
    // ... существующий код сохранения ...
}
```

### Шаг 7: Обновить загрузку для восстановления вкладок

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Добавьте в метод `load()`:

```rust
// Внутри Task в load()
if let Some(serialized_panel) = serialized_panel {
    panel.update(cx, |panel, cx| {
        // ... существующий код восстановления width и selected_agent ...
        
        // ВОССТАНОВИТЬ вкладки
        if !serialized_panel.tabs.is_empty() {
            panel.restore_tabs(
                serialized_panel.tabs,
                serialized_panel.active_tab_index,
                window,
                cx,
            );
        }
        
        cx.notify();
    });
}
```

Добавьте метод восстановления:

```rust
/// Восстанавливает вкладки из сериализованных данных
fn restore_tabs(
    &mut self,
    serialized_tabs: Vec<SerializedTab>,
    active_tab_index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    self.tabs.clear();
    
    for serialized_tab in &serialized_tabs {
        let title = serialized_tab.title.clone();
        let active_view = match serialized_tab.tab_type {
            SerializedTabType::ExternalAgentThread => {
                if let Some(session_id) = &serialized_tab.session_id {
                    if let Some(thread_info) = self.acp_history.read(cx).session_for_id(session_id) {
                        self.create_thread_for_tab(thread_info.clone(), window, cx)
                    } else {
                        self.create_default_native_thread(window, cx)
                    }
                } else {
                    self.create_default_native_thread(window, cx)
                }
            }
            SerializedTabType::TextThread => {
                // Создать новый текстовый поток
                let context = self.text_thread_store
                    .update(cx, |store, cx| store.create(cx));
                let lsp_adapter_delegate = make_lsp_adapter_delegate(&self.project, cx)
                    .log_err()
                    .flatten();
                
                let text_thread_editor = cx.new(|cx| {
                    let mut editor = TextThreadEditor::for_text_thread(
                        context,
                        self.fs.clone(),
                        self.workspace.clone(),
                        self.project.clone(),
                        lsp_adapter_delegate,
                        window,
                        cx,
                    );
                    editor.insert_default_prompt(window, cx);
                    editor
                });
                
                ActiveView::text_thread(
                    text_thread_editor.clone(),
                    self.language_registry.clone(),
                    window,
                    cx,
                )
            }
            SerializedTabType::History => ActiveView::History {
                kind: HistoryKind::AgentThreads,
            },
            SerializedTabType::Configuration => ActiveView::Configuration,
        };
        
        let mut tab = AgentTab::new(title, active_view);
        tab.id = serialized_tab.id;
        tab.session_id = serialized_tab.session_id.clone();
        
        self.tabs.add_tab(tab);
    }
    
    // Выбрать предыдущую активную вкладку
    if !self.tabs.is_empty() {
        let index_to_select = if active_tab_index < self.tabs.tab_count() {
            active_tab_index
        } else {
            0
        };
        self.select_tab(index_to_select, window, cx);
    }
}
```

### Шаг 8: Обновить render() для отображения вкладок

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs`

Модифицируйте существующий метод `render()`:

```rust
impl Render for AgentPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        
        v_flex()
            .size_full()
            .bg(theme.colors().panel_background)
            // Рендерить панель вкладок если есть вкладки
            .when(!self.tabs.is_empty(), |this| {
                this.child(self.render_tab_bar(window, cx))
            })
            // Рендерить активный вид
            .when_some(self.active_tab(), |this, tab| {
                this.child(self.render_active_view(&tab.active_view, window, cx))
            })
            // Рендерить существующий UI (тулбар, контент)
            .child(self.render_toolbar(window, cx))
            .child(self.render_content(window, cx))
            // Рендерить модальные окна
            .child(self.render_onboarding(window, cx))
            .child(self.render_trial_end_upsell(window, cx))
            .child(self.render_configuration_error(window, cx))
    }
}

fn render_tab_bar(
    &self,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> impl IntoElement {
    let theme = cx.theme();
    
    AgentTabsBar::new(
        self.tabs().to_vec(),
        self.active_tab_index,
    )
    .on_close_tab({
        let panel_entity = cx.entity().downgrade();
        move |tab_id, window, cx| {
            if let Some(panel) = panel_entity.upgrade() {
                panel.update(cx, |panel, cx| {
                    if let Some(index) = panel.tabs().iter().position(|t| t.id == tab_id) {
                        panel.close_tab(index, window, cx);
                    }
                });
            }
        }
    })
    .on_select_tab({
        let panel_entity = cx.entity().downgrade();
        move |index, window, cx| {
            if let Some(panel) = panel_entity.upgrade() {
                panel.update(cx, |panel, cx| {
                    panel.select_tab(index, window, cx);
                });
            }
        }
    })
    .on_middle_click_tab({
        let panel_entity = cx.entity().downgrade();
        move |tab_id, window, cx| {
            if let Some(panel) = panel_entity.upgrade() {
                panel.update(cx, |panel, cx| {
                    if let Some(index) = panel.tabs().iter().position(|t| t.id == tab_id) {
                        if panel.tabs().len() > 1 {
                            panel.close_tab(index, window, cx);
                        }
                    }
                });
            }
        }
    })
    .render(window, cx)
}

fn render_active_view(
    &self,
    active_view: &ActiveView,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> impl IntoElement {
    match active_view {
        ActiveView::ExternalAgentThread { thread_view } => {
            thread_view.read(cx).clone().into_any()
        }
        ActiveView::TextThread { 
            text_thread_editor,
            title_editor,
            buffer_search_bar,
            .. 
        } => {
            v_flex()
                .size_full()
                .child(
                    div().h(px(32.)).child(title_editor.read(cx).clone())
                )
                .child(
                    div().flex_1().child(text_thread_editor.read(cx).clone())
                )
                .into_any()
        }
        ActiveView::History { kind } => {
            self.render_history_view(kind, window, cx).into_any()
        }
        ActiveView::Configuration => {
            self.render_configuration_view(window, cx).into_any()
        }
    }
}
```

## Регистрация действий

### Шаг 9: Зарегистрировать обработчики действий

**Файл:** `zed/crates/agent_ui/src/agent_panel.rs` в функции `init()`

Добавьте регистрацию действий для навигации по вкладкам:

```rust
pub fn init(cx: &mut App) {
    // ... существующая инициализация ...
    
    cx.observe_new(
        |workspace: &mut Workspace, _window, _: &mut Context<Workspace>| {
            // ... существующие действия ...
            
            // ДОБАВИТЬ: Регистрация действий вкладок
            workspace.register_action(|workspace, _: &NextAgentTab, window, cx| {
                if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
                    panel.update(cx, |panel, cx| panel.next_tab(window, cx));
                }
            });
            
            workspace.register_action(|workspace, _: &PreviousAgentTab, window, cx| {
                if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
                    panel.update(cx, |panel, cx| panel.previous_tab(window, cx));
                }
            });
            
            workspace.register_action(|workspace, _: &NewAgentTab, window, cx| {
                if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
                    panel.update(cx, |panel, cx| {
                        panel.new_thread(&NewThread, window, cx);
                    });
                }
            });
            
            workspace.register_action(|workspace, _: &CloseAgentTab, window, cx| {
                if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
                    panel.update(cx, |panel, cx| {
                        if panel.tabs().len() > 1 {
                            panel.close_tab(panel.active_tab_index, window, cx);
                        }
                    });
                }
            });
        },
    )
    .detach();
    
    // ... остальной код ...
}
```

## Клавиатурные сокращения

### Шаг 10: Добавить привязки клавиш

**Файл:** `zed/assets/keymaps/default-linux.json`

Добавьте секцию для действий вкладок:

```json
{
  "context": "AgentPanel || AcpThreadView",
  "bindings": {
    "ctrl-tab": "agent::NextAgentTab",
    "ctrl-shift-tab": "agent::PreviousAgentTab",
    "ctrl-t": "agent::NewAgentTab",
    "ctrl-w": "agent::CloseAgentTab",
    "ctrl-1": "agent::SelectAgentTab1",
    "ctrl-2": "agent::SelectAgentTab2",
    "ctrl-3": "agent::SelectAgentTab3",
    "ctrl-4": "agent::SelectAgentTab4",
    "ctrl-5": "agent::SelectAgentTab5",
    "ctrl-6": "agent::SelectAgentTab6",
    "ctrl-7": "agent::SelectAgentTab7",
    "ctrl-8": "agent::SelectAgentTab8",
    "ctrl-9": "agent::SelectAgentTab9"
  }
}
```

Аналогичные изменения для `default-macos.json` и `default-windows.json`:
- macOS: используйте `cmd` вместо `ctrl`
- Windows: используйте `ctrl`

## Дополнительные действия для вкладок

Для полной функциональности добавьте следующие действия:

**Файл:** `zed/crates/agent_ui/src/agent_ui.rs`

```rust
actions!(
    agent,
    [
        // ... существующие действия ...
        
        // ДОБАВИТЬ: Действия для выбора конкретных вкладок
        SelectAgentTab1,
        SelectAgentTab2,
        SelectAgentTab3,
        SelectAgentTab4,
        SelectAgentTab5,
        SelectAgentTab6,
        SelectAgentTab7,
        SelectAgentTab8,
        SelectAgentTab9,
    ]
);
```

Обработчики для этих действий:

```rust
workspace.register_action(|workspace, _: &SelectAgentTab1, window, cx| {
    if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
        panel.update(cx, |panel, cx| {
            if panel.tabs().len() > 0 {
                panel.select_tab(0, window, cx);
            }
        });
    }
});

// Повторите для SelectAgentTab2-9 с индексами 1-8
```

## Тестирование

### Unit тесты

Запустите unit тесты для модуля tabs:

```bash
cd zed
cargo test -p agent_ui --lib tabs::tests
```

### Интеграционные тесты

Создайте тест для проверки функциональности вкладок:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[gpui::test]
    async fn test_multiple_concurrent_threads(cx: &mut TestAppContext) {
        let workspace = cx.add_window(|cx| Workspace::test(cx));
        let agent_panel = workspace.read(cx).panel::<AgentPanel>(cx).unwrap();
        
        // Создать несколько потоков
        for _ in 0..3 {
            agent_panel.update(cx, |panel, window, cx| {
                panel.new_thread(&NewThread, window, cx);
            });
        }
        
        // Проверить, что все вкладки существуют
        let panel = agent_panel.read(cx);
        assert_eq!(panel.tabs().len(), 3);
        
        // Проверить переключение между вкладками
        agent_panel.update(cx, |panel, window, cx| {
            panel.select_tab(0, window, cx);
            assert_eq!(panel.active_tab_index(), 0);
            
            panel.next_tab(window, cx);
            assert_eq!(panel.active_tab_index(), 1);
            
            panel.previous_tab(window, cx);
            assert_eq!(panel.active_tab_index(), 0);
        });
    }
    
    #[gpui::test]
    async fn test_tab_persistence(cx: &mut TestAppContext) {
        let workspace = cx.add_window(|cx| Workspace::test(cx));
        let agent_panel = workspace.read(cx).panel::<AgentPanel>(cx).unwrap();
        
        // Создать вкладки
        agent_panel.update(cx, |panel, window, cx| {
            panel.new_thread(&NewThread, window, cx);
            panel.new_thread(&NewThread, window, cx);
        });
        
        // Сериализовать
        agent_panel.update(cx, |panel, cx| {
            panel.serialize(cx);
        });
        
        // Симулировать перезагрузку
        let new_panel = AgentPanel::load(&workspace, &mut cx.window(0), &mut cx).await.ok().unwrap();
        
        // Проверить восстановление
        let panel = new_panel.read(cx);
        assert_eq!(panel.tabs().len(), 2);
    }
}
```

### Ручное тестирование

1. Создайте несколько потоков через Ctrl-N
2. Проверьте, что вкладки отображаются
3. Переключайтесь между вкладками через Ctrl-Tab
4. Закройте вкладки через кнопку X или Ctrl-W
5. Перезагрузите Zed и проверьте сохранение вкладок
6. Тестируйте с 10+ вкладками
7. Проверьте состояние при закрытии последней вкладки

## Проверка реализации

После завершения интеграции:

- [ ] Компилируется без ошибок
- [ ] Unit тесты проходят
- [ ] Интеграционные тесты проходят
- [ ] Клавиатурные сокращения работают
- [ ] Вкладки создаются при новом потоке
- [ ] Вкладки переключаются корректно
- [ ] Вкладки закрываются корректно
- [ ] Состояние сохраняется и восстанавливается
- [ ] UI отображается корректно
- [ ] Нет утечек памяти
- [ ] Производительность приемлема с множеством вкладок

## Возможные проблемы и решения

### Проблема: Несовпадение типов ActiveView
**Решение:** Убедитесь, что `ActiveView` импортируется из правильного модуля

### Проблема: Вкладки не сохраняются
**Решение:** Проверьте, что метод `serialize()` включает сериализацию вкладок

### Проблема: Переключение вкладок не работает
**Решение:** Убедитесь, что метод `select_tab()` обновляет `active_view`

### Проблема: Ошибки компиляции
**Решение:** Проверьте, что все необходимые типы импортированы

```rust
use crate::tabs::{AgentTab, AgentTabs};
use crate::agent_panel::ActiveView;
```

## Производительность

### Оптимизации

1. **Ленивая загрузка:** Загружать контент вкладки только при переключении
2. **Лимит вкладок:** Ограничить максимальное количество (например, 20)
3. **Debounce сериализации:** Сохранять состояние не при каждом изменении
4. **Virtual scroll:** Использовать виртуальный скроллинг для таб бара при большом количестве

### Мониторинг

Добавьте логирование для отслеживания:

```rust
fn add_new_tab(...) {
    log::info!("Added new tab: {}", title);
    // ... код ...
}

fn select_tab(...) {
    log::info!("Selected tab {}: {}", index);
    // ... код ...
}
```

## Следующие улучшения

После базовой реализации можно добавить:

1. **Drag-and-drop для переупорядочивания вкладок**
2. **Пин-вкладки для важных потоков**
3. **Группы вкладок по темам**
4. **Поиск/фильтрация вкладок**
5. **Закрепление вкладок**
6. **Контекстное меню правой кнопки мыши**
7. **Горячие клавиши для закрытия N вкладок**
8. **Экспорт/импорт вкладок как проекта**

## Заключение

Эта реализация предоставляет:

✅ **Минимальные изменения** в существующей архитектуре
✅ **Полная поддержка** множественных одновременных разговоров
✅ **Надежная сериализация** для сохранения состояния
✅ **Интуитивный UI** с панелью вкладок
✅ **Гибкая архитектура** для будущих улучшений

Общий объем кода: ~800 строк
Время интеграции: 2-3 дня для опытного разработчика
Сложность: Средняя

Для вопросов или проблем обратитесь к документации модуля tabs или примерам кода.