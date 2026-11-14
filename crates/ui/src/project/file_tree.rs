//! Turns a worktree's file list into the Files panel's rows: sorted, with closed folders' contents
//! left out, chains of single-child folders folded, and a filter that opens the folders down to
//! every match.

use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use model::WorktreeFiles;

use super::files_options::{IgnoredFiles, Sort};

/// What decides which rows show, read from the panel's settings.
pub(crate) struct TreeOptions {
    pub dotfiles: bool,
    pub ignored: IgnoredFiles,
    pub fold: bool,
    pub sort: Sort,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RowKind {
    /// Open or closed. An ignored folder can't open, since its contents aren't listed.
    Folder {
        open: bool,
        openable: bool,
    },
    File,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Row {
    /// Relative to the worktree's root. A folded chain's path is its deepest folder's.
    pub path: PathBuf,
    /// The last part of the path, or a folded chain joined with `/`.
    pub name: String,
    pub depth: usize,
    pub kind: RowKind,
    pub ignored: bool,
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    file: bool,
    ignored: bool,
    /// An ignored folder, whose contents aren't listed.
    sealed: bool,
}

/// The rows for `files`, opening the folders in `open`. A non-empty `filter` keeps only files
/// whose path contains it, ignoring case, and opens every folder above them.
pub(crate) fn rows(
    files: &WorktreeFiles,
    open: &HashSet<PathBuf>,
    filter: &str,
    options: &TreeOptions,
) -> Vec<Row> {
    let filter = filter.to_lowercase();
    let shown = |path: &Path| {
        (options.dotfiles
            || !path
                .iter()
                .any(|part| part.to_string_lossy().starts_with('.')))
            && path.to_string_lossy().to_lowercase().contains(&filter)
    };
    let with_ignored = options.ignored != IgnoredFiles::Hidden;
    let mut root = Node::default();
    let entries = files
        .files
        .iter()
        .map(|path| (path, false, false))
        .chain(
            files
                .ignored_files
                .iter()
                .filter(|_| with_ignored)
                .map(|path| (path, true, false)),
        )
        .chain(
            files
                .ignored_folders
                .iter()
                .filter(|_| with_ignored && filter.is_empty())
                .map(|path| (path, true, true)),
        );
    for (path, ignored, sealed) in entries {
        if shown(path) {
            root.insert(path, ignored, sealed);
        }
    }
    let mut walk = Walk {
        open,
        open_all: !filter.is_empty(),
        dim: options.ignored == IgnoredFiles::Dimmed,
        options,
        rows: Vec::new(),
    };
    walk.folder(&root, Path::new(""), 0);
    walk.rows
}

/// What the walk down the tree carries, and the rows it has made so far.
struct Walk<'a> {
    open: &'a HashSet<PathBuf>,
    /// Set while filtering, when every folder above a match opens.
    open_all: bool,
    dim: bool,
    options: &'a TreeOptions,
    rows: Vec<Row>,
}

impl Node {
    fn insert(&mut self, path: &Path, ignored: bool, sealed: bool) {
        let mut node = self;
        for part in path.iter() {
            node = node
                .children
                .entry(part.to_string_lossy().into_owned())
                .or_default();
        }
        node.file = !sealed;
        node.ignored = ignored;
        node.sealed = sealed;
    }

    fn is_folder(&self) -> bool {
        !self.file
    }
}

impl Walk<'_> {
    /// Adds the rows for `folder`'s children, which sit `depth` levels in under `path`.
    fn folder(&mut self, folder: &Node, path: &Path, depth: usize) {
        let options = self.options;
        let mut children: Vec<_> = folder.children.iter().collect();
        children.sort_by_key(|(name, node)| {
            let files_after = options.sort == Sort::FoldersFirst && !node.is_folder();
            (files_after, name.to_lowercase())
        });
        for (name, node) in children {
            let mut name = name.clone();
            let mut full = path.join(&name);
            let mut node = node;
            if node.is_folder() && options.fold {
                while let [(next, inner)] = node.children.iter().collect::<Vec<_>>()[..]
                    && inner.is_folder()
                    && !inner.sealed
                {
                    name = format!("{name}/{next}");
                    full = full.join(next);
                    node = inner;
                }
            }
            let ignored = self.dim && node.ignored;
            if !node.is_folder() {
                self.rows.push(Row {
                    path: full,
                    name,
                    depth,
                    kind: RowKind::File,
                    ignored,
                });
                continue;
            }
            let is_open = !node.sealed && (self.open_all || self.open.contains(&full));
            self.rows.push(Row {
                path: full.clone(),
                name,
                depth,
                kind: RowKind::Folder {
                    open: is_open,
                    openable: !node.sealed,
                },
                ignored,
            });
            if is_open {
                self.folder(node, &full, depth + 1);
            }
        }
    }
}

