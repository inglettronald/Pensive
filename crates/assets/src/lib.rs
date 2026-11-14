//! Files bundled into the app binary, served to GPUI by their path under `assets/`.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! bundled {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../../../assets/", $path)).as_slice())),*]
    };
}

/// Pensive's logo, Lucide icons (ISC license, `assets/icons/LICENSE`) at the version named in
/// each file, and the window controls, drawn for Pensive.
const FILES: &[(&str, &[u8])] = bundled![
    "logo.svg",
    "icons/archive.svg",
    "icons/arrow-down.svg",
    "icons/arrow-down-to-line.svg",
    "icons/arrow-up-from-line.svg",
    "icons/arrow-up.svg",
    "icons/bell.svg",
    "icons/box.svg",
    "icons/braces.svg",
    "icons/check.svg",
    "icons/chevron-down.svg",
    "icons/chevron-right.svg",
    "icons/chevron-up.svg",
    "icons/chevrons-up-down.svg",
    "icons/circle-help-filled.svg",
    "icons/circle-x.svg",
    "icons/cloud.svg",
    "icons/command.svg",
    "icons/download.svg",
    "icons/ellipsis.svg",
    "icons/file-cog.svg",
    "icons/file-diff.svg",
    "icons/file-text.svg",
    "icons/files.svg",
    "icons/fold-vertical.svg",
    "icons/folder.svg",
    "icons/folder-git-2.svg",
    "icons/git-branch.svg",
    "icons/git-commit-horizontal.svg",
    "icons/git-graph.svg",
    "icons/git-merge.svg",
    "icons/hash.svg",
    "icons/list.svg",
    "icons/list-tree.svg",
    "icons/minus.svg",
    "icons/palette.svg",
    "icons/panel-left.svg",
    "icons/play.svg",
    "icons/plus.svg",
    "icons/refresh-cw.svg",
    "icons/rotate-ccw.svg",
    "icons/scroll-text.svg",
    "icons/search-x.svg",
    "icons/search.svg",
    "icons/settings.svg",
    "icons/square.svg",
    "icons/triangle-alert.svg",
    "icons/window-close.svg",
    "icons/window-maximize.svg",
    "icons/window-minimize.svg",
    "icons/x.svg",
];

/// Serves the bundled files. The app and the mock renderer both install it, so `svg()` can load
/// icons by path.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}
