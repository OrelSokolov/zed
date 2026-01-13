//! Утилита для тестирования FindPathTool на реальной директории
//! 
//! Запуск: 
//!   TEST_DIR=/home/oleg/quic-go TEST_PATTERN="*cucumber*" cargo test --package agent --lib tools::test_find_path::test_real_directory -- --nocapture

#[cfg(test)]
mod test {
    use crate::tools::find_path_tool;
    use gpui::TestAppContext;
    use project::Project;
    use settings::SettingsStore;
    use std::path::Path;
    use std::sync::Arc;
    use fs::RealFs;

    #[gpui::test]
    async fn test_real_directory(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
        });
        
        // Замените этот путь на нужную директорию
        let test_dir = std::env::var("TEST_DIR").unwrap_or_else(|_| "/home/oleg/quic-go".to_string());
        let test_pattern = std::env::var("TEST_PATTERN").unwrap_or_else(|_| "*cucumber*".to_string());
        
        println!("Testing FindPathTool on directory: {}", test_dir);
        println!("Search pattern: {}", test_pattern);
        
        let fs = Arc::new(RealFs::new(None, cx.executor().clone()));
        let project = Project::test(fs, [Path::new(&test_dir)], cx).await;
        
        println!("Running search immediately (scan may still be in progress)...");
        
        println!("\nRunning search_paths with pattern: {}", test_pattern);
        let matches = cx
            .update(|cx| find_path_tool::search_paths(&test_pattern, project.clone(), cx))
            .await;
        
        match matches {
            Ok(paths) => {
                println!("\nFound {} matches:", paths.len());
                for (i, path) in paths.iter().enumerate() {
                    println!("  {}. {}", i + 1, path.display());
                }
                
                // Также проверим через snapshot напрямую
                println!("\nChecking snapshot entries directly...");
                let snapshot_info = cx.update(|cx| {
                    let project = project.read(cx);
                    let worktrees: Vec<_> = project.worktrees(cx).collect();
                    let mut total_entries = 0;
                    let mut ignored_entries = 0;
                    let mut external_entries = 0;
                    let mut hidden_entries = 0;
                    let mut unloaded_dirs = 0;
                    
                    for worktree in worktrees {
                        let snapshot = worktree.read(cx).snapshot();
                        for entry in snapshot.entries(false, 0) {
                            total_entries += 1;
                            if entry.is_ignored {
                                ignored_entries += 1;
                            }
                            if entry.is_external {
                                external_entries += 1;
                            }
                            if entry.is_hidden {
                                hidden_entries += 1;
                            }
                            if entry.kind.is_unloaded() {
                                unloaded_dirs += 1;
                            }
                        }
                    }
                    
                    (total_entries, ignored_entries, external_entries, hidden_entries, unloaded_dirs)
                });
                
                println!("Snapshot statistics:");
                println!("  Total entries (include_ignored=false): {}", snapshot_info.0);
                println!("  Ignored entries: {}", snapshot_info.1);
                println!("  External entries: {}", snapshot_info.2);
                println!("  Hidden entries: {}", snapshot_info.3);
                println!("  Unloaded directories: {}", snapshot_info.4);
                
                // Проверим с include_ignored=true
                let snapshot_info_all = cx.update(|cx| {
                    let project = project.read(cx);
                    let worktrees: Vec<_> = project.worktrees(cx).collect();
                    let mut total_entries = 0;
                    
                    for worktree in worktrees {
                        let snapshot = worktree.read(cx).snapshot();
                        for _entry in snapshot.entries(true, 0) {
                            total_entries += 1;
                        }
                    }
                    
                    total_entries
                });
                
                println!("  Total entries (include_ignored=true): {}", snapshot_info_all);
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
            }
        }
    }
}

