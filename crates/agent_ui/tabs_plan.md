# Multi-tabs Implementation Plan

## Overview
This document outlines the remaining work to complete the multi-tabs feature in the Agent Panel.

## Current Status

### ✅ Already Implemented
- Basic data structures (`AgentTab`, `AgentTabs`) with full CRUD methods
- Serialization/deserialization to JSON via `KEY_VALUE_STORE`
- Tab bar rendering with close buttons and switching
- Actions: `NextAgentTab`, `PreviousAgentTab`, `NewAgentTab`, `CloseAgentTab`
- Tab management: `add_new_tab()`, `select_tab()`, `close_tab()`, `next_tab()`, `previous_tab()`
- Async tab title updates when threads change
- State persistence

### ⚠️ Issues (Dead Code Warnings)
Many methods exist but are not integrated:
- `create_thread_for_tab()` - created but unused
- `select_tab_by_id()`, `close_tab_by_id()` - unused in AgentPanel
- `update_tab_modified()` - never called
- `with_title()`, `with_modified()` - builder methods not used
- `is_modified` field - never updated
- Sessions not loaded on restore (tabs are empty)

---

## Implementation Tasks

### Task 1: Restore Sessions from ThreadStore/TextThreadStore
**Priority:** HIGH - Core functionality

**Problem:** `restore_tabs()` creates empty threads instead of loading saved sessions.

**Requirements:**
- Load all messages from sessions (not partial)
- Handle missing/deleted sessions gracefully

**Implementation:**

1. In `agent_panel.rs`, modify `restore_tabs()` method:

