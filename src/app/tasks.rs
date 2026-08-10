// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use cosmic::prelude::*;
use tokio::sync::Semaphore;

use crate::scanner;

use super::Message;

/// Caps how many top-level directories get scanned concurrently. A large
/// home directory can easily have 100+ top-level entries; firing a blocking
/// OS thread for every single one at once (even on many cores) saturates
/// CPU and disk I/O badly enough to starve the GUI thread and make the
/// window appear frozen. Small and fixed rather than core-count-scaled,
/// since UI responsiveness matters more here than raw scan throughput.
static SCAN_CONCURRENCY: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(4));

/// How long typing has to pause before a search actually runs.
const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(150);

pub(super) fn spawn_search_debounce(generation: u64) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            generation
        },
        |generation| cosmic::Action::App(Message::SearchDebounced(generation)),
    )
}

/// Runs the actual search on a blocking thread, never the GUI thread —
/// `search_index()` is fast, but "fast" still isn't "free" on a huge index,
/// and running it inline in `update()` was blocking rendering (including
/// the "Searching…" indicator itself) long enough to look like a freeze.
/// Compiling `query`/`options` into a `Regex` happens once here per search
/// rather than once per index entry. An invalid regex (typically the user
/// mid-way through typing a pattern in regex mode) yields no matches rather
/// than erroring — a stale "no results" is much less disruptive than an
/// error popping in and out on every keystroke.
pub(super) fn spawn_search(
    index: Arc<scanner::SearchIndex>,
    query: String,
    options: scanner::SearchOptions,
    generation: u64,
) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let results = tokio::task::spawn_blocking(move || {
                scanner::compile_search_regex(&query, options)
                    .map(|pattern| scanner::search_index(&index, &pattern))
                    .unwrap_or_default()
            })
            .await
            .expect("search task panicked");
            (generation, results)
        },
        |(generation, results)| cosmic::Action::App(Message::SearchResultsReady(generation, results)),
    )
}

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

/// Checks `<root>/.costree/` for a saved index before deciding how to begin
/// a scan. Runs on a blocking thread even though a single failed/missing
/// file read is cheap, for consistency with how every other filesystem
/// operation in this module avoids the GUI thread.
pub(super) fn spawn_check_saved_index(root: PathBuf) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || scanner::load_index(&root))
                .await
                .expect("load task panicked")
        },
        |saved| cosmic::Action::App(Message::SavedIndexChecked(saved)),
    )
}

pub(super) fn spawn_save_index(root_entry: scanner::Entry, dest: PathBuf) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || scanner::save_index(&root_entry, &dest))
                .await
                .expect("save task panicked")
        },
        |result| cosmic::Action::App(Message::SaveIndexCompleted(result)),
    )
}

pub(super) fn spawn_branch_scan(path: PathBuf, generation: u64) -> Task<cosmic::Action<Message>> {
    Task::perform(
        async move {
            let _permit = SCAN_CONCURRENCY.acquire().await.expect("semaphore never closes");
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

/// Opens the native folder-picker dialog, starting from `current`. Ashpd
/// (the xdg-portal backend) doesn't actually honor the starting directory
/// yet, but setting it is harmless and future-proof.
pub(super) fn spawn_folder_picker(current: PathBuf) -> Task<cosmic::Action<Message>> {
    cosmic::task::future(async move {
        let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
            .title("Choose a folder to scan")
            .directory(current);

        match dialog.open_folder().await {
            Ok(response) => match response.url().to_file_path() {
                Ok(path) => Message::FolderPicked(path),
                Err(()) => {
                    Message::FolderPickError(format!("unsupported location: {}", response.url()))
                }
            },
            Err(cosmic::dialog::file_chooser::Error::Cancelled) => Message::FolderPickCancelled,
            Err(err) => Message::FolderPickError(err.to_string()),
        }
    })
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
