//! Reads a demo's options from the command line, such as `--mock project/unsaved`.

use gpui::{Pixels, Size, px, size};

use super::Options;

const USAGE: &str = "usage: cargo demo -- [--mock <subject>/<mock>] [--size <width>x<height>]";
pub const LISTING_MOCKS: &str = "Ctrl Shift M in the demo lists them.";

pub fn given() -> Result<Options, String> {
    parse(std::env::args().skip(1))
}

/// Exits with code 2, as a command does when it's run wrong.
pub fn stop(problem: &str) -> ! {
    eprintln!("{problem}");
    std::process::exit(2)
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
