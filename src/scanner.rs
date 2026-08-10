// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

/// Name of the folder a scanned root's saved index is stored under. Scans
/// skip this folder entirely — it's app metadata, not user data to measure.
pub const INDEX_DIR_NAME: &str = ".costree";
const INDEX_FILE_NAME: &str = "index.bin";
const INDEX_FORMAT_VERSION: u32 = 1;

/// A single file or directory node in the scanned tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    /// True once `size` and `children` reflect the full subtree. Directories
    /// start out unscanned so the top level can be shown before recursion
    /// into their contents has finished.
    pub scanned: bool,
    /// Populated for directories; sorted largest-first.
    pub children: Vec<Entry>,
}

/// Last directory visited by any in-flight `scan()` call, across however
/// many branches are scanning concurrently. `Subscription::run` only accepts
/// non-capturing function pointers, so a global is the simplest way to get
/// this to the status bar rather than threading a channel through it.
static CURRENT_SCAN_PATH: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));

/// The directory most recently entered by a background scan, for display in
/// the status bar. `None` when nothing is scanning.
pub fn current_scan_path() -> Option<PathBuf> {
    CURRENT_SCAN_PATH.lock().ok().and_then(|guard| guard.clone())
}

pub fn clear_current_scan_path() {
    if let Ok(mut guard) = CURRENT_SCAN_PATH.lock() {
        *guard = None;
    }
}

/// Identifies the "current" scan attempt. `scan()` checks this periodically
/// and bails out as soon as it no longer matches the generation it was
/// started with, which is how cancellation works: bumping the generation
/// invalidates every scan in flight without needing to reach into their
/// (blocking, OS-thread-bound) call stacks directly.
static SCAN_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn current_generation() -> u64 {
    SCAN_GENERATION.load(Ordering::SeqCst)
}

/// Starts a new scan generation, invalidating any scan already in flight.
/// Returns the new generation id to pass to `scan()`.
pub fn next_generation() -> u64 {
    SCAN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

fn entry_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn is_index_dir(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == INDEX_DIR_NAME)
}

