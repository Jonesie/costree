// SPDX-License-Identifier: MIT

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

/// A single file or directory node in the scanned tree.
#[derive(Debug, Clone)]
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

/// Lists only the immediate children of `path`. Files are fully resolved
/// (their size is cheap to read), but directories are returned unscanned
/// with size 0 so the caller can render them immediately and scan their
/// contents separately in the background.
pub fn list_top_level(path: &Path) -> Entry {
    let mut children: Vec<Entry> = fs::read_dir(path)
        .map(|read_dir| {
            read_dir
                .flatten()
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

    let mut children: Vec<Entry> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(path) {
        for dir_entry in read_dir.flatten() {
            children.push(scan(&dir_entry.path(), generation)?);
        }
    }

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

/// True if `entry`'s name, or the name of any entry in its subtree,
/// contains `query` (expected already lowercased).
pub fn subtree_matches(entry: &Entry, query: &str) -> bool {
    entry.name.to_lowercase().contains(query)
        || entry.children.iter().any(|c| subtree_matches(c, query))
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
