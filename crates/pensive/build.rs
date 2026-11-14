//! Embeds the app icon in the Windows exe, where GPUI loads it as every window's icon.

use std::{env, fs, path::PathBuf};

use embed_resource::CompilationResult;
use image::{
    ExtendedColorType,
    codecs::ico::{IcoEncoder, IcoFrame},
};
use resvg::{tiny_skia, usvg};

const ICON: &str = "../../assets/icon.svg";
/// From the title bar's 16 px up to the largest size Explorer shows.
const SIZES: [u32; 5] = [16, 24, 32, 48, 256];

fn main() {
    println!("cargo::rerun-if-changed={ICON}");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let svg = fs::read(ICON).expect("assets/icon.svg exists");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("icon.svg parses");

    let frames: Vec<IcoFrame> = SIZES
        .iter()
        .map(|&size| {
            let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("the size is not zero");
            let scale = size as f32 / tree.size().width();
            resvg::render(
                &tree,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            let png = pixmap.encode_png().expect("the pixmap encodes");
            IcoFrame::with_encoded(png, size, size, ExtendedColorType::Rgba8)
                .expect("each size fits an icon")
        })
        .collect();
    let ico = out_dir.join("pensive.ico");
    IcoEncoder::new(fs::File::create(&ico).expect("OUT_DIR is writable"))
        .encode_images(&frames)
        .expect("the frames encode");

    // GPUI loads icon resource 1.
    let rc = out_dir.join("pensive.rc");
    let ico_path = ico.display().to_string().replace('\\', "\\\\");
    fs::write(&rc, format!("1 ICON \"{ico_path}\"\n")).expect("OUT_DIR is writable");
    match embed_resource::compile(&rc, embed_resource::NONE) {
        CompilationResult::Ok => {}
        problem => panic!("couldn't embed the window icon: {problem}"),
    }
}
