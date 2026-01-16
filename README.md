# Zed (Fork by oleg)

This is a fork of [Zed](https://github.com/zed-industries/zed), a high-performance, multiplayer code editor from the creators of [Atom](https://github.com/atom/atom) and [Tree-sitter](https://github.com/tree-sitter/tree-sitter).

## Changes in This Fork

This fork includes several enhancements and bug fixes:

### 🤖 Z-AI (Zhipu AI) Language Model Provider
Added a new language model provider for Zhipu AI, enabling support for additional AI-powered features.

Features:
- Full integration with Zhipu AI's language models
- Settings UI for configuring Z-AI provider
- Support for model selection and API key configuration

Files:
- `crates/language_models/src/provider/z_ai.rs` (new)
- `crates/language_models/src/settings.rs`
- `crates/settings_content/src/language_model.rs`

### 📑 Multi Agent Tabs
Enhanced agent panel to support multiple agent tabs, allowing users to work with different AI agents simultaneously.

Features:
- Tab-based interface for managing multiple agents
- Improved agent panel UX

Files:
- `crates/agent_ui/src/agent_panel.rs`
- `crates/settings_content/src/agent.rs`

### 🐧 Deb Package Builder
Added script for building Debian packages, making it easier to distribute Zed on Linux.

Features:
- Automated deb package creation
- Standard Linux packaging format

Files:
- `script/build-deb` (new)
- `crates/zed/Cargo.toml`

### 🐛 Bug Fixes
- Fixed possible wax crash (path matching library)
- Fixed compilation errors after merging main branch

## Installation

The original Zed installation instructions apply. For Linux, you can now also use the deb package builder script.

### Building Deb Package

```bash
./script/build-deb
```

## Developing Zed

- [Building Zed for macOS](./docs/src/development/macos.md)
- [Building Zed for Linux](./docs/src/development/linux.md)
- [Building Zed for Windows](./docs/src/development/windows.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for ways you can contribute to Zed.

## Licensing

Same as upstream Zed project. License information for third party dependencies must be correctly provided for CI to pass.

## Sponsorship

Zed is developed by **Zed Industries, Inc.**, a for-profit company.

If you'd like to financially support the original project, you can do so via GitHub Sponsors at https://zed.dev.
