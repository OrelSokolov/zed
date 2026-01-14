# Tabs Testing Guide

This document provides a comprehensive guide for testing the tab management system in the AgentPanel.

## Overview

The tabs system (`src/tabs/mod.rs`) provides a robust tab management infrastructure that allows users to work with multiple agent conversations simultaneously. The testing suite ensures this system works correctly across various scenarios.

## Test Organization

Tests are organized into two main categories:

### 1. Unit Tests

Located in `src/tabs/mod.rs` under the `#[cfg(test)] mod tests` module, these tests focus on:

- Individual method behavior
- Edge cases and error conditions
- Basic state management

Examples:
- `test_agent_tab_creation`: Verifies default tab properties
- `test_agent_tabs_empty`: Tests empty tabs manager state
- `test_agent_tabs_add`: Tests adding a single tab

### 2. Integration Tests

Also in `src/tabs/mod.rs`, these tests cover:

- Complex multi-tab scenarios
- Tab lifecycle management
- Navigation behavior
- State synchronization

Examples:
- `test_create_multiple_tabs`: Tests creating multiple tabs with active state management
- `test_close_middle_tab`: Verifies closing a tab while others exist
- `test_tab_navigation_next`: Tests forward navigation with wrapping

## Running Tests

### Run All Tab Tests

```bash
cargo test -p agent_ui --lib tabs::tests
```

### Run a Specific Test

```bash
cargo test -p agent_ui --lib tabs::tests::test_create_single_tab
```

### Run Tests with Output

```bash
cargo test -p agent_ui --lib tabs::tests -- --nocapture
```

## Test Coverage

### Tab Creation Tests

| Test Name | Description |
|-----------|-------------|
| `test_create_single_tab` | Creates a single tab and verifies its properties |
| `test_create_multiple_tabs` | Creates multiple tabs and verifies only the last is active |
| `test_agent_tabs_add` | Tests adding a tab to an empty manager |
| `test_agent_tabs_multiple` | Tests adding multiple tabs in sequence |

### Tab Selection Tests

| Test Name | Description |
|-----------|-------------|
| `test_select_tab_by_index` | Selects tabs by their index |
| `test_select_invalid_tab` | Tests selecting an out-of-bounds index |
| `test_select_tab_by_id` | Selects tabs by their unique ID |

### Tab Closing Tests

| Test Name | Description |
|-----------|-------------|
| `test_close_middle_tab` | Closes a tab in the middle of the list |
| `test_close_active_tab` | Closes the currently active tab |
| `test_close_last_tab_of_multiple` | Closes the last tab when multiple exist |
| `test_close_last_tab_protected` | Verifies the last tab cannot be closed |
| `test_close_tab_by_id` | Closes a tab by its unique ID |

### Navigation Tests

| Test Name | Description |
|-----------|-------------|
| `test_tab_navigation_next` | Tests forward navigation with wrapping |
| `test_tab_navigation_previous` | Tests backward navigation with wrapping |
| `test_agent_tabs_next` | Tests basic next tab functionality |
| `test_agent_tabs_previous` | Tests basic previous tab functionality |

### State Management Tests

| Test Name | Description |
|-----------|-------------|
| `test_tab_modified_state` | Tests updating the modified flag on tabs |
| `test_only_one_tab_remains_active` | Ensures only one tab is active at a time |
| `test_empty_tabs_navigation` | Tests navigation behavior on empty tabs |
| `test_clear_tabs` | Tests clearing all tabs |
| `test_agent_tabs_update_title` | Tests updating tab titles |
| `test_agent_tabs_update_modified` | Tests updating the modified state |

### Lookup Tests

| Test Name | Description |
|-----------|-------------|
| `test_find_tab_by_id` | Tests finding tabs by their unique ID |
| `test_find_tab_by_type` | Tests finding tabs by their type |
| `test_agent_tabs_index_of` | Tests getting the index of a tab by ID |
| `test_agent_tabs_index_of_not_found` | Tests index lookup for non-existent tab |

### Tab Type Tests

| Test Name | Description |
|-----------|-------------|
| `test_tab_types` | Verifies different tab types (Thread, TextThread, History, Configuration) |

## Test Patterns

### Creating Test Tabs

```rust
fn create_test_tabs(count: usize) -> AgentTabs {
    let mut tabs = AgentTabs::new();
    for i in 0..count {
        let title = format!("Tab {}", i + 1);
        let tab = AgentTab::new(title, TabType::Thread);
        tabs.add_tab(tab);
    }
    tabs
}
```