```rust
fn restore_tabs(
    &mut self,
    serialized_tabs: Vec<SerializedTab>,
    active_tab_index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    // Clear default tab that was created in new()
    self.tabs.clear();
    self.tab_views.clear();

    // Restore each serialized tab
    for serialized_tab in &serialized_tabs {
        let title = serialized_tab.title.clone();

        let active_view = match serialized_tab.tab_type {
            SerializedTabType::ExternalAgentThread => {
                // Load session from ThreadStore
                if let Some(session_id) = &serialized_tab.session_id {
                    // Query ThreadStore for session info
                    if let Some(session_info) = self.load_session_info(session_id, cx) {
                        self.create_thread_for_tab(session_info, window, cx)
                    } else {
                        // Session not found, create default
                        self.create_default_native_thread(window, cx)
                    }
                } else {
                    self.create_default_native_thread(window, cx)
                }
            }
            SerializedTabType::TextThread => {
                // Load text thread from TextThreadStore
                if let Some(session_id) = &serialized_tab.session_id {
                    if let Some(text_thread) = self.load_text_thread(session_id, window, cx) {
                        let title = text_thread.read(cx).summary().or_default();
                        let lsp_adapter_delegate = make_lsp_adapter_delegate(&self.project, cx)
                            .log_err()
                            .flatten();
                        let text_thread_editor = cx.new(|cx| {
                            TextThreadEditor::for_text_thread(
                                text_thread,
                                self.fs.clone(),
                                self.workspace.clone(),
                                self.project.clone(),
                                lsp_adapter_delegate,
                                window,
                                cx,
                            )
                        });
                        ActiveView::text_thread(
                            text_thread_editor.clone(),
                            self.language_registry.clone(),
                            window,
                            cx,
                        )
                    } else {
                        // Session not found, create default
                        self.create_default_text_thread(window, cx)
                    }
                } else {
                    self.create_default_text_thread(window, cx)
                }
            }
            SerializedTabType::History => ActiveView::History {
                kind: HistoryKind::AgentThreads,
            },
            SerializedTabType::Configuration => ActiveView::Configuration,
        };

        let tab_type = match &serialized_tab.tab_type {
            SerializedTabType::ExternalAgentThread => TabType::Thread,
            SerializedTabType::TextThread => TabType::TextThread,
            SerializedTabType::History => TabType::History,
            SerializedTabType::Configuration => TabType::Configuration,
        };

        let mut tab = AgentTab::new(title, tab_type);
        tab.id = serialized_tab.id;
        tab.session_id = serialized_tab.session_id.clone();

        self.tabs.add_tab(tab);
        self.tab_views.push(Some(active_view));
    }

    // Select previously active tab
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

2. Add helper method to load session info:

```rust
fn load_session_info(
    &self,
    session_id: &agent_client_protocol::SessionId,
    cx: &Context<Self>,
) -> Option<AgentSessionInfo> {
    // Query ThreadStore for session by session_id
    // Return AgentSessionInfo if found
    self.thread_store
        .read(cx)
        .get_session(session_id)
        .ok()
        .flatten()
}
```

3. Add helper method to load text thread:

```rust
fn load_text_thread(
    &self,
    session_id: &agent_client_protocol::SessionId,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Option<Entity<TextThread>> {
    // Query TextThreadStore for thread by session_id
    self.text_thread_store
        .read(cx)
        .get_thread(session_id)
        .ok()
        .flatten()
}
```

**Tasks:**
- [ ] Implement `load_session_info()` method
- [ ] Implement `load_text_thread()` method  
- [ ] Update `restore_tabs()` to load sessions
- [ ] Add error handling for missing sessions
- [ ] Test restore with saved sessions

---

### Task 2: Track Modification State
**Priority:** HIGH - Core functionality

**Problem:** `is_modified` field in `AgentTab` is never updated.

**Requirements:**
- For TextThread: track unsaved changes
- For External Agent Thread: track new messages since last save
- Update UI to show modified indicator (italic text already implemented)

**Implementation:**

1. For Text Threads, modify `open_text_thread()`:

```rust
pub(crate) fn open_text_thread(
    &mut self,
    text_thread: Entity<TextThread>,
    window: &mut Window,
    cx: &mut Context<Self>,
) {
    let title = text_thread.read(cx).summary().or_default();
    let lsp_adapter_delegate = make_lsp_adapter_delegate(&self.project.clone(), cx)
        .log_err()
        .flatten();
    let editor = cx.new(|cx| {
        TextThreadEditor::for_text_thread(
            text_thread.clone(),
            self.fs.clone(),
            self.workspace.clone(),
            self.project.clone(),
            lsp_adapter_delegate,
            window,
            cx,
        )
    });

    if self.selected_agent != AgentType::TextThread {
        self.selected_agent = AgentType::TextThread;
        self.serialize(cx);
    }

    let tab_title = title.clone();
    self.add_new_tab(
        tab_title,
        ActiveView::text_thread(editor.clone(), self.language_registry.clone(), window, cx),
        true,
        window,
        cx,
    );

    // Get tab_id for subscription
    let tab_id = if let Some(active_tab) = self.tabs.active_tab() {
        active_tab.id
    } else {
        return;
    };

    // Subscribe to editor changes for modification tracking
    let tab_id_for_subscription = tab_id;
    cx.observe(&editor, move |this, editor, cx| {
        let is_modified = editor.read(cx).is_modified();
        if let Some(tab) = this.tabs.find_tab_by_id_mut(&tab_id_for_subscription) {
            if tab.is_modified != is_modified {
                tab.is_modified = is_modified;
                cx.notify();
            }
        }
    }).detach();
}
```

2. For External Agent Threads, modify `_external_thread()`:

```rust
// After creating the tab and thread_view_clone, add modification tracking
let tab_id_for_modification = tab_id;
cx.observe(&thread_view_clone, move |this, thread_view, cx| {
    // Determine if thread has new unsaved messages
    let is_modified = if let Some(thread) = thread_view.read(cx).thread() {
        thread.read(cx).has_unsaved_messages()
    } else {
        false
    };

    if let Some(tab) = this.tabs.find_tab_by_id_mut(&tab_id_for_modification) {
        if tab.is_modified != is_modified {
            tab.is_modified = is_modified;
            cx.notify();
        }
    }
}).detach();
```

**Tasks:**
- [ ] Add `has_unsaved_messages()` method to Thread (if not exists)
- [ ] Subscribe to TextThreadEditor changes
- [ ] Subscribe to AcpThreadView changes
- [ ] Update modification flag in tabs
- [ ] Test modification indicator display

---

### Task 3: Use `select_tab_by_id()` and `close_tab_by_id()`
**Priority:** MEDIUM - Code quality

**Problem:** Methods exist but aren't used in AgentPanel.

**Implementation:**

Replace `index_of + select_tab` with direct ID-based selection where appropriate.

1. In `render_tabs_bar()`, change tab click handler:

```rust
// Current:
panel.update(cx, |panel, cx| {
    panel.select_tab(index, window, cx);
});

// Replace with:
panel.update(cx, |panel, cx| {
    let _ = panel.tabs.select_tab_by_id(tab.id);
    // Update active view based on new selection
    if let Some(new_index) = panel.tabs.index_of(tab.id) {
        panel.select_tab(new_index, window, cx);
    }
});
```

Actually, since we need to swap the active view, we should keep using `select_tab(index)` in the UI handlers. The `select_tab_by_id()` is more useful for internal logic.

2. For closing tabs:

```rust
// In close button handler:
if let Some(panel) = panel.upgrade() {
    panel.update(cx, |panel, cx| {
        panel.tabs.close_tab_by_id(tab_id).ok();
        // Update active view
        if let Some(new_index) = panel.tabs.active_index() {
            panel.select_tab(new_index, window, cx);
        }
    });
}
```

**Tasks:**
- [ ] Identify places where ID-based selection makes code cleaner
- [ ] Replace appropriate `index_of + select/close` with ID-based methods
- [ ] Verify tab switching still works correctly

---

### Task 4: Always Show Tab Bar
**Priority:** LOW - UX improvement

**Problem:** Tab bar is hidden when `tab_count <= 1`.

**Requirements:**
- Always show tab bar (remove the condition)
- Consider settings to make it configurable in future

**Implementation:**

In `render_tabs_bar()`:

```rust
fn render_tabs_bar(&self, _window: &mut Window, cx: &Context<Self>) -> Option<AnyElement> {
    // Remove this condition:
    // if self.tabs.tab_count() <= 1 {
    //     return None;
    // }

    // Always render tab bar
    let theme = cx.theme();
    // ... rest of implementation
}
```

**Tasks:**
- [ ] Remove `tab_count <= 1` condition
- [ ] Test single tab appearance
- [ ] Ensure tab bar layout works for single tab

---

### Task 5: Prevent Configuration Tab Duplication
**Priority:** MEDIUM - UX improvement

**Problem:** Multiple Configuration tabs can be created. Similar check exists for History.

**Implementation:**

Add check in `open_configuration()` similar to `open_history()`:

```rust
fn open_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    // Check if Configuration tab already exists
    if let Some(tab) = self.tabs.find_tab_by_type(TabType::Configuration) {
        if let Some(index) = self.tabs.index_of(tab.id) {
            self.select_tab(index, window, cx);
            return;
        }
    }

    // Create configuration view...
    self.add_new_tab("Settings", ActiveView::Configuration, true, window, cx);
}
```

**Tasks:**
- [ ] Add duplicate check to `open_configuration()`
- [ ] Test that Configuration tab switches instead of duplicating

---

### Task 6: Use Builder Methods for Tab Creation
**Priority:** LOW - Code quality

**Problem:** `with_title()` and `with_modified()` exist but aren't used.

**Implementation:**

Replace direct field assignment with builder methods:

```rust
// In restore_tabs():
let mut tab = AgentTab::new(title, tab_type)
    .with_id(serialized_tab.id)
    .with_session_id(serialized_tab.session_id.clone());
