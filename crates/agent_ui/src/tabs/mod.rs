//! Tabs module for AgentPanel
//!
//! Provides tab management for multiple concurrent Zed-Agent conversations.
//!
//! # Overview
//!
//! This module implements a comprehensive tab management system for the AgentPanel,
//! allowing users to work with multiple conversations simultaneously. The system
//! supports different types of tabs (threads, text threads, history, configuration)
//! and provides methods for creating, selecting, and closing tabs.
//!
//! # Components
//!
//! - [`AgentTab`]: Represents a single conversation tab with metadata
//! - [`AgentTabs`]: Manages a collection of tabs with state and navigation
//! - [`TabType`]: Enum defining the different content types for tabs
//!
//! # Testing
//!
//! The module includes comprehensive unit and integration tests covering:
//!
//! - **Tab Creation**: Creating single and multiple tabs
//! - **Tab Selection**: Selecting tabs by index and ID
//! - **Tab Closing**: Closing tabs with proper state management
//! - **Navigation**: Next/previous tab navigation with wrapping
//! - **Edge Cases**: Closing the last tab (protected), empty tabs, etc.
//! - **State Management**: Modified flags, active states, title updates
//!
//! To run all tab-related tests:
//! ```bash
//! cargo test -p agent_ui --lib tabs::tests
//! ```
//!
//! To run a specific test:
//! ```bash
//! cargo test -p agent_ui --lib tabs::tests::test_create_single_tab
//! ```

use agent_client_protocol as acp;
use gpui::SharedString;
use std::time::Instant;

use uuid::Uuid;

/// Represents the type of content displayed in a tab
///
/// This enum describes what kind of view a tab contains without
/// storing the actual view entities (which are managed by AgentPanel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabType {
    /// An agent thread conversation
    Thread,
    /// A text-based thread
    TextThread,
    /// Thread history view
    History,
    /// Agent configuration/settings view
    Configuration,
}

/// Represents a single conversation tab in the agent panel
///
/// Each tab maintains its own conversation state, history, and context.
#[derive(Clone, Debug)]
pub struct AgentTab {
    /// Unique identifier for this tab
    pub id: Uuid,

    /// Display title shown in the tab bar
    pub title: SharedString,

    /// The type of content displayed in this tab
    pub tab_type: TabType,

    /// When this tab was created
    pub created_at: Instant,

    /// Whether this tab is currently active/selected
    pub is_active: bool,

    /// The session ID for agent threads (None for non-thread tabs)
    pub session_id: Option<acp::SessionId>,

    /// Whether this tab has unsaved changes or is currently processing
    pub is_modified: bool,
}

impl AgentTab {
    /// Creates a new agent tab
    pub fn new(title: impl Into<SharedString>, tab_type: TabType) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            tab_type,
            created_at: Instant::now(),
            is_active: false,
            session_id: None,
            is_modified: false,
        }
    }

    /// Updates tab's title
    pub fn with_title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Marks tab as modified or unmodified
    pub fn with_modified(mut self, modified: bool) -> Self {
        self.is_modified = modified;
        self
    }
}

impl Default for AgentTab {
    fn default() -> Self {
        Self::new("Untitled", TabType::Configuration)
    }
}

/// Manages multiple tabs in the agent panel
///
/// This struct provides thread-safe tab management with methods for
/// creating, selecting, and closing tabs.
#[derive(Clone)]
pub struct AgentTabs {
    tabs: Vec<AgentTab>,
    active_tab_index: usize,
}

