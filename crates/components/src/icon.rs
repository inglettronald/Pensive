//! A bundled icon, tinted like text.

use gpui::{App, Hsla, IntoElement, RenderOnce, SharedString, Styled, Window, svg};
use strum::{IntoStaticStr, VariantArray};
use theme::{ActiveTheme, Dp, dp};

/// A Lucide icon, named as Lucide names it, or a window control, which are drawn to match each
/// other in size where Lucide's minus, square and x don't. Each one is a file in `assets/icons`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantArray, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum IconName {
    Archive,
    ArrowDown,
    ArrowDownToLine,
    ArrowUp,
    ArrowUpFromLine,
    Bell,
    Box,
    Braces,
    Check,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    ChevronsUpDown,
    CircleHelpFilled,
    CircleX,
    Cloud,
    Command,
    Download,
    Ellipsis,
    FileCog,
    FileDiff,
    FileText,
    Files,
    FoldVertical,
    Folder,
    #[strum(serialize = "folder-git-2")]
    FolderGit2,
    GitBranch,
    GitCommitHorizontal,
    GitGraph,
    GitMerge,
    Hash,
    List,
    ListTree,
    Minus,
    Palette,
    PanelLeft,
    Play,
    Plus,
    RefreshCw,
    RotateCcw,
    ScrollText,
    Search,
    SearchX,
    Settings,
    Square,
    TriangleAlert,
    WindowClose,
    WindowMaximize,
    WindowMinimize,
    X,
}

impl IconName {
    pub fn path(self) -> SharedString {
        format!("icons/{}.svg", <&str>::from(self)).into()
    }
}

#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    size: Dp,
    color: Option<Hsla>,
}

impl Icon {
    /// A 16 px icon in `text_muted`.
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: dp(16.),
            color: None,
        }
    }

    pub fn size(mut self, size: Dp) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        svg()
            .path(self.name.path())
            .size(self.size)
            .flex_none()
            .text_color(self.color.unwrap_or(cx.theme().colors.text_muted))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("icon", "gallery", || {
            let named = |size: f32| {
                IconName::VARIANTS.iter().map(move |&name| {
                    (
                        <&str>::from(name),
                        Icon::new(name).size(dp(size)).into_any_element(),
                    )
                })
            };
            crate::Gallery::new("Icon")
                .section("16 px", named(16.))
                .section("20 px", named(20.))
                .into_any_element()
        })
        .size(960., 520.),
    ]
}

#[cfg(test)]
mod tests {
    use gpui::AssetSource;

    use super::*;

    #[test]
    fn every_icon_is_bundled() {
        for &name in IconName::VARIANTS {
            let file = assets::Assets.load(&name.path()).unwrap();
            assert!(file.is_some(), "{} is missing from assets", name.path());
        }
    }
}
