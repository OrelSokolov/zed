use agent_client_protocol::ToolKind;
use anyhow::{Context as _, Result, anyhow};
use futures::FutureExt as _;
use gpui::{App, Entity, SharedString, Task};
use project::{Project, ProjectPath};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use util::markdown::MarkdownInlineCode;
use util::rel_path::RelPath;

use crate::{AgentTool, ToolCallEventStream};

/// Creates a new directory at the specified path within the project. Returns confirmation that the directory was created.
///
/// This tool creates a directory and all necessary parent directories. It should be used whenever you need to create new directories within the project.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateDirectoryToolInput {
    /// The path of the new directory.
    ///
    /// <example>
    /// If the project has the following structure:
    ///
    /// - directory1/
    /// - directory2/
    ///
    /// You can create a new directory by providing a path of "directory1/new_directory"
    /// </example>
    pub path: String,
}

pub struct CreateDirectoryTool {
    project: Entity<Project>,
}

impl CreateDirectoryTool {
    pub fn new(project: Entity<Project>) -> Self {
        Self { project }
    }
}

impl AgentTool for CreateDirectoryTool {
    type Input = CreateDirectoryToolInput;
    type Output = String;

    fn name() -> &'static str {
        "create_directory"
    }

    fn kind() -> ToolKind {
        ToolKind::Read
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        if let Ok(input) = input {
            format!("Create directory {}", MarkdownInlineCode(&input.path)).into()
        } else {
            "Create directory".into()
        }
    }

    fn run(
        self: Arc<Self>,
        input: Self::Input,
        event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output>> {
        let project = self.project.read(cx);
        
        // First, try to find the path directly
        let project_path = if let Some(path) = project.find_project_path(&input.path, cx) {
            // Path found - check if it already exists
            if let Some(entry) = project.entry_for_path(&path, cx) {
                if entry.is_dir() {
                    return Task::ready(Ok(format!("Directory {} already exists", input.path)));
                } else {
                    return Task::ready(Err(anyhow!(
                        "Cannot create directory: {} already exists as a file",
                        input.path
                    )));
                }
            }
            Some(path)
        } else {
            // Path not found - try to resolve it as a relative path in worktree
            let path = Path::new(&input.path);
            let path_style = project.path_style(cx);
            let worktrees: Vec<_> = project.worktrees(cx).collect();
            
            // Try to resolve as relative path
            if let Ok(rel_path) = RelPath::new(path, path_style) {
                // Check if path starts with a worktree root name
                let mut resolved_path = None;
                for worktree in &worktrees {
                    let worktree_root_name = worktree.read(cx).root_name();
                    if let Ok(relative_path) = path.strip_prefix(worktree_root_name.as_std_path()) {
                        if let Ok(rel_path) = RelPath::new(relative_path, path_style) {
                            resolved_path = Some(ProjectPath {
                                worktree_id: worktree.read(cx).id(),
                                path: rel_path.into_arc(),
                            });
                            break;
                        }
                    }
                }
                
                if let Some(path) = resolved_path {
                    Some(path)
                } else if worktrees.len() == 1 {
                    // Single worktree - assume path is relative to worktree root
                    let worktree = &worktrees[0];
                    Some(ProjectPath {
                        worktree_id: worktree.read(cx).id(),
                        path: rel_path.into_arc(),
                    })
                } else {
                    // Multiple worktrees - try to find parent directory
                    let parent_path = path.parent();
                    let mut resolved = None;
                    if let Some(parent) = parent_path {
                        // Try to find parent directory in any worktree
                        if let Some(parent_project_path) = project.find_project_path(parent, cx) {
                            // Get the directory name
                            if let Some(dir_name_str) = path.file_name().and_then(|n| n.to_str()) {
                                if let Ok(dir_name) = RelPath::unix(dir_name_str) {
                                    resolved = Some(ProjectPath {
                                        path: parent_project_path.path.join(dir_name),
                                        worktree_id: parent_project_path.worktree_id,
                                    });
                                }
                            }
                        }
                    }
                    resolved
                }
            } else {
                None
            }
        };
        
        let project_path = match project_path {
            Some(path) => path,
            None => {
                return Task::ready(Err(anyhow!(
                    "Path to create was outside the project: {}",
                    input.path
                )));
            }
        };
        
        let destination_path: Arc<str> = input.path.as_str().into();
        let project_path_clone = project_path.clone();
        let project_weak = self.project.downgrade();

        // Collect parent directories that need to be created
        let parents_to_create = self.project.update(cx, |project, cx| {
            let mut current_path = project_path_clone.path.as_ref();
            let mut parents = Vec::new();
            
            // Collect all missing parent directories
            while let Some(parent) = current_path.parent() {
                let parent_project_path = ProjectPath {
                    path: Arc::from(parent),
                    worktree_id: project_path_clone.worktree_id,
                };
                
                if project.entry_for_path(&parent_project_path, cx).is_none() {
                    parents.push(parent_project_path);
                    current_path = parent;
                } else {
                    break;
                }
            }
            
            // Reverse to create from root to leaf
            parents.reverse();
            parents
        });

        let create_entry = self.project.update(cx, |project, cx| {
            project.create_entry(project_path.clone(), true, cx)
        });

        cx.spawn(async move |cx| {
            // First create parent directories if needed
            if !parents_to_create.is_empty() {
                for parent_path in parents_to_create {
                    let create_parent = project_weak.update(cx, |project, cx| {
                        project.create_entry(parent_path, true, cx)
                    })?;
                    create_parent.await?;
                }
            }
            
            // Then create the target directory
            futures::select! {
                result = create_entry.fuse() => {
                    result.with_context(|| format!("Creating directory {destination_path}"))?;
                }
                _ = event_stream.cancelled_by_user().fuse() => {
                    anyhow::bail!("Create directory cancelled by user");
                }
            }

            Ok(format!("Created directory {destination_path}"))
        })
    }
}
