// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use std::sync::Arc;

use cosmic::prelude::*;

use crate::scanner;

use super::tasks;
use super::{AppModel, Message};

pub(super) fn update(app: &mut AppModel, message: Message) -> Task<cosmic::Action<Message>> {
    match message {
        Message::TopLevelListed(entry) => {
            app.listing = false;

            let branch_dirs: Vec<PathBuf> = entry
                .children
                .iter()
                .filter(|c| c.is_dir)
                .map(|c| c.path.clone())
                .collect();

            app.pending_branches = branch_dirs.len();
            app.total_branches = branch_dirs.len();
            app.root = Some(entry);
            scanner::index_subtree(app.root.as_ref().unwrap(), Arc::make_mut(&mut app.search_index));

            if branch_dirs.is_empty() {
                // No subdirectories to scan in the background, so no
                // BranchScanned will ever arrive to mark the scan complete.
                app.last_scan_time = Some(scanner::now_unix());
            }

            let generation = app.generation;
            return Task::batch(
                branch_dirs
                    .into_iter()
                    .map(|path| tasks::spawn_branch_scan(path, generation)),
            );
        }
        Message::BranchScanned(path, scanned) => {
            if let Some(entry) = scanned {
                scanner::index_subtree(&entry, Arc::make_mut(&mut app.search_index));
                if let Some(root) = &mut app.root {
                    if let Some(child) = root.children.iter_mut().find(|c| c.path == path) {
                        *child = entry;
                    }
                    root.children.sort_by(|a, b| b.size.cmp(&a.size));
                    root.size = root.children.iter().map(|c| c.size).sum();
                }
            }
            app.pending_branches = app.pending_branches.saturating_sub(1);
            if app.pending_branches == 0 {
                scanner::clear_current_scan_path();
                app.last_scan_time = Some(scanner::now_unix());
            }
        }
        Message::ToggleExpand(path) => {
            if !app.expanded.remove(&path) {
                app.expanded.insert(path);
            }
        }
        Message::Select(path) => {
            app.selected = Some(path);
        }
        Message::SavedIndexChecked(saved) => {
            app.loading_saved_index = false;
            let Some(saved) = saved else {
                return tasks::spawn_top_level_listing(app.scan_root.clone());
            };
            app.listing = false;
            app.last_scan_time = Some(saved.scanned_at);
            scanner::index_subtree(&saved.root, Arc::make_mut(&mut app.search_index));
            app.root = Some(saved.root);
        }
        Message::DiskSpaceChecked(space) => {
            app.disk_space = space;
        }
        Message::SaveIndex => {
            if let Some(root) = &app.root {
                app.saving_index = true;
                return tasks::spawn_save_index(root.clone(), app.scan_root.clone());
            }
        }
        Message::SaveIndexCompleted(Ok(())) => {
            app.saving_index = false;
            app.last_error = None;
        }
        Message::SaveIndexCompleted(Err(err)) => {
            app.saving_index = false;
            app.last_error = Some(format!("couldn't save index: {err}"));
        }
        Message::Rescan => {
            return app.force_rescan(app.scan_root.clone());
        }
        Message::CancelScan => {
            app.cancel_scan();
        }
        Message::HideDotfilesToggled(value) => {
            app.hide_dotfiles = value;
            if let Some(handle) = &app.config_handle {
                use cosmic::cosmic_config::CosmicConfigEntry;
                let config = crate::config::Config { hide_dotfiles: value };
                if let Err(err) = config.write_entry(handle) {
                    eprintln!("failed to save config: {err}");
                }
            }
        }
        Message::SearchChanged(value) => {
            app.search_query = value;
            app.search_generation += 1;
            if app.search_query.is_empty() {
                app.search_results = None;
                app.searching = false;
            } else {
                app.searching = true;
                return tasks::spawn_search_debounce(app.search_generation);
            }
        }
        Message::SearchRegexToggled(value) => {
            app.search_regex = value;
            return app.rerun_search();
        }
        Message::SearchCaseSensitiveToggled(value) => {
            app.search_case_sensitive = value;
            return app.rerun_search();
        }
        Message::SearchWholeWordToggled(value) => {
            app.search_whole_word = value;
            return app.rerun_search();
        }
        Message::SearchDebounced(generation) => {
            if generation == app.search_generation {
                return tasks::spawn_search(
                    app.search_index.clone(),
                    app.search_query.clone(),
                    app.search_options(),
                    generation,
                );
            }
        }
        Message::SearchResultsReady(generation, results) => {
            if generation == app.search_generation {
                app.search_results = Some(results);
                app.searching = false;
            }
        }
        Message::RootInputChanged(value) => {
            app.root_input = value;
        }
        Message::RootSubmitted => {
            let candidate = PathBuf::from(app.root_input.trim());
            if candidate.is_dir() {
                return app.begin_scan(candidate);
            }
            app.last_error = Some(format!("not a directory: {}", candidate.display()));
        }
        Message::QuickRootSelected(index) => {
            if let Some((_, path)) = app.quick_roots.get(index).cloned() {
                return app.begin_scan(path);
            }
        }
        Message::EditTitle => {
            app.title_editing = true;
            app.root_input = app.scan_root.to_string_lossy().into_owned();
            return tasks::spawn_folder_picker(app.scan_root.clone());
        }
        Message::TitleEditCancelled => {
            app.title_editing = false;
        }
        Message::FolderPicked(path) => {
            return app.begin_scan(path);
        }
        Message::FolderPickCancelled => {
            // Leave the title editable — the user may still want to type a
            // path by hand instead of using the dialog.
        }
        Message::FolderPickError(err) => {
            app.last_error = Some(err);
        }
        Message::DeleteRequested => {
            if let Some(selected) = &app.selected {
                if *selected != app.scan_root {
                    app.confirm_delete = Some(selected.clone());
                }
            }
        }
        Message::DeleteContextRequested(path) => {
            if path != app.scan_root {
                app.selected = Some(path.clone());
                app.confirm_delete = Some(path);
            }
        }
        Message::DeleteCancelled => {
            app.confirm_delete = None;
        }
        Message::DeleteConfirmed => {
            if let Some(path) = app.confirm_delete.take() {
                let is_dir = app
                    .root
                    .as_ref()
                    .and_then(|root| scanner::find_entry(root, &path))
                    .is_some_and(|entry| entry.is_dir);
                return tasks::spawn_delete(path, is_dir);
            }
        }
        Message::DeleteCompleted(path, Ok(())) => {
            if let Some(root) = &mut app.root {
                scanner::remove_path(root, &path);
            }
            scanner::remove_from_index(Arc::make_mut(&mut app.search_index), &path);
            if app.selected.as_deref() == Some(path.as_path()) {
                app.selected = None;
            }
            app.expanded.remove(&path);
            app.last_error = None;
            return tasks::spawn_disk_space(app.scan_root.clone());
        }
        Message::DeleteCompleted(path, Err(err)) => {
            app.last_error = Some(format!("couldn't delete {}: {err}", path.display()));
        }
        Message::OpenInFiles(path) => {
            tasks::open_in_files(&path);
        }
        Message::RenameRequested(path) => {
            if path != app.scan_root {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                app.rename_target = Some((path, name));
            }
        }
        Message::RenameInputChanged(value) => {
            if let Some((_, name)) = &mut app.rename_target {
                *name = value;
            }
        }
        Message::RenameCancelled => {
            app.rename_target = None;
        }
        Message::RenameConfirmed => {
            if let Some((old_path, new_name)) = app.rename_target.take() {
                let new_name = new_name.trim();
                if new_name.is_empty() || new_name.contains('/') {
                    app.last_error = Some(format!("invalid name: {new_name:?}"));
                } else if let Some(parent) = old_path.parent() {
                    let new_path = parent.join(new_name);
                    return tasks::spawn_rename(old_path, new_path);
                }
            }
        }
        Message::RenameCompleted(old_path, new_path, Ok(())) => {
            if let Some(root) = &mut app.root {
                scanner::rename_entry(root, &old_path, &new_path);
            }
            scanner::reindex_prefix(Arc::make_mut(&mut app.search_index), &old_path, &new_path);
            if let Some(rel) = app.selected.as_ref().and_then(|p| p.strip_prefix(&old_path).ok()) {
                app.selected = Some(new_path.join(rel));
            }
            let expanded = std::mem::take(&mut app.expanded);
            app.expanded = expanded
                .into_iter()
                .map(|p| match p.strip_prefix(&old_path) {
                    Ok(rel) => new_path.join(rel),
                    Err(_) => p,
                })
                .collect();
            app.last_error = None;
        }
        Message::RenameCompleted(old_path, _, Err(err)) => {
            app.last_error = Some(format!("couldn't rename {}: {err}", old_path.display()));
        }
        Message::Surface(action) => {
            return cosmic::task::message(cosmic::Action::Cosmic(
                cosmic::app::Action::Surface(action),
            ));
        }
        Message::Tick => {
            app.tick_count = app.tick_count.wrapping_add(1);
        }
    }
    Task::none()
}
