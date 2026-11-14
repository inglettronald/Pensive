//! Reads a demo's options from the page's URL, such as `?mock=project/unsaved`.

use web_sys::UrlSearchParams;

use super::Options;

/// The `mock` parameter. Other parameters are ignored, as pages usually do.
pub fn given() -> Result<Options, String> {
    let search = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    let params = UrlSearchParams::new_with_str(&search)
        .map_err(|_| format!("couldn't read the parameters in `{search}`"))?;
    Ok(Options {
        mock: params.get("mock"),
        size: None,
    })
}

/// A tab can't exit, so this panics, and the panic's message shows in the console.
pub fn stop(problem: &str) -> ! {
    panic!("{problem}")
}
