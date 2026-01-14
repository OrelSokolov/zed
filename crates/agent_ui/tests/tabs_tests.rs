//! Integration tests for AgentPanel tabs functionality
//!
//! Tests cover:
//! - Creating new tabs
//! - Selecting tabs
//! - Closing tabs
//! - Edge cases (last tab protection)

use agent_ui::{
    agent_panel::AgentPanel,
    tabs::{AgentTab, AgentTabs, TabType},
};
use gpui::{BackgroundExecutor, TestAppContext};
use std::sync::Arc;

// Helper to create a simple AgentTabs instance for testing
fn create_test_tabs(count: usize) -> AgentTabs {
    let mut tabs = AgentTabs::new();
    for i in 0..count {
        let title = format!("Tab {}", i + 1);
        let tab = AgentTab::new(title, TabType::Thread);
        tabs.add_tab(tab);
    }
    tabs
}

#[gpui::test]
async fn test_create_single_tab(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(1);

    assert_eq!(tabs.tab_count(), 1);
    assert_eq!(tabs.active_index(), 0);

    let active_tab = tabs.active_tab().expect("Should have active tab");
    assert_eq!(active_tab.title, "Tab 1");
    assert!(active_tab.is_active);
}

#[gpui::test]
async fn test_create_multiple_tabs(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    assert_eq!(tabs.tab_count(), 3);
    assert_eq!(tabs.active_index(), 2); // Last added tab should be active

    // Verify all tabs are present
    let tabs_list = tabs.tabs();
    assert_eq!(tabs_list[0].title, "Tab 1");
    assert_eq!(tabs_list[1].title, "Tab 2");
    assert_eq!(tabs_list[2].title, "Tab 3");

    // Only last tab should be active
    assert!(!tabs_list[0].is_active);
    assert!(!tabs_list[1].is_active);
    assert!(tabs_list[2].is_active);
}

#[gpui::test]
async fn test_select_tab_by_index(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Select first tab
    let result = tabs.select_tab(0);
    assert!(result.is_some());
    assert_eq!(tabs.active_index(), 0);
    assert!(tabs.active_tab().unwrap().is_active);
    assert_eq!(tabs.active_tab().unwrap().title, "Tab 1");

    // Select second tab
    tabs.select_tab(1);
    assert_eq!(tabs.active_index(), 1);
    assert_eq!(tabs.active_tab().unwrap().title, "Tab 2");

    // Select last tab
    tabs.select_tab(2);
    assert_eq!(tabs.active_index(), 2);
    assert_eq!(tabs.active_tab().unwrap().title, "Tab 3");
}

#[gpui::test]
async fn test_select_invalid_tab(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(2);

    // Try to select out-of-bounds index
    let result = tabs.select_tab(5);
    assert!(result.is_none());

    // Active tab should remain unchanged
    assert_eq!(tabs.active_index(), 1); // Last tab was active
    assert!(tabs.active_tab().is_some());
}

#[gpui::test]
async fn test_close_middle_tab(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Select first tab
    tabs.select_tab(0);

    // Close middle tab (index 1)
    let closed = tabs.close_tab(1);
    assert!(closed.is_some());
    assert_eq!(closed.unwrap().title, "Tab 2");

    // Verify state after close
    assert_eq!(tabs.tab_count(), 2);
    assert_eq!(tabs.active_index(), 0); // Still on tab 1

    let tabs_list = tabs.tabs();
    assert_eq!(tabs_list[0].title, "Tab 1");
    assert_eq!(tabs_list[1].title, "Tab 3");
}

#[gpui::test]
async fn test_close_active_tab(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Select first tab
    tabs.select_tab(0);

    // Close active tab (index 0)
    let closed = tabs.close_tab(0);
    assert!(closed.is_some());
    assert_eq!(closed.unwrap().title, "Tab 1");

    // Active index should be adjusted
    assert_eq!(tabs.active_index(), 0); // Now pointing to what was Tab 2
    assert_eq!(tabs.active_tab().unwrap().title, "Tab 2");
    assert!(tabs.active_tab().unwrap().is_active);
}

#[gpui::test]
async fn test_close_last_tab_protected(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(1);

    // Try to close the only tab
    let closed = tabs.close_tab(0);
    assert!(closed.is_none());

    // Tab should still exist
    assert_eq!(tabs.tab_count(), 1);
    assert_eq!(tabs.active_index(), 0);
    assert!(tabs.active_tab().is_some());
}

#[gpui::test]
async fn test_close_last_of_multiple_tabs(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Select middle tab
    tabs.select_tab(1);

    // Close last tab
    let closed = tabs.close_tab(2);
    assert!(closed.is_some());
    assert_eq!(closed.unwrap().title, "Tab 3");

    // Active tab should remain unchanged
    assert_eq!(tabs.tab_count(), 2);
    assert_eq!(tabs.active_index(), 1);
    assert_eq!(tabs.active_tab().unwrap().title, "Tab 2");
}

#[gpui::test]
async fn test_next_tab_wrapping(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Start at first tab
    tabs.select_tab(0);
    assert_eq!(tabs.active_index(), 0);

    // Move to next
    tabs.next_tab();
    assert_eq!(tabs.active_index(), 1);

    // Move to next
    tabs.next_tab();
    assert_eq!(tabs.active_index(), 2);

    // Should wrap to first tab
    tabs.next_tab();
    assert_eq!(tabs.active_index(), 0);
}

