//! The options a demo starts with: the mock to open, and the window's size. `cargo demo` reads
//! them from its command line, and the browser build from its page's URL.

#[cfg(not(target_family = "wasm"))]
mod command_line;
#[cfg(not(target_family = "wasm"))]
use command_line as source;
#[cfg(target_family = "wasm")]
mod page_url;
#[cfg(target_family = "wasm")]
use page_url as source;

use gpui::{Pixels, Size};
pub use source::stop;

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// A mock's id, such as `project/unsaved`, to open in place of the landing screen.
    pub mock: Option<String>,
    /// Only the desktop sets it. A browser tab is the window, so the tab's size is the window's.
    pub size: Option<Size<Pixels>>,
}

impl Options {
    /// Reads the options, or says what's wrong with them and how to write them.
    pub fn read() -> Result<Self, String> {
        let options = source::given()?;
        if let Some(id) = &options.mock
            && !ui::every_mock().iter().any(|mock| mock.id() == *id)
        {
            return Err(format!(
                "no mock is called `{id}`. Ctrl Shift M in the demo lists them."
            ));
        }
        Ok(options)
    }
}
