// Example: How to integrate tabs into AgentPanel for concurrent Zed-Agent conversations
//
// This example demonstrates the modifications needed to add tab support to AgentPanel,
// allowing multiple Zed-Agent conversations to work simultaneously.

use std::time::Instant;
use uuid::Uuid;
use gpui::{App, Context, Entity, SharedString, Window};
use ui::prelude::*;

// ============================================================================
// PART 1: TAB STRUCTURE
// ============================================================================

/// Represents a single tab in the agent panel
#[derive(Clone, Debug)]
pub struct AgentTab {
    pub id: Uuid,
    pub title: SharedString,
    pub active_view: ActiveView,
    pub created_at: Instant,
    pub is_active: bool,
}

impl AgentTab {
    pub fn new(title: impl Into<SharedString>, active_view: ActiveView) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            active_view,
            created_at: Instant::now(),
            is_active: false,
        }
    }
}

// ============================================================================
// PART 2: AGENT PANEL MODIFICATIONS
// ============================================================================

/// The modified AgentPanel struct with tabs support
pub struct AgentPanel {
    // ... existing fields ...

    // NEW: Tabs management
    tabs: Vec<AgentTab>,
    active_tab_index: usize,

    // ... other existing fields ...
}

impl AgentPanel {
    pub fn new(/* existing parameters */) -> Self {
        Self {
            // ... existing initialization ...

            // NEW: Initialize tabs with an empty state
            tabs: Vec::new(),
            active_tab_index: 0,

            // ... other initialization ...
        }
    }

    // ========================================================================
    // EXISTING METHOD MODIFICATION: set_active_view → add_new_tab
    // ========================================================================

    /// OLD METHOD (to be replaced):
    /// fn set_active_view(&mut self, new_view: ActiveView, focus: bool, ...)
    ///
    /// NEW METHOD: Add a new tab instead of replacing the current view
    pub fn add_new_tab(
        &mut self,
        title: impl Into<SharedString>,
        active_view: ActiveView,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create new tab
        let tab = AgentTab::new(title, active_view);

        // Deactivate all existing tabs
        for tab in &mut self.tabs {
            tab.is_active = false;
        }

        // Add the new tab
        self.tabs.push(tab);
        let index = self.tabs.len() - 1;
        self.tabs[index].is_active = true;
        self.active_tab_index = index;

        // Handle focus if requested
        if focus {
            self.focus_handle(cx).focus(window, cx);
        }

        // Notify to trigger re-render
        cx.notify();

        // Optional: Auto-save tabs state
        self.serialize(cx);
    }

    // ========================================================================
    // NEW METHODS FOR TAB MANAGEMENT
    // ========================================================================

