//! Every file in a worktree, as Git sees it: the ones it tracks or would add, and the ones
//! `.gitignore` leaves out.

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorktreeFiles {
    /// Every file on disk that Git tracks or would add, relative to the worktree's root. A
    /// tracked file deleted from disk isn't here.
    pub files: Vec<PathBuf>,
    /// Files that `.gitignore` leaves out.
    pub ignored_files: Vec<PathBuf>,
    /// Folders that `.gitignore` leaves out whole, such as `target`. Their contents aren't listed.
    pub ignored_folders: Vec<PathBuf>,
}
