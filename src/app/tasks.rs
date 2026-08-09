// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use cosmic::prelude::*;

use crate::scanner;

use super::Message;

pub(super) fn spawn_top_level_listing(root: PathBuf) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || scanner::list_top_level(&root))
                .await
                .expect("listing task panicked")
        },
        |entry| cosmic::Action::App(Message::TopLevelListed(entry)),
    )
}

pub(super) fn spawn_branch_scan(path: PathBuf, generation: u64) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let result_path = path.clone();
            let entry = tokio::task::spawn_blocking(move || scanner::scan(&path, generation))
                .await
                .expect("scan task panicked");
            (result_path, entry)
        },
        |(path, entry)| cosmic::Action::App(Message::BranchScanned(path, entry)),
    )
}

pub(super) fn spawn_delete(path: PathBuf, is_dir: bool) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let result_path = path.clone();
            let result = tokio::task::spawn_blocking(move || {
                if is_dir {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                }
            })
            .await
            .expect("delete task panicked");

            (result_path, result.map_err(|err| err.to_string()))
        },
        |(path, result)| cosmic::Action::App(Message::DeleteCompleted(path, result)),
    )
}

pub(super) fn spawn_rename(old_path: PathBuf, new_path: PathBuf) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let (result_old, result_new) = (old_path.clone(), new_path.clone());
            let result = tokio::task::spawn_blocking(move || std::fs::rename(&old_path, &new_path))
                .await
                .expect("rename task panicked");
            (result_old, result_new, result.map_err(|err| err.to_string()))
        },
        |(old_path, new_path, result)| {
            cosmic::Action::App(Message::RenameCompleted(old_path, new_path, result))
        },
    )
}

/// For a directory, launches COSMIC Files on it. cosmic-files has no CLI
/// support for selecting a specific file within its parent folder, so for a
/// file this instead opens it directly with its default application (via
/// `xdg-open`, same mechanism a double-click in a file manager would use).
pub(super) fn open_in_files(path: &Path) {
    if path.is_dir() {
        if let Err(err) = std::process::Command::new("cosmic-files").arg(path).spawn() {
            eprintln!("failed to launch cosmic-files: {err}");
        }
    } else if let Err(err) = std::process::Command::new("xdg-open").arg(path).spawn() {
        eprintln!("failed to open {}: {err}", path.display());
    }
}
