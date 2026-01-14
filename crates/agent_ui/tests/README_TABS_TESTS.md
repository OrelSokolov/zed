# Tabs Tests README

This directory contains documentation and resources for testing the tab management system in AgentPanel.

## Quick Start

### Run All Tab Tests

```bash
cargo test -p agent_ui --lib tabs::tests
```

### Run a Specific Test

```bash
cargo test -p agent_ui --lib tabs::tests::test_create_single_tab
```

## Test Suite Overview

The tabs testing suite is located in `src/tabs/mod.rs` and includes:

- **28 unit and integration tests** covering all tab functionality
- Tests for creation, selection, closing, and navigation
- Edge case testing (empty tabs, last tab protection, etc.)
- State management verification (active states, modified flags, titles)

## Test Categories

### 1. Creation Tests
- Single tab creation
- Multiple tab creation
- Active state assignment

### 2. Selection Tests
- Select by index
- Select by ID
- Invalid selection handling

### 3. Closing Tests
- Close middle tab
- Close active tab
- Close last tab (protected)
- Close by ID

### 4. Navigation Tests
- Next tab with wrapping
- Previous tab with wrapping

### 5. State Management Tests
- Modified state updates
- Title updates
- Active state consistency
- Tab clearing

## Test Coverage

| Feature | Coverage |
|---------|----------|
| Tab Creation | ✅ Full |
| Tab Selection | ✅ Full |
| Tab Closing | ✅ Full |
| Navigation | ✅ Full |
| State Management | ✅ Full |
| Edge Cases | ✅ Full |

## Key Test Files

- `src/tabs/mod.rs` - Implementation with embedded tests
- `tests/tabs_testing_guide.md` - Comprehensive testing guide
- `tests/tabs_tests.rs` - Standalone integration tests (work in progress)

## Common Commands

```bash
# Run with output
cargo test -p agent_ui --lib tabs::tests -- --nocapture

# Run specific test category
cargo test -p agent_ui --lib tabs::tests::test_create

# Run failed tests only
cargo test -p agent_ui --lib tabs::tests -- --failed
```

## Writing New Tests

1. Add test to `src/tabs/mod.rs` in the `#[cfg(test)] mod tests` module
2. Follow naming convention: `test_<feature>_<scenario>`
3. Include setup, operation, and verification phases
4. Test both success and failure cases

Example:

```rust
#[test]
fn test_my_new_feature() {
    let mut tabs = AgentTabs::new();
    
    // Setup
    tabs.add_tab(AgentTab::new("Tab 1", TabType::Thread));
    
    // Perform operation
    let result = tabs.my_new_operation();
    
    // Verify
    assert!(result.is_some());
    assert_eq!(tabs.tab_count(), 1);
}
```

## Documentation

- **[Tabs Testing Guide](tabs_testing_guide.md)** - Comprehensive guide with examples and best practices
- **[Tabs Module](../src/tabs/mod.rs)** - Implementation with inline documentation

## Current Status

- ✅ All basic tab operations tested
- ✅ Edge cases covered
- ✅ State management verified
- 🚧 UI integration tests (planned)
- 🚧 Persistence tests (planned)

## Notes

- Tests are currently unit/integration level
- Full GPUI integration tests to be added
- All tests are synchronous (no async operations tested yet)