/// Lists only the immediate children of `path`. Files are fully resolved
/// (their size is cheap to read), but directories are returned unscanned
/// with size 0 so the caller can render them immediately and scan their
/// contents separately in the background.
pub fn list_top_level(path: &Path) -> Entry {
    let mut children: Vec<Entry> = fs::read_dir(path)
        .map(|read_dir| {
            read_dir
                .flatten()
                .filter(|dir_entry| !is_index_dir(&dir_entry.path()))
                .map(|dir_entry| {
                    let child_path = dir_entry.path();
                    let metadata = fs::symlink_metadata(&child_path);
                    let is_dir = metadata.as_ref().map(fs::Metadata::is_dir).unwrap_or(false);

                    if is_dir {
                        Entry {
                            name: entry_name(&child_path),
                            path: child_path,
                            size: 0,
                            is_dir: true,
                            scanned: false,
                            children: Vec::new(),
                        }
                    } else {
                        let size = metadata.map(|m| m.len()).unwrap_or(0);
                        Entry {
                            name: entry_name(&child_path),
                            path: child_path,
                            size,
                            is_dir: false,
                            scanned: true,
                            children: Vec::new(),
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    children.sort_by(|a, b| b.size.cmp(&a.size));
    let size = children.iter().map(|c| c.size).sum();

    Entry {
        name: entry_name(path),
        path: path.to_path_buf(),
        size,
        is_dir: true,
        scanned: false,
        children,
    }
}

/// Recursively scans `path`, computing the size of every file and the
/// cumulative size of every directory. Unreadable entries (permission
/// denied, broken symlinks, etc.) are skipped rather than aborting the scan.
///
/// Checks `generation` against the live scan generation before descending
/// into each directory, returning `None` as soon as it's stale (i.e. the
/// scan was cancelled or superseded by a new one).
pub fn scan(path: &Path, generation: u64) -> Option<Entry> {
    if current_generation() != generation {
        return None;
    }

    let metadata = fs::symlink_metadata(path);
    let is_dir = metadata.as_ref().map(fs::Metadata::is_dir).unwrap_or(false);

    if is_dir {
        if let Ok(mut guard) = CURRENT_SCAN_PATH.lock() {
            *guard = Some(path.to_path_buf());
        }
    }

    if !is_dir {
        let size = metadata.map(|m| m.len()).unwrap_or(0);
        return Some(Entry {
            name: entry_name(path),
            path: path.to_path_buf(),
            size,
            is_dir: false,
            scanned: true,
            children: Vec::new(),
        });
    }

    // Recurses into children in parallel via rayon's work-stealing pool
    // (shared and bounded to available cores, so this scales across a whole
    // scan without the thread-explosion risk of spawning an OS thread per
    // directory) — stat-heavy directory walks are latency-bound on I/O, so
    // more concurrent readers finish a tree far faster than walking it
    // single-threaded. `collect::<Option<Vec<_>>>()` short-circuits to
    // `None` the same way the old sequential `?`-per-child loop did if the
    // generation goes stale partway through.
    let mut children: Vec<Entry> = match fs::read_dir(path) {
        Ok(read_dir) => {
            let child_paths: Vec<PathBuf> = read_dir
                .flatten()
                .map(|dir_entry| dir_entry.path())
                .filter(|child_path| !is_index_dir(child_path))
                .collect();
            child_paths
                .par_iter()
                .map(|child_path| scan(child_path, generation))
                .collect::<Option<Vec<_>>>()?
        }
        Err(_) => Vec::new(),
    };

    children.sort_by(|a, b| b.size.cmp(&a.size));
    let size = children.iter().map(|c| c.size).sum();

    Some(Entry {
        name: entry_name(path),
        path: path.to_path_buf(),
        size,
        is_dir: true,
        scanned: true,
        children,
    })
}

/// An on-disk snapshot of a scan, saved under `<root>/.costree/index.json`
/// so it can be reloaded later without rescanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedIndex {
    version: u32,
    /// Unix timestamp (seconds) of when this snapshot was saved.
    pub scanned_at: u64,
    pub root: Entry,
}

/// Current time as a Unix timestamp (seconds), or 0 if the clock is somehow
/// set before the epoch.
pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Saves `root` under `<dest>/.costree/index.bin`, in bincode's compact
/// binary encoding rather than a text format — the tree stores a full path
/// per entry, so a large scan's save file adds up fast in a verbose format
/// like JSON. Streams straight to the file rather than building an
/// in-memory buffer first, to keep peak memory down on a huge tree. Fails
/// gracefully (no panics) if `dest` isn't writable — the caller is expected
/// to surface the error rather than treat it as fatal.
pub fn save_index(root: &Entry, dest: &Path) -> Result<(), String> {
    let index_dir = dest.join(INDEX_DIR_NAME);
    fs::create_dir_all(&index_dir).map_err(|err| err.to_string())?;

    let saved = SavedIndex { version: INDEX_FORMAT_VERSION, scanned_at: now_unix(), root: root.clone() };
    let mut file = fs::File::create(index_dir.join(INDEX_FILE_NAME)).map_err(|err| err.to_string())?;
    bincode::serde::encode_into_std_write(&saved, &mut file, bincode::config::standard())
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Loads a previously saved index for `root`, if one exists and matches the
/// current format version. Returns `None` (not an error) for any failure —
/// missing/unreadable/corrupt/outdated saves should just fall back to a
/// fresh scan rather than blocking the user with an error.
pub fn load_index(root: &Path) -> Option<SavedIndex> {
    let path = root.join(INDEX_DIR_NAME).join(INDEX_FILE_NAME);
    let mut file = fs::File::open(path).ok()?;
    let saved: SavedIndex =
        bincode::serde::decode_from_std_read(&mut file, bincode::config::standard()).ok()?;
    (saved.version == INDEX_FORMAT_VERSION).then_some(saved)
}

/// Formats a Unix timestamp as a short relative string, e.g. `"12m ago"`.
pub fn format_relative_time(timestamp: u64) -> String {
    let elapsed = now_unix().saturating_sub(timestamp);

    if elapsed < 60 {
        "just now".to_string()
    } else if elapsed < 3600 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 86400 {
        format!("{}h ago", elapsed / 3600)
    } else {
        format!("{}d ago", elapsed / 86400)
    }
}

/// Shortens an absolute path for display in the status bar, so a deep tree
/// doesn't wrap (or push everything else off) the status row while
/// scanning — e.g. `/hdd2/media/backups/2024/photos` becomes
/// `/hdd2/.../2024/photos/`. Only collapses when doing so actually hides at
/// least two segments; shallow paths are returned unchanged.
pub fn shorten_scan_path(path: &Path) -> String {
    let full = path.display().to_string();
    let segments: Vec<&str> = full.split('/').filter(|s| !s.is_empty()).collect();

    // Only collapse when doing so actually hides at least two segments —
    // below that, the shortened form isn't meaningfully shorter than the
    // original.
    if segments.len() < 5 {
        return full;
    }

    let first = segments[0];
    let last_two = &segments[segments.len() - 2..];
    format!("/{first}/.../{}/{}/", last_two[0], last_two[1])
}

/// Finds the entry at `target` anywhere in the tree rooted at `node`.
pub fn find_entry<'a>(node: &'a Entry, target: &Path) -> Option<&'a Entry> {
    if node.path == target {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_entry(child, target))
}

/// Removes the entry at `target` from the tree rooted at `node`, recomputing
/// the size and sort order of every ancestor along the way. Returns `true`
/// if an entry was removed.
pub fn remove_path(node: &mut Entry, target: &Path) -> bool {
    let removed = if let Some(idx) = node.children.iter().position(|c| c.path == target) {
        node.children.remove(idx);
        true
    } else {
        node.children
            .iter_mut()
            .any(|child| child.is_dir && remove_path(child, target))
    };

    if removed {
        node.size = node.children.iter().map(|c| c.size).sum();
        node.children.sort_by(|a, b| b.size.cmp(&a.size));
    }

    removed
}

/// Renames the entry at `target` (and, if it's a directory, every
/// descendant's path prefix) to `new_path` in the tree rooted at `node`.
/// Returns `true` if an entry was renamed.
pub fn rename_entry(node: &mut Entry, target: &Path, new_path: &Path) -> bool {
    if node.path == target {
        rewrite_paths(node, target, new_path);
        return true;
    }
    node.children
        .iter_mut()
        .any(|child| child.is_dir && rename_entry(child, target, new_path))
}

fn rewrite_paths(node: &mut Entry, old_prefix: &Path, new_prefix: &Path) {
    if let Ok(rel) = node.path.strip_prefix(old_prefix) {
        node.path = new_prefix.join(rel);
    }
    node.name = node
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.path.to_string_lossy().into_owned());

    for child in &mut node.children {
        rewrite_paths(child, old_prefix, new_prefix);
    }
}

/// A flattened `(path, name)` entry for fast searching, built incrementally
/// as branches finish scanning so a search never has to walk the nested
/// tree itself. Names keep their original case — case sensitivity is a
/// per-search toggle handled by the compiled `Regex`, not baked into the
/// index.
pub type SearchIndex = Vec<(PathBuf, String)>;

/// Which of the search toggles (regex / case-sensitive / whole-word) are
/// active. All three collapse into a single compiled `Regex` via
/// [`compile_search_regex`] rather than branching the matching logic three
/// separate ways.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// Compiles `query` into a `Regex` per `options`: a literal query is
/// `regex::escape`d first so the toggles behave the same whether the user
/// is typing plain text or a real pattern. `whole_word` wraps the result in
/// `\b...\b` regardless of whether regex mode is on, so it composes with a
/// user-supplied pattern the same way it does with a literal search term.
/// Returns `Err` on invalid regex input — expected while the user is
/// mid-way through typing a pattern, not a bug to surface loudly.
pub fn compile_search_regex(query: &str, options: SearchOptions) -> Result<Regex, regex::Error> {
    let pattern = if options.regex { query.to_string() } else { regex::escape(query) };
    let pattern = if options.whole_word { format!(r"\b(?:{pattern})\b") } else { pattern };
    RegexBuilder::new(&pattern).case_insensitive(!options.case_sensitive).build()
}

/// Appends `entry` and every descendant to `index`.
pub fn index_subtree(entry: &Entry, index: &mut SearchIndex) {
    index.push((entry.path.clone(), entry.name.clone()));
    for child in &entry.children {
        index_subtree(child, index);
    }
}

/// Removes every indexed entry at or under `path` (e.g. after a delete).
pub fn remove_from_index(index: &mut SearchIndex, path: &Path) {
    index.retain(|(p, _)| !p.starts_with(path));
}

/// Rewrites every indexed path with the `old_prefix` prefix to use
/// `new_prefix` instead (e.g. after a rename).
pub fn reindex_prefix(index: &mut SearchIndex, old_prefix: &Path, new_prefix: &Path) {
    for (path, _) in index.iter_mut() {
        if let Ok(rel) = path.strip_prefix(old_prefix) {
            *path = new_prefix.join(rel);
        }
    }
}

/// Returns the paths of every indexed entry whose name matches `pattern`,
/// plus every ancestor of a match — i.e. exactly the set of entries a
/// search-filtered tree view should keep.
///
/// Once an ancestor has been recorded by an earlier match, every path
/// above it must already be recorded too, so walking stops as soon as it
/// hits one — this keeps the total ancestor-walking work bounded across
/// the whole index rather than repeating it per match.
pub fn search_index(index: &SearchIndex, pattern: &Regex) -> HashSet<PathBuf> {
    let mut matches = HashSet::new();
    for (path, name) in index {
        if !pattern.is_match(name) {
            continue;
        }
        for ancestor in path.ancestors() {
            if !matches.insert(ancestor.to_path_buf()) {
                break;
            }
        }
    }
    matches
}

/// Quick-access scan roots: the user's home directory, the filesystem root,
/// and any other real (non-virtual) mounted filesystems, so the user can
/// point costree at a whole drive or a separate mounted volume instead of
/// just `$HOME`.
pub fn detect_roots(home: &Path) -> Vec<(String, PathBuf)> {
    let mut roots = vec![
        ("Home".to_string(), home.to_path_buf()),
        ("Filesystem (/)".to_string(), PathBuf::from("/")),
    ];

    const REAL_FILESYSTEMS: &[&str] = &[
        "ext2", "ext3", "ext4", "btrfs", "xfs", "zfs", "ntfs", "ntfs3", "vfat", "exfat", "f2fs",
        "reiserfs", "jfs", "hfsplus",
    ];

    let Ok(contents) = fs::read_to_string("/proc/mounts") else {
        return roots;
    };

    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_device), Some(mount_point), Some(fstype)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };

        if !REAL_FILESYSTEMS.contains(&fstype) {
            continue;
        }

        // /proc/mounts escapes spaces (and a few other characters) as octal.
        let mount_point = PathBuf::from(mount_point.replace("\\040", " "));
        if roots.iter().any(|(_, p)| *p == mount_point) {
            continue;
        }

        let label = mount_point.to_string_lossy().into_owned();
        roots.push((label, mount_point));
    }

    roots
}