impl AgentTabs {
    /// Creates a new empty tabs manager
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_index: 0,
        }
    }

    /// Adds a new tab and selects it
    ///
    /// This method deactivates all existing tabs, adds the new tab,
    /// and marks it as active.
    pub fn add_tab(&mut self, tab: AgentTab) -> &mut AgentTab {
        // Mark all tabs as inactive
        for tab in &mut self.tabs {
            tab.is_active = false;
        }

        // Add the new tab
        self.tabs.push(tab);
        let index = self.tabs.len() - 1;

        // Set as active
        self.tabs[index].is_active = true;
        self.active_tab_index = index;

        &mut self.tabs[index]
    }

    /// Selects a tab by index
    ///
    /// Returns `Some(&AgentTab)` if the index is valid, `None` otherwise.
    pub fn select_tab(&mut self, index: usize) -> Option<&AgentTab> {
        if index >= self.tabs.len() {
            return None;
        }

        // Mark all tabs as inactive
        for tab in &mut self.tabs {
            tab.is_active = false;
        }

        // Mark selected tab as active
        self.tabs[index].is_active = true;
        self.active_tab_index = index;

        Some(&self.tabs[index])
    }

    /// Selects a tab by its unique ID
    ///
    /// Returns `Some(&AgentTab)` if found, `None` otherwise.
    pub fn select_tab_by_id(&mut self, id: Uuid) -> Option<&AgentTab> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        self.select_tab(index)
    }

    /// Closes a tab by index
    ///
    /// The last tab cannot be closed; this method returns `None` in that case.
    /// Returns `Some(AgentTab)` with the closed tab if successful.
    pub fn close_tab(&mut self, index: usize) -> Option<AgentTab> {
        if index >= self.tabs.len() {
            return None;
        }

        // Don't allow closing the last tab
        if self.tabs.len() == 1 {
            return None;
        }

        let removed = self.tabs.remove(index);

        // Adjust active index if needed
        if index <= self.active_tab_index {
            self.active_tab_index = self.active_tab_index.saturating_sub(1);
        }

        // Ensure we have an active tab
        if !self.tabs.is_empty() && self.active_tab_index < self.tabs.len() {
            self.tabs[self.active_tab_index].is_active = true;
        } else if !self.tabs.is_empty() {
            self.active_tab_index = 0;
            self.tabs[0].is_active = true;
        }

        Some(removed)
    }

    /// Closes a tab by its unique ID
    ///
    /// Returns `Some(AgentTab)` with the closed tab if found, `None` otherwise.
    pub fn close_tab_by_id(&mut self, id: Uuid) -> Option<AgentTab> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        self.close_tab(index)
    }

    /// Returns a reference to the active tab
    pub fn active_tab(&self) -> Option<&AgentTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Returns a mutable reference to the active tab
    pub fn active_tab_mut(&mut self) -> Option<&mut AgentTab> {
        self.tabs.get_mut(self.active_tab_index)
    }

    /// Returns all tabs
    pub fn tabs(&self) -> &[AgentTab] {
        &self.tabs
    }

    /// Returns a mutable reference to all tabs
    pub fn tabs_mut(&mut self) -> &mut [AgentTab] {
        &mut self.tabs
    }

    /// Returns the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns whether there are any tabs
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Returns the active tab index
    pub fn active_index(&self) -> usize {
        self.active_tab_index
    }

    /// Moves to the next tab (wraps around to the first tab)
    pub fn next_tab(&mut self) -> Option<&AgentTab> {
        if self.tabs.is_empty() {
            return None;
        }
        let new_index = (self.active_tab_index + 1) % self.tabs.len();
        self.select_tab(new_index)
    }

    /// Moves to the previous tab (wraps around to the last tab)
    pub fn previous_tab(&mut self) -> Option<&AgentTab> {
        if self.tabs.is_empty() {
            return None;
        }
        let new_index = if self.active_tab_index == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab_index - 1
        };
        self.select_tab(new_index)
    }

    /// Updates the title of a tab by its ID
    ///
    /// Returns `true` if the tab was found and updated, `false` otherwise.
    pub fn update_tab_title(&mut self, id: Uuid, title: impl Into<SharedString>) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.title = title.into();
            true
        } else {
            false
        }
    }

    /// Updates the modified state of a tab by its ID
    ///
    /// Returns `true` if the tab was found and updated, `false` otherwise.
    pub fn update_tab_modified(&mut self, id: Uuid, modified: bool) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.is_modified = modified;
            true
        } else {
            false
        }
    }

    /// Finds a tab by its session ID (for agent threads)
    ///
    /// Returns `Some(&AgentTab)` if found, `None` otherwise.
    pub fn find_tab_by_session(&self, session_id: &acp::SessionId) -> Option<&AgentTab> {
        self.tabs
            .iter()
            .find(|tab| tab.session_id.as_ref() == Some(session_id))
    }

    /// Finds a tab by its type
    ///
    /// Returns `Some(&AgentTab)` if found, `None` otherwise.
    pub fn find_tab_by_type(&self, tab_type: TabType) -> Option<&AgentTab> {
        self.tabs.iter().find(|tab| tab.tab_type == tab_type)
    }

    /// Finds a tab by its session ID (for agent threads) - mutable version
    ///
    /// Returns `Some(&mut AgentTab)` if found, `None` otherwise.
    pub fn find_tab_by_session_mut(
        &mut self,
        session_id: &acp::SessionId,
    ) -> Option<&mut AgentTab> {
        self.tabs
            .iter_mut()
            .find(|tab| tab.session_id.as_ref() == Some(session_id))
    }

    /// Clears all tabs
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.active_tab_index = 0;
    }

    /// Returns the index of a tab by its ID
    pub fn index_of(&self, id: Uuid) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }

    /// Finds a tab by its UUID ID
    ///
    /// Returns `Some(&AgentTab)` if found, `None` otherwise.
    pub fn find_tab_by_id(&self, id: &Uuid) -> Option<&AgentTab> {
        self.tabs.iter().find(|tab| tab.id == *id)
    }

    /// Finds a mutable reference to a tab by its UUID ID
    ///
    /// Returns `Some(&mut AgentTab)` if found, `None` otherwise.
    pub fn find_tab_by_id_mut(&mut self, id: &Uuid) -> Option<&mut AgentTab> {
        self.tabs.iter_mut().find(|tab| tab.id == *id)
    }
}