/// Every folder above `path`, from the top down, so opening them all shows it.
pub(crate) fn ancestors(path: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    let mut above: Vec<_> = path
        .ancestors()
        .skip(1)
        .filter(|folder| !folder.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    above.reverse();
    above.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> WorktreeFiles {
        WorktreeFiles {
            files: paths.iter().map(PathBuf::from).collect(),
            ..WorktreeFiles::default()
        }
    }

    fn options() -> TreeOptions {
        TreeOptions {
            dotfiles: true,
            ignored: IgnoredFiles::Dimmed,
            fold: true,
            sort: Sort::FoldersFirst,
        }
    }

    fn names(rows: &[Row]) -> Vec<(usize, &str)> {
        rows.iter()
            .map(|row| (row.depth, row.name.as_str()))
            .collect()
    }

    fn opened(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn folders_come_first_and_closed_folders_hide_their_contents() {
        let files = files(&[
            "README.md",
            "src/lib.rs",
            "src/a.rs",
            "docs/b.md",
            "Cargo.toml",
        ]);
        let rows = rows(&files, &opened(&["src"]), "", &options());
        assert_eq!(
            names(&rows),
            [
                (0, "docs"),
                (0, "src"),
                (1, "a.rs"),
                (1, "lib.rs"),
                (0, "Cargo.toml"),
                (0, "README.md"),
            ]
        );
    }

    #[test]
    fn mixed_sorts_folders_among_files() {
        let files = files(&["b.rs", "a/x.rs", "c.rs"]);
        let options = TreeOptions {
            sort: Sort::Mixed,
            ..options()
        };
        assert_eq!(
            names(&rows(&files, &HashSet::new(), "", &options)),
            [(0, "a"), (0, "b.rs"), (0, "c.rs")]
        );
    }

    #[test]
    fn a_chain_of_single_child_folders_folds_into_one_row() {
        let files = files(&["crates/ui/src/lib.rs", "crates/ui/Cargo.toml"]);
        let rows = rows(&files, &opened(&["crates/ui"]), "", &options());
        assert_eq!(
            names(&rows),
            [(0, "crates/ui"), (1, "src"), (1, "Cargo.toml")]
        );
        assert_eq!(rows[0].path, Path::new("crates/ui"));

        let unfolded = TreeOptions {
            fold: false,
            ..options()
        };
        let rows = super::rows(&files, &opened(&["crates"]), "", &unfolded);
        assert_eq!(names(&rows), [(0, "crates"), (1, "ui")]);
    }

    #[test]
    fn a_filter_keeps_matches_and_opens_the_folders_above_them() {
        let files = files(&["src/palette.rs", "src/root.rs", "docs/palette.md"]);
        let rows = rows(&files, &HashSet::new(), "PALETTE", &options());
        assert_eq!(
            names(&rows),
            [
                (0, "docs"),
                (1, "palette.md"),
                (0, "src"),
                (1, "palette.rs")
            ]
        );
    }

    #[test]
    fn dotfiles_and_ignored_entries_follow_their_settings() {
        let files = WorktreeFiles {
            files: vec![".gitignore".into(), "src/lib.rs".into()],
            ignored_files: vec!["notes.log".into()],
            ignored_folders: vec!["target".into()],
        };
        let rows = rows(&files, &HashSet::new(), "", &options());
        assert_eq!(
            names(&rows),
            [
                (0, "src"),
                (0, "target"),
                (0, ".gitignore"),
                (0, "notes.log")
            ]
        );
        let target = &rows[1];
        assert!(target.ignored);
        assert_eq!(
            target.kind,
            RowKind::Folder {
                open: false,
                openable: false
            }
        );

        let hidden = TreeOptions {
            dotfiles: false,
            ignored: IgnoredFiles::Hidden,
            ..options()
        };
        assert_eq!(
            names(&super::rows(&files, &HashSet::new(), "", &hidden)),
            [(0, "src")]
        );
    }

    #[test]
    fn ancestors_run_from_the_top_down() {
        let above: Vec<_> = ancestors(Path::new("a/b/c.rs")).collect();
        assert_eq!(above, [PathBuf::from("a"), PathBuf::from("a/b")]);
    }
}