    /// Select a tab by index
    pub fn select_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }

        // Deactivate current tab
        if let Some(current) = self.tabs.get_mut(self.active_tab_index) {
            current.is_active = false;
        }

        // Activate new tab
        self.active_tab_index = index;
        self.tabs[index].is_active = true;

        cx.notify();
    }

    /// Select a tab by ID
    pub fn select_tab_by_id(
        &mut self,
        id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
            self.select_tab(index, window, cx);
        }
    }

    /// Close a tab by index
    pub fn close_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }

        // Don't close the last tab - replace with a new empty thread instead
        if self.tabs.len() == 1 {
            self.new_thread(&NewThread, window, cx);
            return;
        }

        // Remove the tab
        self.tabs.remove(index);

        // Adjust active index if needed
        if index <= self.active_tab_index {
            self.active_tab_index = self.active_tab_index.saturating_sub(1);
        }

        // Ensure we have an active tab
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }

        self.tabs[self.active_tab_index].is_active = true;

        cx.notify();
        self.serialize(cx);
    }

    /// Move to the next tab
    pub fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }

        let new_index = (self.active_tab_index + 1) % self.tabs.len();
        self.select_tab(new_index, window, cx);
    }

    /// Move to the previous tab
    pub fn previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }

        let new_index = if self.active_tab_index == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab_index - 1
        };

        self.select_tab(new_index, window, cx);
    }

    /// Get the active tab
    pub fn active_tab(&self) -> Option<&AgentTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Get all tabs
    pub fn tabs(&self) -> &[AgentTab] {
        &self.tabs
    }

    // ========================================================================
    // MODIFIED EXISTING METHODS
    // ========================================================================

    /// MODIFIED: Instead of replacing active_view, create a new tab
    fn new_thread(&mut self, _action: &NewThread, window: &mut Window, cx: &mut Context<Self>) {
        // OLD CODE:
        // self.new_agent_thread(AgentType::NativeAgent, window, cx);

        // NEW CODE: Create a new thread and add it as a tab
        let title = "New Thread";

        // Create the thread view (same as before)
        let thread_view = self.create_native_agent_thread(window, cx);

        // Add as a new tab instead of replacing the current view
        self.add_new_tab(
            title,
            ActiveView::ExternalAgentThread { thread_view },
            true,
            window,
            cx,
        );
    }

    /// MODIFIED: Create a new tab for the thread instead of replacing
    fn load_agent_thread(
        &mut self,
        thread_info: AgentSessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // OLD CODE:
        // self.external_thread(Some(ExternalAgent::NativeAgent), Some(thread_info), ...);

        // NEW CODE: Create the thread and add as a new tab
        let title = thread_info.title.clone().unwrap_or_else(|| "Thread".into());
        let thread_view = self.load_thread_from_session(thread_info, window, cx);

        self.add_new_tab(
            title,
            ActiveView::ExternalAgentThread { thread_view },
            true,
            window,
            cx,
        );
    }

    // ========================================================================
    // RENDER MODIFICATIONS
    // ========================================================================

    /// MODIFIED: Render the tab bar above the active view
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.colors().panel_background)
            .when(!self.tabs.is_empty(), |this| {
                // Render tab bar
                this.child(self.render_tab_bar(window, cx))
            })
            .when_some(self.active_tab(), |this, tab| {
                // Render the active view
                this.child(self.render_active_view(&tab.active_view, window, cx))
            })
            .when(self.tabs.is_empty(), |this| {
                // Show empty state or create first tab
                this.child(self.render_empty_state(window, cx))
            })
    }

    /// Render the tab bar UI
    fn render_tab_bar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .h(px(32.))
            .border_b_1()
            .border_color(theme.colors().border)
            .gap(px(2.))
            .px_2()
            .items_center()
            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                let is_active = index == self.active_tab_index;
                let tab_id = tab.id;

                div()
                    .id(ElementId::Named(format!("agent-tab-{}", tab.id).into()))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_full()
                    .min_w(px(100.))
                    .max_w(px(200.))
                    .px_2()
                    .gap_2()
                    .rounded_t_md()
                    .cursor_pointer()
                    .when(is_active, |this| {
                        this.bg(theme.colors().surface_background)
                            .border_b_2()
                            .border_color(theme.colors().border_selected)
                    })
                    .when(!is_active, |this| {
                        this.hover(|style| style.bg(theme.colors().surface_hover))
                    })
                    .child(
                        div()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_sm()
                            .text_color(theme.colors().text)
                            .child(tab.title.clone()),
                    )
                    .when(self.tabs.len() > 1, |this| {
                        this.child(
                            IconButton::new(format!("close-tab-{}", tab.id), IconName::Close)
                                .icon_size(IconSize::XSmall)
                                .xsmall()
                                .rounded()
                                .on_click(cx.listener(move |this, event, window, cx| {
                                    event.stop_propagation();
                                    this.close_tab(index, window, cx);
                                })),
                        )
                    })
                    .on_click(cx.listener(move |this, event, window, cx| {
                        event.stop_propagation();
                        this.select_tab(index, window, cx);
                    }))
            }))
    }

    /// Render the active view content
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
            ActiveView::TextThread { text_thread_editor, .. } => {
                text_thread_editor.read(cx).clone().into_any()
            }
            ActiveView::History { .. } => {
                // Render history view
                div().child("History View").into_any()
            }
            ActiveView::Configuration => {
                // Render configuration view
                div().child("Configuration View").into_any()
            }
        }
    }

    /// Render empty state when no tabs exist
    fn render_empty_state(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .text_color(cx.theme().colors().text_muted)
                    .child("No active conversations"),
            )
            .child(
                ui::Button::new("start-new-thread", "Start New Conversation")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.new_thread(&NewThread, window, cx);
                    })),
            )
    }

    // ========================================================================
    // SERIALIZATION MODIFICATIONS
    // ========================================================================

    /// MODIFIED: Save tabs state
    fn serialize(&mut self, cx: &mut Context<Self>) {
        // Save active tab index and minimal tab information
        let serialized = SerializedAgentPanel {
            width: self.width,
            selected_agent: self.selected_agent,
            active_tab_index: self.active_tab_index,
            tab_count: self.tabs.len(),
            // ... other fields
        };

        // Write to key-value store
        let serialized = serde_json::to_string(&serialized).ok();
        if let Some(data) = serialized {
            cx.background_spawn({
                async move {
                    db::kvp::KEY_VALUE_STORE
                        .write_kvp("agent_panel_state".to_string(), data)
                        .await
                        .log_err();
                }
            })
            .detach();
        }
    }

    // ========================================================================
    // HELPER METHODS (to be implemented)
    // ========================================================================

    fn create_native_agent_thread(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<AcpThreadView> {
        // Implementation would create the thread view
        // Similar to existing code in AgentPanel::_external_thread
        todo!("Implement thread creation")
    }

    fn load_thread_from_session(
        &mut self,
        thread_info: AgentSessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<AcpThreadView> {
        // Implementation would load thread from session
        todo!("Implement thread loading")
    }
}

// ============================================================================
// PART 3: ACTION HANDLERS
// ============================================================================

/// Register tab navigation actions
pub fn register_tab_actions(cx: &mut App) {
    // Register next tab action
    cx.register_action(|workspace, _: &NextAgentTab, window, cx| {
        if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
            panel.update(cx, |panel, cx| panel.next_tab(window, cx));
        }
    });

    // Register previous tab action
    cx.register_action(|workspace, _: &PreviousAgentTab, window, cx| {
        if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
            panel.update(cx, |panel, cx| panel.previous_tab(window, cx));
        }
    });

    // Register new tab action
    cx.register_action(|workspace, _: &NewAgentTab, window, cx| {
        if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
            panel.update(cx, |panel, cx| panel.new_thread(&NewThread, window, cx));
        }
    });

    // Register close tab action
    cx.register_action(|workspace, _: &CloseAgentTab, window, cx| {
        if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
            panel.update(cx, |panel, cx| {
                if panel.tabs().len() > 1 {
                    panel.close_tab(panel.active_index(), window, cx);
                }
            });
        }
    });
}

