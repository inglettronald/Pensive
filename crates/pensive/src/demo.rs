//! The options `cargo demo` takes: the mock to start on, and the window's size.

use gpui::{Pixels, Size, px, size};

const USAGE: &str = "usage: cargo demo -- [--mock <subject>/<mock>] [--size <width>x<height>]";

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// A mock's id, such as `project/unsaved`, to open in place of the landing screen.
    pub mock: Option<String>,
    pub size: Option<Size<Pixels>>,
}

impl Options {
    /// Reads the command line, or says what's wrong with it and how to write it.
    pub fn from_args() -> Result<Self, String> {
        let options = parse(std::env::args().skip(1))?;
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

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--mock" => options.mock = Some(value()?),
            "--size" => options.size = Some(parse_size(&value()?)?),
            _ => return Err(format!("unknown option `{arg}`\n{USAGE}")),
        }
    }
    Ok(options)
}

/// `1280x800` as a size in pixels.
fn parse_size(text: &str) -> Result<Size<Pixels>, String> {
    let (width, height) = text
        .split_once('x')
        .and_then(|(width, height)| Some((width.parse::<f32>().ok()?, height.parse::<f32>().ok()?)))
        .filter(|(width, height)| *width > 0. && *height > 0.)
        .ok_or_else(|| format!("`{text}` isn't a size such as 1280x800\n{USAGE}"))?;
    Ok(size(px(width), px(height)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Result<Options, String> {
        parse(text.split_whitespace().map(String::from))
    }

    #[test]
    fn reads_a_mock_and_a_size_in_any_order() {
        let expected = Options {
            mock: Some("project/unsaved".into()),
            size: Some(size(px(1280.), px(800.))),
        };
        assert_eq!(args("--mock project/unsaved --size 1280x800"), Ok(expected));
        assert_eq!(args(""), Ok(Options::default()));
    }

    #[test]
    fn says_what_is_wrong_with_the_options() {
        assert!(
            args("--size 1280")
                .unwrap_err()
                .starts_with("`1280` isn't a size")
        );
        assert!(
            args("--mock")
                .unwrap_err()
                .starts_with("--mock needs a value")
        );
        assert!(
            args("--theme light")
                .unwrap_err()
                .starts_with("unknown option")
        );
    }
}
