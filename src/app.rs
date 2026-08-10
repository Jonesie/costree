// SPDX-License-Identifier: MIT

mod subscription;
mod tasks;
mod update;
mod view;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use cosmic::iced::Subscription;
use cosmic::prelude::*;

use crate::scanner::{self, Entry};

const APP_TITLE: &str = "CosTree";

pub struct AppModel {
    core: cosmic::Core,
    scan_root: PathBuf,
    root: Option<Entry>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    /// Path awaiting user confirmation in the delete dialog.
    confirm_delete: Option<PathBuf>,
    last_error: Option<String>,
    /// True while the initial (fast, single-level) directory listing is in flight.
    listing: bool,
    /// Number of top-level directories whose recursive scan hasn't completed yet.
    pending_branches: usize,
    total_branches: usize,
    hide_dotfiles: bool,
    search_query: String,
    /// Interpret `search_query` as a regex instead of a literal substring.
    search_regex: bool,
    /// Match case exactly instead of the default case-insensitive search.
    search_case_sensitive: bool,
    /// Match on word boundaries only, not substrings.
    search_whole_word: bool,
    /// Flat index built incrementally as branches finish scanning, so
    /// search doesn't have to walk the nested tree itself. Arc'd so handing
    /// a snapshot to the background search task is an O(1) refcount bump
    /// instead of a full clone of a potentially huge Vec.
    search_index: Arc<scanner::SearchIndex>,
    /// Cached result of the last completed search. Computed only when the
    /// debounced search actually runs, not on every render — otherwise it
    /// was being recomputed on every 150ms scan-progress tick too, not
    /// just on keystrokes.
    search_results: Option<HashSet<PathBuf>>,
    /// True while waiting for typing to pause (or for the search itself to
    /// finish) — drives the "Searching…" indicator.
    searching: bool,
    /// Bumped on every keystroke; a debounced search only takes effect if
    /// its generation still matches this when it fires, so rapid typing
    /// doesn't stack up redundant searches.
    search_generation: u64,
    /// Text currently in the root-path field (may not match `scan_root` while being edited).
    root_input: String,
    /// True while the title is showing an editable path field (opened via
    /// the pen icon) instead of the plain scan-root text.
    title_editing: bool,
    /// Home, filesystem root, and other detected mounted volumes, for the quick-pick dropdown.
    quick_roots: Vec<(String, PathBuf)>,
    /// Current scan generation; bumped on every rescan or cancel so in-flight
    /// background scans can tell they've been superseded.
    generation: u64,
    /// Path and in-progress new name for the rename dialog.
    rename_target: Option<(PathBuf, String)>,
    /// Handle to the on-disk config, used to persist settings like
    /// `hide_dotfiles` between runs. `None` if it couldn't be opened.
    config_handle: Option<cosmic::cosmic_config::Config>,
    /// When the data currently displayed was scanned — either just now, or
    /// whenever a loaded `.costree` save was originally made. `None` while
    /// nothing has finished scanning/loading yet.
    last_scan_time: Option<u64>,
    /// True while a manual "Save index" write is in flight — drives the
    /// "Saving…" indicator and disables the Save button so a second save
    /// can't be queued on top of one already running.
    saving_index: bool,
    /// Free/total space on the filesystem `scan_root` lives on. Refreshed
    /// when a scan begins and after a successful delete — the two moments
    /// disk usage is most likely to have changed as a direct result of
    /// using costree. `None` if it hasn't been read yet or the syscall failed.
    disk_space: Option<scanner::DiskSpace>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TopLevelListed(Entry),
    BranchScanned(PathBuf, Option<Entry>),
    ToggleExpand(PathBuf),
    Select(PathBuf),
    Rescan,
    CancelScan,
    DeleteRequested,
    DeleteContextRequested(PathBuf),
    DeleteConfirmed,
    DeleteCancelled,
    DeleteCompleted(PathBuf, Result<(), String>),
    Tick,
    HideDotfilesToggled(bool),
    SearchChanged(String),
    SearchRegexToggled(bool),
    SearchCaseSensitiveToggled(bool),
    SearchWholeWordToggled(bool),
    SearchDebounced(u64),
    SearchResultsReady(u64, HashSet<PathBuf>),
    RootInputChanged(String),
    RootSubmitted,
    QuickRootSelected(usize),
    EditTitle,
    TitleEditCancelled,
    FolderPicked(PathBuf),
    FolderPickCancelled,
    FolderPickError(String),
    OpenInFiles(PathBuf),
    RenameRequested(PathBuf),
    RenameInputChanged(String),
    RenameConfirmed,
    RenameCancelled,
    RenameCompleted(PathBuf, PathBuf, Result<(), String>),
    SaveIndex,
    SaveIndexCompleted(Result<(), String>),
    /// Result of checking `<root>/.costree/` for a saved index when
    /// beginning a scan. `Some` loads it directly instead of scanning;
    /// `None` falls through to a normal fresh scan.
    SavedIndexChecked(Option<scanner::SavedIndex>),
    DiskSpaceChecked(Option<scanner::DiskSpace>),
    /// Plumbing required for context menus to open as native popups
    /// (correctly positioned at the cursor) instead of a slow in-window
    /// overlay fallback.
    Surface(cosmic::surface::Action),
}