// ============================================================================
// PART 4: KEYBINDINGS
// ============================================================================

/// Example keybindings for tab navigation (to be added to keymap files)
pub const TAB_NAVIGATION_KEYBINDINGS: &str = r#"
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
    "ctrl-5": "agent::SelectAgentTab5"
  }
}
"#;

// ============================================================================
// PART 5: SUMMARY OF CHANGES
// ============================================================================

/// This example demonstrates the following changes to enable tabs:
///
/// 1. **Struct Changes:**
///    - Added `tabs: Vec<AgentTab>` to store all tabs
///    - Added `active_tab_index: usize` to track which tab is active
///
/// 2. **Method Changes:**
///    - `set_active_view()` → `add_new_tab()` - creates tabs instead of replacing
///    - `new_thread()` - now creates new tabs
///    - `load_agent_thread()` - now creates new tabs
///
/// 3. **New Methods:**
///    - `select_tab(index)` - switch to a specific tab
///    - `select_tab_by_id(id)` - switch by tab ID
///    - `close_tab(index)` - close a tab
///    - `next_tab()` - navigate to next tab
///    - `previous_tab()` - navigate to previous tab
///    - `active_tab()` - get the active tab
///    - `tabs()` - get all tabs
///
/// 4. **UI Changes:**
///    - Added `render_tab_bar()` - renders the tab bar
///    - Modified `render()` - includes tab bar and handles empty state
///
/// 5. **Persistence:**
///    - Modified `serialize()` - saves tabs state
///    - Need to add `load()` deserialization for tabs
///
/// 6. **Actions:**
///    - Need to add action definitions for tab navigation
///    - Register action handlers for tab management
///
/// BENEFITS:
/// - Multiple conversations can work simultaneously
/// - Users can switch between conversations without losing state
/// - Each conversation maintains its own history and context
/// - Familiar tab-based UI pattern
/// - Minimal changes to existing AcpThreadView implementation