```

Note: This requires adding `with_id()` and `with_session_id()` methods first.

**Tasks:**
- [ ] Add `with_id()` method to AgentTab
- [ ] Add `with_session_id()` method to AgentTab
- [ ] Use builder methods in appropriate places
- [ ] Consider removing direct field access in favor of builders

---

### Task 7: Handle Edge Cases
**Priority:** MEDIUM - Stability

**Potential Issues:**

1. **Closing last tab:** Currently creates new thread. Verify this is desired behavior.

2. **Session not found during restore:** Handle gracefully (create empty thread or skip).

3. **Tab title updates:** Ensure title updates propagate correctly when thread title changes.

4. **Tab state consistency:** Ensure `tab_views` vector stays in sync with `tabs` vector.

**Tasks:**
- [ ] Verify last tab close behavior
- [ ] Test restore with deleted sessions
- [ ] Verify title update propagation
- [ ] Add assertions for tab/views consistency

---

## Testing Checklist

After implementing all tasks, verify:

- [ ] Creating new tabs works (External Agent, Text Thread, History, Configuration)
- [ ] Switching between tabs works correctly
- [ ] Closing tabs works (single tab, middle tab, last tab)
- [ ] Tab titles update when thread titles change
- [ ] Modified indicator shows for unsaved changes
- [ ] State persists across restarts
- [ ] Sessions load correctly on restore
- [ ] No duplicate History/Configuration tabs
- [ ] Tab bar shows for single tab
- [ ] Keyboard shortcuts work (Next/Previous Tab, New Tab, Close Tab)

---

## Remaining Dead Code After Implementation

After completing the above, these warnings should be resolved:

**AgentTabs:**
- `select_tab_by_id()` - Used in duplicate prevention
- `close_tab_by_id()` - Used in close handlers
- `active_tab_mut()` - May still be unused, consider removal if truly unused
- `update_tab_modified()` - Should be used if Task 2 is implemented

**AgentTab:**
- `with_title()` - May be unused after Task 6, consider if truly needed
- `with_modified()` - May be unused after Task 6, consider if truly needed

**AgentPanel:**
- `set_active_view()` - Legacy method, can be removed if not used
- `active_tab()`, `tabs()`, `tab_count()` - Used in render, but may be candidates for removal if inline works better
- `find_tab_by_session()` - Used in update logic
- `update_tab_title()` - Used in async updates
- `create_thread_for_tab()` - Used in restore

---

## Priority Order

1. **Task 1:** Restore sessions from stores (HIGH - core functionality)
2. **Task 2:** Track modification state (HIGH - core functionality)
3. **Task 5:** Prevent Configuration duplication (MEDIUM - UX bug)
4. **Task 3:** Use ID-based methods (MEDIUM - code quality)
5. **Task 7:** Handle edge cases (MEDIUM - stability)
6. **Task 4:** Always show tab bar (LOW - UX preference)
7. **Task 6:** Use builder methods (LOW - code quality)

Start with Task 1 and Task 2 as they are critical for the feature to work correctly.