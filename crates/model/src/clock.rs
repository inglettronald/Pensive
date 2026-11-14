//! The wall clock, read the same way on the desktop and in a browser.

use std::time::SystemTime;

/// The time now. `SystemTime::now` panics in a browser, so there it reads the browser's clock.
#[allow(
    clippy::disallowed_methods,
    reason = "this is the clock the rest of model reads"
)]
pub(crate) fn now() -> SystemTime {
    #[cfg(target_family = "wasm")]
    {
        let since = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default();
        SystemTime::UNIX_EPOCH + since
    }
    #[cfg(not(target_family = "wasm"))]
    SystemTime::now()
}