impl AppModel {
    /// Resets scan-related state and kicks off a fresh top-level listing of `root`.
    fn begin_scan(&mut self, root: PathBuf) -> Task<cosmic::Action<Message>> {
        self.generation = scanner::next_generation();
        self.scan_root = root.clone();
        self.root_input = root.to_string_lossy().into_owned();
        self.title_editing = false;
        self.expanded.clear();
        self.expanded.insert(root.clone());
        self.listing = true;
        self.pending_branches = 0;
        self.total_branches = 0;
        self.root = None;
        self.selected = None;
        self.confirm_delete = None;
        self.rename_target = None;
        self.last_error = None;
        Arc::make_mut(&mut self.search_index).clear();
        self.search_results = None;
        self.searching = false;
        self.last_scan_time = None;
        self.saving_index = false;
        Task::batch([tasks::spawn_check_saved_index(root.clone()), tasks::spawn_disk_space(root)])
    }

    fn search_options(&self) -> scanner::SearchOptions {
        scanner::SearchOptions {
            regex: self.search_regex,
            case_sensitive: self.search_case_sensitive,
            whole_word: self.search_whole_word,
        }
    }

    /// Re-runs the current search immediately (no debounce) against the
    /// latest toggle state — used when a regex/case/whole-word toggle
    /// changes, since that's a discrete click rather than typing that
    /// benefits from waiting out a pause.
    fn rerun_search(&mut self) -> Task<cosmic::Action<Message>> {
        self.search_generation += 1;
        if self.search_query.is_empty() {
            self.search_results = None;
            self.searching = false;
            Task::none()
        } else {
            self.searching = true;
            tasks::spawn_search(
                self.search_index.clone(),
                self.search_query.clone(),
                self.search_options(),
                self.search_generation,
            )
        }
    }

    /// Cancels any scan in flight; already-scanned branches keep their data.
    fn cancel_scan(&mut self) {
        scanner::next_generation();
        scanner::clear_current_scan_path();
        self.pending_branches = 0;
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "net.jonesie.Costree";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let scan_root = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"));

        use cosmic::cosmic_config::CosmicConfigEntry;

        let config_handle =
            cosmic::cosmic_config::Config::new(Self::APP_ID, crate::config::Config::VERSION).ok();
        let hide_dotfiles = config_handle
            .as_ref()
            .map(|handle| {
                crate::config::Config::get_entry(handle)
                    .unwrap_or_else(|(_errors, config)| config)
                    .hide_dotfiles
            })
            .unwrap_or(false);

        let mut app = AppModel {
            core,
            scan_root: scan_root.clone(),
            root: None,
            expanded: HashSet::new(),
            selected: None,
            confirm_delete: None,
            last_error: None,
            listing: true,
            pending_branches: 0,
            total_branches: 0,
            hide_dotfiles,
            search_query: String::new(),
            search_regex: false,
            search_case_sensitive: false,
            search_whole_word: false,
            search_index: Arc::new(Vec::new()),
            search_results: None,
            searching: false,
            search_generation: 0,
            root_input: scan_root.to_string_lossy().into_owned(),
            title_editing: false,
            quick_roots: scanner::detect_roots(&scan_root),
            generation: 0,
            rename_target: None,
            config_handle,
            last_scan_time: None,
            saving_index: false,
            disk_space: None,
        };

        app.core_mut().set_header_title(APP_TITLE.to_string());

        let title_task = match app.core().main_window_id() {
            Some(id) => app.set_window_title(APP_TITLE.to_string(), id),
            None => Task::none(),
        };

        let scan_task = app.begin_scan(scan_root);

        (app, Task::batch([title_task, scan_task]))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        view::view(self)
    }

    fn footer(&self) -> Option<Element<'_, Self::Message>> {
        view::footer(self)
    }

    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        view::dialog(self)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        subscription::subscription(self)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        update::update(self, message)
    }
}
