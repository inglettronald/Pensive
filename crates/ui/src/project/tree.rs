//! Folds changed files' paths into a tree, merging each chain of single-child folders into one row.

use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TreeRow {
    /// A folder, or a chain of folders joined with `/`.
    Folder { depth: usize, name: String },
    /// The file at this index of the input.
    File { depth: usize, index: usize },
}

#[derive(Default)]
struct Folder {
    /// Folders and files in the order they first appear.
    children: Vec<Child>,
}

enum Child {
    Folder(String, Folder),
    File(usize),
}

/// The rows for `paths`, keeping the order in which folders and files first appear.
pub(crate) fn tree<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Vec<TreeRow> {
    let mut root = Folder::default();
    for (index, path) in paths.into_iter().enumerate() {
        let folders: Vec<_> = path
            .parent()
            .into_iter()
            .flat_map(Path::components)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        root.insert(&folders, index);
    }
    let mut rows = Vec::new();
    root.flatten(0, &mut rows);
    rows
}

impl Folder {
    fn insert(&mut self, folders: &[String], index: usize) {
        let Some((first, rest)) = folders.split_first() else {
            self.children.push(Child::File(index));
            return;
        };
        let existing = self.children.iter_mut().find_map(|child| match child {
            Child::Folder(name, folder) if name == first => Some(folder),
            _ => None,
        });
        match existing {
            Some(folder) => folder.insert(rest, index),
            None => {
                let mut folder = Folder::default();
                folder.insert(rest, index);
                self.children.push(Child::Folder(first.clone(), folder));
            }
        }
    }

    fn flatten(&self, depth: usize, rows: &mut Vec<TreeRow>) {
        for child in &self.children {
            match child {
                Child::File(index) => rows.push(TreeRow::File {
                    depth,
                    index: *index,
                }),
                Child::Folder(name, folder) => {
                    let mut name = name.clone();
                    let mut folder = folder;
                    while let [Child::Folder(next, inner)] = folder.children.as_slice() {
                        name = format!("{name}/{next}");
                        folder = inner;
                    }
                    rows.push(TreeRow::Folder { depth, name });
                    folder.flatten(depth + 1, rows);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn rows(paths: &[&str]) -> Vec<TreeRow> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        tree(paths.iter().map(PathBuf::as_path))
    }

    fn folder(depth: usize, name: &str) -> TreeRow {
        TreeRow::Folder {
            depth,
            name: name.into(),
        }
    }

    fn file(depth: usize, index: usize) -> TreeRow {
        TreeRow::File { depth, index }
    }

    #[test]
    fn single_child_folders_merge() {
        assert_eq!(
            rows(&["crates/ui/src/root.rs"]),
            vec![folder(0, "crates/ui/src"), file(1, 0)]
        );
    }

    #[test]
    fn a_folder_with_siblings_keeps_its_own_row() {
        assert_eq!(
            rows(&[
                "crates/ui/src/root.rs",
                "crates/theme/src/colors.rs",
                "docs/design.md"
            ]),
            vec![
                folder(0, "crates"),
                folder(1, "ui/src"),
                file(2, 0),
                folder(1, "theme/src"),
                file(2, 1),
                folder(0, "docs"),
                file(1, 2),
            ]
        );
    }

    #[test]
    fn files_and_folders_keep_the_order_they_first_appear_in() {
        assert_eq!(
            rows(&["src/palette.rs", "src/palette/row.rs", "README.md"]),
            vec![
                folder(0, "src"),
                file(1, 0),
                folder(1, "palette"),
                file(2, 1),
                file(0, 2),
            ]
        );
    }
}
