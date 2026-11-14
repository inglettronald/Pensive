//! Renders the app icon to the PNG that `app_icon` bundles.

#![allow(
    clippy::disallowed_methods,
    reason = "a build script reads its input and writes to OUT_DIR"
)]

use std::{env, fs, path::Path};

use resvg::{tiny_skia, usvg};

const ICON: &str = "../../assets/icon.svg";
const SIZE: u32 = 256;

fn main() {
    println!("cargo::rerun-if-changed={ICON}");
    let svg = fs::read(ICON).expect("assets/icon.svg exists");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("icon.svg parses");
    let mut pixmap = tiny_skia::Pixmap::new(SIZE, SIZE).expect("the size is not zero");
    let scale = SIZE as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    pixmap
        .save_png(Path::new(&out_dir).join("icon.png"))
        .expect("OUT_DIR is writable");
}