/// Free/total space (in bytes) for the filesystem containing a path.
#[derive(Debug, Clone, Copy)]
pub struct DiskSpace {
    pub total: u64,
    pub free: u64,
}

/// Reads free/total space for the filesystem containing `path`, via
/// `statvfs(3)`. `statvfs` reports on the filesystem containing whatever
/// path is passed to it, not just its mount point, so there's no need to
/// separately resolve which mount `path` lives on the way
/// `detect_roots()` has to. Returns `None` if the syscall fails (e.g. the
/// path no longer exists) — this is a status readout, not something that
/// should ever surface as an error to the user.
pub fn disk_space(path: &Path) -> Option<DiskSpace> {
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }

    let block_size = stat.f_frsize as u64;
    Some(DiskSpace {
        total: stat.f_blocks as u64 * block_size,
        // f_bavail (blocks available to an unprivileged user) rather than
        // f_bfree (which includes root-reserved blocks) — matches what a
        // user would understand "free space" to mean.
        free: stat.f_bavail as u64 * block_size,
    })
}

/// Formats a byte count as a human-readable string, e.g. `4.2 GB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `scan()`'s cancellation check reads a global generation counter, so
    /// tests that rely on a specific generation value must not run
    /// concurrently with each other (cargo test runs tests in parallel by
    /// default within one process).
    static GENERATION_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn entry(name: &str, path: &str, size: u64, is_dir: bool, children: Vec<Entry>) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(path),
            size,
            is_dir,
            scanned: true,
            children,
        }
    }

    #[test]
    fn human_size_formats_bytes() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn disk_space_reports_nonzero_totals_for_a_real_path() {
        let space = disk_space(Path::new("/")).expect("statvfs on / should succeed");
        assert!(space.total > 0);
        assert!(space.free <= space.total);
    }

    #[test]
    fn disk_space_returns_none_for_a_nonexistent_path() {
        assert!(disk_space(Path::new("/no/such/path/costree-test")).is_none());
    }

    #[test]
    fn find_entry_locates_nested_path() {
        let child = entry("b", "/a/b", 10, false, vec![]);
        let root = entry("a", "/a", 10, true, vec![child]);

        assert_eq!(find_entry(&root, Path::new("/a/b")).map(|e| &e.name), Some(&"b".to_string()));
        assert!(find_entry(&root, Path::new("/a/missing")).is_none());
    }

    #[test]
    fn remove_path_updates_ancestor_size_and_sort_order() {
        let small = entry("small", "/a/small", 10, false, vec![]);
        let big = entry("big", "/a/big", 100, false, vec![]);
        let mut root = entry("a", "/a", 110, true, vec![big, small]);

        assert!(remove_path(&mut root, Path::new("/a/big")));
        assert_eq!(root.size, 10);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "small");
    }

    #[test]
    fn remove_path_returns_false_for_missing_entry() {
        let mut root = entry("a", "/a", 0, true, vec![]);
        assert!(!remove_path(&mut root, Path::new("/a/missing")));
        assert_eq!(root.children.len(), 0);
    }

    #[test]
    fn remove_path_recurses_into_nested_directories() {
        let grandchild = entry("g", "/a/b/g", 5, false, vec![]);
        let child = entry("b", "/a/b", 5, true, vec![grandchild]);
        let mut root = entry("a", "/a", 5, true, vec![child]);

        assert!(remove_path(&mut root, Path::new("/a/b/g")));
        assert_eq!(root.size, 0);
        assert_eq!(root.children[0].children.len(), 0);
    }

    #[test]
    fn rename_entry_rewrites_descendant_paths() {
        let grandchild = entry("g", "/a/b/g", 5, false, vec![]);
        let child = entry("b", "/a/b", 5, true, vec![grandchild]);
        let mut root = entry("a", "/a", 5, true, vec![child]);

        assert!(rename_entry(&mut root, Path::new("/a/b"), Path::new("/a/c")));

        let renamed = &root.children[0];
        assert_eq!(renamed.name, "c");
        assert_eq!(renamed.path, PathBuf::from("/a/c"));
        assert_eq!(renamed.children[0].path, PathBuf::from("/a/c/g"));
        assert_eq!(renamed.children[0].name, "g");
    }

    #[test]
    fn rename_entry_returns_false_for_missing_entry() {
        let mut root = entry("a", "/a", 0, true, vec![]);
        assert!(!rename_entry(&mut root, Path::new("/a/missing"), Path::new("/a/renamed")));
    }

    #[test]
    fn index_subtree_flattens_and_preserves_case() {
        let child = entry("Target.TXT", "/a/Target.TXT", 1, false, vec![]);
        let root = entry("A", "/a", 1, true, vec![child]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        assert_eq!(index.len(), 2);
        assert!(index.contains(&(PathBuf::from("/a"), "A".to_string())));
        assert!(index.contains(&(PathBuf::from("/a/Target.TXT"), "Target.TXT".to_string())));
    }

    fn regex(query: &str, options: SearchOptions) -> Regex {
        compile_search_regex(query, options).expect("valid pattern")
    }

    #[test]
    fn search_index_includes_matches_and_their_ancestors() {
        let hit = entry("target.txt", "/a/b/target.txt", 1, false, vec![]);
        let miss = entry("other.txt", "/a/b/other.txt", 1, false, vec![]);
        let b = entry("b", "/a/b", 2, true, vec![hit, miss]);
        let root = entry("a", "/a", 2, true, vec![b]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);
        let matches = search_index(&index, &regex("target", SearchOptions::default()));

        assert!(matches.contains(Path::new("/a/b/target.txt")));
        assert!(matches.contains(Path::new("/a/b"))); // ancestor of a match
        assert!(matches.contains(Path::new("/a"))); // ancestor of a match
        assert!(!matches.contains(Path::new("/a/b/other.txt")));
    }

    #[test]
    fn search_index_is_empty_when_nothing_matches() {
        let child = entry("file.txt", "/a/file.txt", 1, false, vec![]);
        let root = entry("a", "/a", 1, true, vec![child]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        assert!(search_index(&index, &regex("nope", SearchOptions::default())).is_empty());
    }

    #[test]
    fn search_is_case_insensitive_by_default_and_case_sensitive_when_toggled() {
        let child = entry("Target.txt", "/a/Target.txt", 1, false, vec![]);
        let root = entry("a", "/a", 1, true, vec![child]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        assert!(search_index(&index, &regex("target", SearchOptions::default()))
            .contains(Path::new("/a/Target.txt")));

        let case_sensitive = SearchOptions { case_sensitive: true, ..Default::default() };
        assert!(!search_index(&index, &regex("target", case_sensitive))
            .contains(Path::new("/a/Target.txt")));
        assert!(search_index(&index, &regex("Target", case_sensitive))
            .contains(Path::new("/a/Target.txt")));
    }

    #[test]
    fn whole_word_option_rejects_substring_matches() {
        let child = entry("target.txt", "/a/target.txt", 1, false, vec![]);
        let root = entry("a", "/a", 1, true, vec![child]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        let whole_word = SearchOptions { whole_word: true, ..Default::default() };
        assert!(search_index(&index, &regex("target", whole_word)).contains(Path::new("/a/target.txt")));
        assert!(search_index(&index, &regex("targe", whole_word)).is_empty());
    }

    #[test]
    fn literal_query_with_regex_metacharacters_is_escaped_when_regex_mode_is_off() {
        let child = entry("a.b", "/a.b", 1, false, vec![]);
        let other = entry("axb", "/axb", 1, false, vec![]);
        let root = entry("", "/", 0, true, vec![child, other]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        let matches = search_index(&index, &regex("a.b", SearchOptions::default()));
        assert!(matches.contains(Path::new("/a.b")));
        assert!(!matches.contains(Path::new("/axb")));
    }

    #[test]
    fn regex_mode_interprets_metacharacters() {
        let child = entry("a.b", "/a.b", 1, false, vec![]);
        let other = entry("axb", "/axb", 1, false, vec![]);
        let root = entry("", "/", 0, true, vec![child, other]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);

        let regex_on = SearchOptions { regex: true, ..Default::default() };
        let matches = search_index(&index, &regex("a.b", regex_on));
        assert!(matches.contains(Path::new("/a.b")));
        assert!(matches.contains(Path::new("/axb")));
    }

    #[test]
    fn invalid_regex_fails_to_compile_instead_of_panicking() {
        let regex_on = SearchOptions { regex: true, ..Default::default() };
        assert!(compile_search_regex("(unclosed", regex_on).is_err());
    }

    #[test]
    fn remove_from_index_drops_path_and_descendants() {
        let child = entry("g", "/a/b/g", 1, false, vec![]);
        let b = entry("b", "/a/b", 1, true, vec![child]);
        let root = entry("a", "/a", 1, true, vec![b]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);
        remove_from_index(&mut index, Path::new("/a/b"));

        assert_eq!(index, vec![(PathBuf::from("/a"), "a".to_string())]);
    }

    #[test]
    fn reindex_prefix_rewrites_matching_paths() {
        let child = entry("g", "/a/b/g", 1, false, vec![]);
        let b = entry("b", "/a/b", 1, true, vec![child]);
        let root = entry("a", "/a", 1, true, vec![b]);

        let mut index = SearchIndex::new();
        index_subtree(&root, &mut index);
        reindex_prefix(&mut index, Path::new("/a/b"), Path::new("/a/c"));

        assert!(index.contains(&(PathBuf::from("/a/c"), "b".to_string())));
        assert!(index.contains(&(PathBuf::from("/a/c/g"), "g".to_string())));
        assert!(index.contains(&(PathBuf::from("/a"), "a".to_string())));
    }

    #[test]
    fn detect_roots_always_includes_home_and_filesystem_root() {
        let home = PathBuf::from("/home/example");
        let roots = detect_roots(&home);

        assert!(roots.iter().any(|(label, path)| label == "Home" && path == &home));
        assert!(roots.iter().any(|(_, path)| path.as_path() == Path::new("/")));
    }

    #[test]
    fn list_top_level_reads_real_directory() {
        let dir = tempfile::tempdir().expect("create temp dir");
        fs::write(dir.path().join("file.txt"), b"hello").expect("write file");
        fs::create_dir(dir.path().join("subdir")).expect("create subdir");

        let listed = list_top_level(dir.path());

        assert!(listed.is_dir);
        assert!(!listed.scanned);
        assert_eq!(listed.children.len(), 2);

        let file = listed.children.iter().find(|c| c.name == "file.txt").expect("file listed");
        assert!(!file.is_dir);
        assert!(file.scanned);
        assert_eq!(file.size, 5);

        let subdir = listed.children.iter().find(|c| c.name == "subdir").expect("subdir listed");
        assert!(subdir.is_dir);
        assert!(!subdir.scanned);
        assert_eq!(subdir.size, 0);
    }

    #[test]
    fn scan_computes_recursive_size() {
        let _guard = GENERATION_TEST_LOCK.lock().unwrap();

        let dir = tempfile::tempdir().expect("create temp dir");
        fs::write(dir.path().join("a.txt"), b"12345").expect("write file"); // 5 bytes
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).expect("create subdir");
        fs::write(sub.join("b.txt"), b"1234567890").expect("write file"); // 10 bytes

        let generation = next_generation();
        let scanned = scan(dir.path(), generation).expect("scan should not be cancelled");

        assert!(scanned.scanned);
        assert_eq!(scanned.size, 15);

        let sub_entry = scanned.children.iter().find(|c| c.name == "sub").expect("subdir scanned");
        assert!(sub_entry.scanned);
        assert_eq!(sub_entry.size, 10);
    }

    #[test]
    fn scan_returns_none_once_generation_is_superseded() {
        let _guard = GENERATION_TEST_LOCK.lock().unwrap();

        let dir = tempfile::tempdir().expect("create temp dir");
        fs::create_dir(dir.path().join("sub")).expect("create subdir");

        let stale_generation = next_generation();
        next_generation(); // supersede it

        assert!(scan(dir.path(), stale_generation).is_none());
    }

    #[test]
    fn save_and_load_index_round_trips() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let child = entry("b", "/a/b", 10, false, vec![]);
        let root = entry("a", "/a", 10, true, vec![child]);

        save_index(&root, dir.path()).expect("save should succeed");

        let loaded = load_index(dir.path()).expect("load should find the saved index");
        assert_eq!(loaded.root.name, "a");
        assert_eq!(loaded.root.children[0].name, "b");
        assert_eq!(loaded.root.children[0].size, 10);
    }

    #[test]
    fn load_index_returns_none_when_absent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        assert!(load_index(dir.path()).is_none());
    }

    #[test]
    fn save_index_fails_gracefully_on_unwritable_destination() {
        let root = entry("a", "/a", 0, true, vec![]);
        // A path that can't exist as a writable directory.
        let dest = Path::new("/dev/null/costree-test-should-not-exist");
        assert!(save_index(&root, dest).is_err());
    }

    #[test]
    fn format_relative_time_buckets_correctly() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(format_relative_time(now), "just now");
        assert_eq!(format_relative_time(now - 120), "2m ago");
        assert_eq!(format_relative_time(now - 7200), "2h ago");
        assert_eq!(format_relative_time(now - 172_800), "2d ago");
    }

    #[test]
    fn shorten_scan_path_leaves_shallow_paths_untouched() {
        assert_eq!(shorten_scan_path(Path::new("/hdd2/somefolder1/somefolder2")), "/hdd2/somefolder1/somefolder2");
        assert_eq!(shorten_scan_path(Path::new("/a/b/c/d")), "/a/b/c/d");
    }

    #[test]
    fn shorten_scan_path_collapses_deep_paths() {
        assert_eq!(
            shorten_scan_path(Path::new("/hdd2/mnt/data/somefolder1/somefolder2")),
            "/hdd2/.../somefolder1/somefolder2/"
        );
        assert_eq!(
            shorten_scan_path(Path::new("/a/b/c/d/e/f/g")),
            "/a/.../f/g/"
        );
    }

    #[test]
    fn scan_skips_the_index_directory() {
        let _guard = GENERATION_TEST_LOCK.lock().unwrap();

        let dir = tempfile::tempdir().expect("create temp dir");
        fs::write(dir.path().join("file.txt"), b"hi").expect("write file");
        fs::create_dir(dir.path().join(INDEX_DIR_NAME)).expect("create index dir");
        fs::write(dir.path().join(INDEX_DIR_NAME).join("index.json"), b"{}")
            .expect("write index file");

        let generation = next_generation();
        let scanned = scan(dir.path(), generation).expect("scan should not be cancelled");

        assert_eq!(scanned.children.len(), 1);
        assert_eq!(scanned.children[0].name, "file.txt");
    }

    #[test]
    fn list_top_level_skips_the_index_directory() {
        let dir = tempfile::tempdir().expect("create temp dir");
        fs::write(dir.path().join("file.txt"), b"hi").expect("write file");
        fs::create_dir(dir.path().join(INDEX_DIR_NAME)).expect("create index dir");

        let listed = list_top_level(dir.path());

        assert_eq!(listed.children.len(), 1);
        assert_eq!(listed.children[0].name, "file.txt");
    }
}
