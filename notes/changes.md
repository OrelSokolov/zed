# Changes Summary

## Dependencies
- Added `glob` dependency to workspace and agent crate
- Added `lmstudio` dependency to workspace and language_models
- Added `log` dependency to lmstudio crate

## Agent System Prompt
- Added instruction to respond in the same language as the user's question
- Updated path handling instructions: now requires absolute paths (starting with `/`) instead of relative paths based on project root names
- Improved worktree path display format to show both root name and absolute path

## Agent Tools
- Enhanced `create_directory_tool` with improved path resolution logic for absolute paths
- Refactored `find_path_tool` to better handle absolute path resolution

## LMStudio Integration
- Added `reasoning` field to LMStudio settings with support for Low, Medium, and High levels
- Integrated reasoning parameter into chat completion requests when thinking is allowed
- Added filtering of special tokens (format `<|token_name|>`) from model output to prevent metadata tokens from appearing in user-facing text

## Agent UI
- Added reasoning selector UI component in thread view for LMStudio models
- Enhanced text thread editor functionality

## Settings
- Added reasoning configuration support in language model settings