#[gpui::test]
async fn test_previous_tab_wrapping(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    // Start at last tab
    tabs.select_tab(2);
    assert_eq!(tabs.active_index(), 2);

    // Move to previous
    tabs.previous_tab();
    assert_eq!(tabs.active_index(), 1);

    // Move to previous
    tabs.previous_tab();
    assert_eq!(tabs.active_index(), 0);

    // Should wrap to last tab
    tabs.previous_tab();
    assert_eq!(tabs.active_index(), 2);
}

#[gpui::test]
async fn test_tab_types(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = AgentTabs::new();

    // Add different tab types
    tabs.add_tab(AgentTab::new("Thread", TabType::Thread));
    tabs.add_tab(AgentTab::new("Text Thread", TabType::TextThread));
    tabs.add_tab(AgentTab::new("History", TabType::History));
    tabs.add_tab(AgentTab::new("Config", TabType::Configuration));

    assert_eq!(tabs.tab_count(), 4);

    let tabs_list = tabs.tabs();
    assert_eq!(tabs_list[0].tab_type, TabType::Thread);
    assert_eq!(tabs_list[1].tab_type, TabType::TextThread);
    assert_eq!(tabs_list[2].tab_type, TabType::History);
    assert_eq!(tabs_list[3].tab_type, TabType::Configuration);
}

#[gpui::test]
async fn test_find_tab_by_id(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = AgentTabs::new();

    let tab1_id = tabs.add_tab(AgentTab::new("Tab 1", TabType::Thread)).id;
    let tab2_id = tabs.add_tab(AgentTab::new("Tab 2", TabType::TextThread)).id;

    // Find existing tab
    let found = tabs.find_tab_by_id(&tab1_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Tab 1");

    // Find non-existent tab
    let not_found = tabs.find_tab_by_id(&uuid::Uuid::new_v4());
    assert!(not_found.is_none());
}

#[gpui::test]
async fn test_close_tab_by_id(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = AgentTabs::new();

    tabs.add_tab(AgentTab::new("Tab 1", TabType::Thread));
    let tab2_id = tabs.add_tab(AgentTab::new("Tab 2", TabType::TextThread)).id;
    tabs.add_tab(AgentTab::new("Tab 3", TabType::History));

    // Close by ID
    let closed = tabs.close_tab_by_id(tab2_id);
    assert!(closed.is_some());
    assert_eq!(closed.unwrap().title, "Tab 2");

    assert_eq!(tabs.tab_count(), 2);

    // Try to close non-existent tab by ID
    let not_closed = tabs.close_tab_by_id(uuid::Uuid::new_v4());
    assert!(not_closed.is_none());
    assert_eq!(tabs.tab_count(), 2);
}

#[gpui::test]
async fn test_select_tab_by_id(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    let tab2_id = tabs.tabs()[1].id;

    // Select by ID
    let result = tabs.select_tab_by_id(tab2_id);
    assert!(result.is_some());
    assert_eq!(tabs.active_index(), 1);
    assert_eq!(result.unwrap().title, "Tab 2");

    // Try to select non-existent tab
    let not_selected = tabs.select_tab_by_id(uuid::Uuid::new_v4());
    assert!(not_selected.is_none());
}

#[gpui::test]
async fn test_update_tab_title(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = AgentTabs::new();

    let tab1_id = tabs
        .add_tab(AgentTab::new("Original Title", TabType::Thread))
        .id;

    // Update title
    let updated = tabs.update_tab_title(tab1_id, "New Title");
    assert!(updated);

    let tab = tabs.find_tab_by_id(&tab1_id).unwrap();
    assert_eq!(tab.title, "New Title");

    // Try to update non-existent tab
    let not_updated = tabs.update_tab_title(uuid::Uuid::new_v4(), "Another Title");
    assert!(!not_updated);
}

#[gpui::test]
async fn test_tab_modified_state(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = AgentTabs::new();

    let tab1_id = tabs.add_tab(AgentTab::new("Tab 1", TabType::Thread)).id;

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

#[gpui::test]
async fn test_empty_tabs_manager(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let tabs = AgentTabs::new();

    assert!(tabs.is_empty());
    assert_eq!(tabs.tab_count(), 0);
    assert!(tabs.active_tab().is_none());

    // Navigation should do nothing
    tabs.next_tab();
    tabs.previous_tab();
    assert!(tabs.is_empty());
}

#[gpui::test]
async fn test_clear_tabs(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    assert_eq!(tabs.tab_count(), 3);

    tabs.clear();

    assert!(tabs.is_empty());
    assert_eq!(tabs.active_index(), 0);
}

#[gpui::test]
async fn test_tab_index_of(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

    let tab1_id = tabs.tabs()[0].id;
    let tab2_id = tabs.tabs()[1].id;
    let tab3_id = tabs.tabs()[2].id;

    assert_eq!(tabs.index_of(tab1_id), Some(0));
    assert_eq!(tabs.index_of(tab2_id), Some(1));
    assert_eq!(tabs.index_of(tab3_id), Some(2));

    // Non-existent tab
    assert_eq!(tabs.index_of(uuid::Uuid::new_v4()), None);
}

#[gpui::test]
async fn test_only_one_tab_remains_active(_executor: BackgroundExecutor, cx: &mut TestAppContext) {
    let mut tabs = create_test_tabs(3);

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
