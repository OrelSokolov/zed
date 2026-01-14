//! Tab Rendering Methods for AgentPanel
//!
//! This file provides the tab rendering methods that would be integrated
//! into AgentPanel to support multiple concurrent conversations.

use gpui::{App, Context, Entity, Window};
use ui::{prelude::*, *};
use uuid::Uuid;

use crate::agent_panel::ActiveView;
use crate::tabs::{AgentTab, AgentTabsBar};

impl AgentPanel {
    // ============================================================================
    // TAB BAR RENDERING
    // ============================================================================

    /// Renders the tab bar for agent panel
    ///
    /// This displays all available tabs with appropriate styling
    /// and handles user interactions for selecting and closing tabs.
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

    /// Renders an individual tab item
    ///
    /// This is called by the tabs bar component for each tab.
    fn render_tab_item(
        &self,
        index: usize,
        tab: &AgentTab,
        is_active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

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
            .child(self.render_tab_title(tab))
            .when(self.tabs().len() > 1, |this| {
                this.child(self.render_tab_close_button(index, window, cx))
            })
            .on_click(cx.listener(move |this, event, window, cx| {
                event.stop_propagation();
                this.select_tab(index, window, cx);
            }))
    }

    /// Renders the title text for a tab
    ///
    /// Displays the tab title with appropriate styling
    /// and truncation if it's too long.
    fn render_tab_title(&self, tab: &AgentTab) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_sm()
            .when(tab.is_modified, |this| this.italic())
            .text_color(theme.colors().text)
            .child(tab.title.clone())
    }

    /// Renders the close button for a tab
    ///
    /// Shows an X button that closes the tab when clicked.
    fn render_tab_close_button(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        IconButton::new(
            format!("close-tab-{}", self.tabs()[index].id),
            IconName::Close,
        )
        .icon_size(IconSize::XSmall)
        .xsmall()
        .rounded()
        .on_click(cx.listener(move |this, event, window, cx| {
            event.stop_propagation();
            if this.tabs().len() > 1 {
                this.close_tab(index, window, cx);
            }
        }))
    }

    /// Renders an empty state when no tabs exist
    ///
    /// Shown when the agent panel has no active conversations.
    fn render_empty_state(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(theme.colors().panel_background)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(
                        ui::Icon::new(IconName::ZedAssistant)
                            .size(px(64.))
                            .text_color(theme.colors().text_muted),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(ui::FontWeight::Medium)
                            .text_color(theme.colors().text_muted)
                            .child("No Active Conversations"),
                    )
                    .child(
                        ui::Button::new("start-new-thread", "Start New Conversation")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_thread(&super::NewThread, window, cx);
                            })),
                    ),
            )
    }

    /// Renders the active view content
    ///
    /// Displays the content of the currently selected tab.
    fn render_active_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.active_tab().map(|tab| match &tab.active_view {
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
                        div()
                            .h(px(32.))
                            .child(title_editor.read(cx).clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(text_thread_editor.read(cx).clone()),
                    )
                    .into_any()
            }
            ActiveView::History { kind } => {
                self.render_history_view(kind, window, cx).into_any()
            }
            ActiveView::Configuration => {
                self.render_configuration_view(window, cx).into_any()
            }
        })
    }

    /// Renders the history view
    ///
    /// Displays conversation history for the selected agent type.
    fn render_history_view(
        &self,
        kind: &crate::HistoryKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match kind {
            crate::HistoryKind::AgentThreads => {
                self.acp_history.read(cx).clone().into_any()
            }
            crate::HistoryKind::TextThreads => {
                self.text_thread_history.read(cx).clone().into_any()
            }
        }
    }

    /// Renders the configuration view
    ///
    /// Displays agent settings and configuration options.
    fn render_configuration_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.configuration
            .as_ref()
            .map(|config| config.read(cx).clone().into_any())
            .unwrap_or_else(|| {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .child("Loading configuration...")
                    .into_any()
            })
    }

    // ============================================================================
    // ACTION HANDLERS FOR TAB NAVIGATION
    // ============================================================================

    /// Handles the NextAgentTab action
    ///
    /// Navigates to the next tab in the list.
    fn handle_next_tab(&mut self, _: &super::NextAgentTab, window: &mut Window, cx: &mut Context<Self>) {
        self.next_tab(window, cx);
    }

    /// Handles the PreviousAgentTab action
    ///
    /// Navigates to the previous tab in the list.
    fn handle_previous_tab(&mut self, _: &super::PreviousAgentTab, window: &mut Window, cx: &mut Context<Self>) {
        self.previous_tab(window, cx);
    }

    /// Handles the NewAgentTab action
    ///
    /// Creates a new tab with a fresh conversation.
    fn handle_new_tab(&mut self, _: &super::NewAgentTab, window: &mut Window, cx: &mut Context<Self>) {
        self.new_thread(&super::NewThread, window, cx);
    }

    /// Handles the CloseAgentTab action
    ///
    /// Closes the currently active tab (if not the last one).
    fn handle_close_tab(&mut self, _: &super::CloseAgentTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs().len() > 1 {
            self.close_tab(self.active_tab_index, window, cx);
        }
    }

    // ============================================================================
    // RENDER MODIFICATIONS FOR TABS SUPPORT
    // ============================================================================

    /// Modified render method that includes tab bar
    ///
    /// This replaces the existing render method to include the tab bar.
    pub fn render_with_tabs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.colors().panel_background)
            // Render tab bar if we have tabs
            .when(!self.tabs().is_empty(), |this| {
                this.child(self.render_tab_bar(window, cx))
            })
            // Render active view
            .when_some(self.active_tab(), |this, tab| {
                this.child(self.render_active_view(window, cx))
            })
            // Render empty state
            .when(self.tabs().is_empty(), |this| {
                this.child(self.render_empty_state(window, cx))
            })
    }
}

// ============================================================================
// INTEGRATION NOTES
// ============================================================================

/*
To integrate these tab rendering methods into AgentPanel:

1. Add this file content to AgentPanel implementation
2. Modify the existing render() method to call render_with_tabs()
3. Register action handlers in init():

   cx.observe_new(|workspace, window, cx| {
       workspace.register_action(|workspace, _: &NextAgentTab, window, cx| {
           if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
               panel.update(cx, |panel, cx| panel.handle_next_tab(&NextAgentTab, window, cx));
           }
       });

       workspace.register_action(|workspace, _: &PreviousAgentTab, window, cx| {
           if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
               panel.update(cx, |panel, cx| panel.handle_previous_tab(&PreviousAgentTab, window, cx));
           }
       });

       workspace.register_action(|workspace, _: &NewAgentTab, window, cx| {
           if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
               panel.update(cx, |panel, cx| panel.handle_new_tab(&NewAgentTab, window, cx));
           }
       });

       workspace.register_action(|workspace, _: &CloseAgentTab, window, cx| {
           if let Some(panel) = workspace.panel::<AgentPanel>(cx) {
               panel.update(cx, |panel, cx| panel.handle_close_tab(&CloseAgentTab, window, cx));
           }
       });
   })
   .detach();

4. Add keybindings in keymap files:

   In zed/assets/keymaps/default-linux.json:
   {
     "context": "AgentPanel || AcpThreadView",
     "bindings": {
       "ctrl-tab": "agent::NextAgentTab",
       "ctrl-shift-tab": "agent::PreviousAgentTab",
       "ctrl-t": "agent::NewAgentTab",
       "ctrl-w": "agent::CloseAgentTab"
     }
   }

5. Update tests to verify tab functionality

*/