impl Default for AgentTabs {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders the tab bar for the agent panel
///
/// This component displays all available tabs and handles user interactions
/// for selecting and closing tabs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_tab_creation() {
        let tab = AgentTab::default();

        assert_eq!(tab.title, "Untitled");
        assert!(!tab.is_active);
        assert!(!tab.is_modified);
        assert!(tab.session_id.is_none());
    }

    #[test]
    fn test_agent_tabs_empty() {
        let tabs = AgentTabs::new();

        assert!(tabs.is_empty());
        assert_eq!(tabs.tab_count(), 0);
        assert!(tabs.active_tab().is_none());
    }

    #[test]
    fn test_agent_tabs_add() {
        let mut tabs = AgentTabs::new();
        let tab = AgentTab::default();
        let tab_id = tab.id;
        tabs.add_tab(tab);

        assert_eq!(tabs.tab_count(), 1);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.find_tab_by_id(&tab_id).unwrap().title, "Untitled");
    }

    #[test]
    fn test_agent_tabs_multiple() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        assert_eq!(tabs.tab_count(), 3);
        assert_eq!(tabs.active_index(), 2);
        assert!(tabs.active_tab().unwrap().is_active);
    }

    #[test]
    fn test_agent_tabs_select() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        tabs.select_tab(0);

        assert_eq!(tabs.active_index(), 0);
        assert!(tabs.active_tab().unwrap().is_active);
        assert_eq!(tabs.active_tab().unwrap().title, "Tab 1");
    }

    #[test]
    fn test_agent_tabs_select_invalid() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        let result = tabs.select_tab(5);

        assert!(result.is_none());
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn test_agent_tabs_close() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        let closed = tabs.close_tab(0);

        assert!(closed.is_some());
        assert_eq!(tabs.tab_count(), 1);
        assert_eq!(tabs.active_tab().unwrap().title, "Tab 2");
    }

    #[test]
    fn test_agent_tabs_close_last_tab() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());

        let closed = tabs.close_tab(0);

        assert!(closed.is_none());
        assert_eq!(tabs.tab_count(), 1);
    }

    #[test]
    fn test_agent_tabs_close_invalid() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());

        let closed = tabs.close_tab(5);

        assert!(closed.is_none());
        assert_eq!(tabs.tab_count(), 1);
    }

    #[test]
    fn test_agent_tabs_next() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        // Start at index 1
        tabs.select_tab(1);
        assert_eq!(tabs.active_index(), 1);

        // Next should wrap to 0
        tabs.next_tab();
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn test_agent_tabs_previous() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        // Start at index 0
        tabs.select_tab(0);
        assert_eq!(tabs.active_index(), 0);

        // Previous should wrap to 1
        tabs.previous_tab();
        assert_eq!(tabs.active_index(), 1);
    }

    #[test]
    fn test_agent_tabs_update_title() {
        let mut tabs = AgentTabs::new();

        let tab_id = tabs.add_tab(AgentTab::default()).id;

        let updated = tabs.update_tab_title(tab_id, "New Title");

        assert!(updated);
        assert_eq!(tabs.find_tab_by_id(&tab_id).unwrap().title, "New Title");
        assert!(!tabs.find_tab_by_id(&tab_id).unwrap().is_modified);
    }

    #[test]
    fn test_agent_tabs_update_title_invalid() {
        let mut tabs = AgentTabs::new();

        let updated = tabs.update_tab_title(Uuid::new_v4(), "New Title");

        assert!(!updated);
    }

    #[test]
    fn test_agent_tabs_update_modified() {
        let mut tabs = AgentTabs::new();

        let tab_id = tabs.add_tab(AgentTab::default()).id;

        let updated = tabs.update_tab_modified(tab_id, true);

        assert!(updated);
        assert!(tabs.find_tab_by_id(&tab_id).unwrap().is_modified);
    }

    #[test]
    fn test_agent_tabs_clear() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::default());
        tabs.add_tab(AgentTab::default());

        assert_eq!(tabs.tab_count(), 2);

        tabs.clear();

        assert!(tabs.is_empty());
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn test_agent_tab_with_title() {
        let tab = AgentTab::default().with_title("Updated");

        assert_eq!(tab.title, "Updated");
        assert!(!tab.is_modified);
    }

    #[test]
    fn test_agent_tab_with_modified() {
        let tab = AgentTab::default().with_modified(true);

        assert!(tab.is_modified);
    }

    #[test]
    fn test_agent_tabs_index_of() {
        let mut tabs = AgentTabs::new();

        let tab1_id = tabs.add_tab(AgentTab::default()).id;
        tabs.add_tab(AgentTab::default());

        let index = tabs.index_of(tab1_id);

        assert_eq!(index, Some(0));
    }

    #[test]
    fn test_agent_tabs_index_of_not_found() {
        let tabs = AgentTabs::new();

        let index = tabs.index_of(Uuid::new_v4());

        assert_eq!(index, None);
    }

    // Integration tests for tab creation and closing

    #[test]
    fn test_create_single_tab() {
        let mut tabs = AgentTabs::new();
        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));

        assert_eq!(tabs.tab_count(), 1);
        assert_eq!(tabs.active_index(), 0);

        let active_tab = tabs.active_tab().expect("Should have active tab");
        assert_eq!(active_tab.title, "Thread 1");
        assert!(active_tab.is_active);
        assert_eq!(active_tab.tab_type, TabType::Thread);
    }

    #[test]
    fn test_create_multiple_tabs() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        assert_eq!(tabs.tab_count(), 3);
        assert_eq!(tabs.active_index(), 2); // Last added tab should be active

        // Verify all tabs are present
        let tabs_list = tabs.tabs();
        assert_eq!(tabs_list[0].title, "Thread 1");
        assert_eq!(tabs_list[1].title, "Text Thread 1");
        assert_eq!(tabs_list[2].title, "History");

        // Only last tab should be active
        assert!(!tabs_list[0].is_active);
        assert!(!tabs_list[1].is_active);
        assert!(tabs_list[2].is_active);
    }

    #[test]
    fn test_close_middle_tab() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Select first tab
        tabs.select_tab(0);

        // Close middle tab (index 1)
        let closed = tabs.close_tab(1);
        assert!(closed.is_some());
        assert_eq!(closed.unwrap().title, "Text Thread 1");

        // Verify state after close
        assert_eq!(tabs.tab_count(), 2);
        assert_eq!(tabs.active_index(), 0); // Still on thread 1

        let tabs_list = tabs.tabs();
        assert_eq!(tabs_list[0].title, "Thread 1");
        assert_eq!(tabs_list[1].title, "History");
    }

    #[test]
    fn test_close_active_tab() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Select first tab
        tabs.select_tab(0);

        // Close active tab (index 0)
        let closed = tabs.close_tab(0);
        assert!(closed.is_some());
        assert_eq!(closed.unwrap().title, "Thread 1");

        // Active index should be adjusted
        assert_eq!(tabs.active_index(), 0); // Now pointing to what was index 1
        assert_eq!(tabs.active_tab().unwrap().title, "Text Thread 1");
        assert!(tabs.active_tab().unwrap().is_active);
    }

    #[test]
    fn test_close_last_tab_of_multiple() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Select middle tab
        tabs.select_tab(1);

        // Close last tab
        let closed = tabs.close_tab(2);
        assert!(closed.is_some());
        assert_eq!(closed.unwrap().title, "History");

        // Active tab should remain unchanged
        assert_eq!(tabs.tab_count(), 2);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active_tab().unwrap().title, "Text Thread 1");
    }

    #[test]
    fn test_close_last_tab_protected() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));

        // Try to close the only tab
        let closed = tabs.close_tab(0);
        assert!(closed.is_none());

        // Tab should still exist
        assert_eq!(tabs.tab_count(), 1);
        assert_eq!(tabs.active_index(), 0);
        assert!(tabs.active_tab().is_some());
        assert_eq!(tabs.active_tab().unwrap().title, "Thread 1");
    }

    #[test]
    fn test_tab_navigation_next() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Start at first tab
        tabs.select_tab(0);
        assert_eq!(tabs.active_index(), 0);

        // Move to next
        tabs.next_tab();
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active_tab().unwrap().title, "Text Thread 1");

        // Move to next
        tabs.next_tab();
        assert_eq!(tabs.active_index(), 2);
        assert_eq!(tabs.active_tab().unwrap().title, "History");

        // Should wrap to first tab
        tabs.next_tab();
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.active_tab().unwrap().title, "Thread 1");
    }

    #[test]
    fn test_tab_navigation_previous() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Start at last tab
        tabs.select_tab(2);
        assert_eq!(tabs.active_index(), 2);

        // Move to previous
        tabs.previous_tab();
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active_tab().unwrap().title, "Text Thread 1");

        // Move to previous
        tabs.previous_tab();
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.active_tab().unwrap().title, "Thread 1");

        // Should wrap to last tab
        tabs.previous_tab();
        assert_eq!(tabs.active_index(), 2);
        assert_eq!(tabs.active_tab().unwrap().title, "History");
    }

    #[test]
    fn test_close_tab_by_id() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        let tab2_id = tabs
            .add_tab(AgentTab::new("Text Thread 1", TabType::TextThread))
            .id;
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Close by ID
        let closed = tabs.close_tab_by_id(tab2_id);
        assert!(closed.is_some());
        assert_eq!(closed.unwrap().title, "Text Thread 1");

        assert_eq!(tabs.tab_count(), 2);

        // Try to close non-existent tab by ID
        let not_closed = tabs.close_tab_by_id(Uuid::new_v4());
        assert!(not_closed.is_none());
        assert_eq!(tabs.tab_count(), 2);
    }

    #[test]
    fn test_select_tab_by_id() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        let tab2_id = tabs
            .add_tab(AgentTab::new("Text Thread 1", TabType::TextThread))
            .id;
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Select by ID
        let result = tabs.select_tab_by_id(tab2_id);
        assert!(result.is_some());
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(result.unwrap().title, "Text Thread 1");

        // Try to select non-existent tab
        let not_selected = tabs.select_tab_by_id(Uuid::new_v4());
        assert!(not_selected.is_none());
    }

    #[test]
    fn test_tab_modified_state() {
        let mut tabs = AgentTabs::new();

        let tab1_id = tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread)).id;

        // Initially not modified
        let tab = tabs.find_tab_by_id(&tab1_id).unwrap();
        assert!(!tab.is_modified);

        // Mark as modified
        let updated = tabs.update_tab_modified(tab1_id, true);
        assert!(updated);
        assert!(tabs.find_tab_by_id(&tab1_id).unwrap().is_modified);

        // Mark as unmodified
        tabs.update_tab_modified(tab1_id, false);
        assert!(!tabs.find_tab_by_id(&tab1_id).unwrap().is_modified);
    }

    #[test]
    fn test_only_one_tab_remains_active() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        // Select first tab
        tabs.select_tab(0);

        let active_id = tabs.active_tab().unwrap().id;

        // Ensure only one tab is active
        let active_count = tabs.tabs().iter().filter(|t| t.is_active).count();
        assert_eq!(active_count, 1);

        // Switch to another tab
        tabs.select_tab(2);

        // Verify first tab is now inactive
        assert!(!tabs.find_tab_by_id(&active_id).unwrap().is_active);

        // Verify only the new tab is active
        let active_count = tabs.tabs().iter().filter(|t| t.is_active).count();
        assert_eq!(active_count, 1);
        assert!(tabs.active_tab().unwrap().is_active);
    }

    #[test]
    fn test_empty_tabs_navigation() {
        let mut tabs = AgentTabs::new();

        assert!(tabs.is_empty());
        assert_eq!(tabs.tab_count(), 0);
        assert!(tabs.active_tab().is_none());

        // Navigation should do nothing
        tabs.next_tab();
        tabs.previous_tab();
        assert!(tabs.is_empty());
        assert!(tabs.active_tab().is_none());
    }

    #[test]
    fn test_clear_tabs() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));

        assert_eq!(tabs.tab_count(), 3);

        tabs.clear();

        assert!(tabs.is_empty());
        assert_eq!(tabs.active_index(), 0);
        assert!(tabs.active_tab().is_none());
    }

    #[test]
    fn test_find_tab_by_type() {
        let mut tabs = AgentTabs::new();

        tabs.add_tab(AgentTab::new("Thread 1", TabType::Thread));
        tabs.add_tab(AgentTab::new("Text Thread 1", TabType::TextThread));
        tabs.add_tab(AgentTab::new("History", TabType::History));
        tabs.add_tab(AgentTab::new("Configuration", TabType::Configuration));

        assert_eq!(tabs.tab_count(), 4);

        let thread_tab = tabs.find_tab_by_type(TabType::Thread);
        assert!(thread_tab.is_some());
        assert_eq!(thread_tab.unwrap().title, "Thread 1");

        let history_tab = tabs.find_tab_by_type(TabType::History);
        assert!(history_tab.is_some());
        assert_eq!(history_tab.unwrap().title, "History");

        // Test non-existent type
        let mut tabs_empty = AgentTabs::new();
        let not_found = tabs_empty.find_tab_by_type(TabType::Configuration);
        assert!(not_found.is_none());
    }
}