### Verifying Tab State

```rust
// Check tab count
assert_eq!(tabs.tab_count(), 3);

// Check active index
assert_eq!(tabs.active_index(), 2);

// Verify active tab properties
let active_tab = tabs.active_tab().expect("Should have active tab");
assert!(active_tab.is_active);
assert_eq!(active_tab.title, "Expected Title");
```

### Testing Tab Operations

```rust
// Add a tab and capture its ID
let tab_id = tabs.add_tab(AgentTab::new("New Tab", TabType::Thread)).id;

// Close a tab
let closed = tabs.close_tab(index);
assert!(closed.is_some());

// Select a tab
let selected = tabs.select_tab(index);
assert!(selected.is_some());
```

## Best Practices

### 1. Test Edge Cases

Always test boundary conditions:
- Empty tabs manager
- Single tab
- First/last tab operations
- Invalid indices/IDs

### 2. Verify State After Operations

After each operation, verify:
- Tab count is correct
- Active index is updated
- Active tab properties are correct
- Only one tab is active

### 3. Test Type Safety

When testing with different tab types:
```rust
tabs.add_tab(AgentTab::new("Thread", TabType::Thread));
tabs.add_tab(AgentTab::new("Text Thread", TabType::TextThread));
tabs.add_tab(AgentTab::new("History", TabType::History));
```

### 4. Use Descriptive Test Names

Follow the pattern: `test_<feature>_<scenario>`
- `test_close_active_tab`
- `test_create_multiple_tabs`
- `test_select_tab_by_id`

### 5. Test Return Values

Verify that methods return appropriate values:
```rust
// Success case
let closed = tabs.close_tab(0);
assert!(closed.is_some());

// Failure case
let not_closed = tabs.close_tab(100);
assert!(not_closed.is_none());
```

## Common Assertions

```rust
// Tab existence
assert_eq!(tabs.tab_count(), expected_count);
assert!(!tabs.is_empty());

// Active state
assert_eq!(tabs.active_index(), expected_index);
assert!(tabs.active_tab().is_some());
assert!(tabs.active_tab().unwrap().is_active);

// Tab properties
assert_eq!(tab.title, expected_title);
assert_eq!(tab.tab_type, expected_type);
assert!(!tab.is_modified);

// Operation results
assert!(result.is_some());
assert!(updated);
assert_eq!(index, Some(expected_index));
```

## Adding New Tests

When adding a new test:

1. Determine if it's a unit or integration test
2. Choose an appropriate test name following the naming convention
3. Set up the initial state (empty tabs, pre-populated tabs, etc.)
4. Perform the operation being tested
5. Verify all relevant aspects of the state
6. Include edge cases and error conditions if applicable

### Example: Adding a Test for a New Feature

```rust
#[test]
fn test_new_feature_scenario() {
    // Setup
    let mut tabs = AgentTabs::new();
    tabs.add_tab(AgentTab::new("Tab 1", TabType::Thread));
    tabs.add_tab(AgentTab::new("Tab 2", TabType::TextThread));
    
    // Perform operation
    let result = tabs.new_method();
    
    // Verify result
    assert!(result.is_some());
    
    // Verify state
    assert_eq!(tabs.tab_count(), 2);
    assert_eq!(tabs.active_index(), expected_index);
}
```

## Troubleshooting

### Tests Not Running

If tests aren't being discovered:
- Ensure tests are in the `#[cfg(test)] mod tests` module
- Check that the test function is marked with `#[test]`
- Verify the module is properly nested

### Test Failures

Common failure reasons:
- Incorrect expected values
- Race conditions (shouldn't occur with current design)
- State not properly reset between tests
- Incorrect assumptions about default behavior

### Compilation Errors

Common compilation issues:
- Missing imports (ensure `use super::*;` is in the test module)
- Wrong method signatures
- Incorrect struct field access

## Related Documentation

- [Tabs Module API Documentation](../src/tabs/mod.rs)
- [AgentPanel Documentation](../src/agent_panel.rs)
- [GPUI Testing Guidelines](../../gpui/tests/)

## Future Testing Improvements

Potential areas for expanded testing:

- **Performance Tests**: Test behavior with large numbers of tabs
- **Concurrent Access**: Test thread-safe operations (if applicable)
- **UI Integration**: Test integration with AgentPanel rendering
- **Persistence Tests**: Test tab state serialization/deserialization
- **Event Handling**: Test tab-related event emissions

## Contact

For questions about the tabs testing system or to report issues, please refer to the project's contribution guidelines.