//! Embeds the app icon in the Windows exe, where GPUI loads it as every window's icon.

use embed_resource::{CompilationResult, ParamsIncludeDirs};

const RESOURCES: &str = "resources/windows";

fn main() {
    println!("cargo::rerun-if-changed={RESOURCES}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let rc = format!("{RESOURCES}/pensive.rc");
    match embed_resource::compile(rc, ParamsIncludeDirs([RESOURCES])) {
        CompilationResult::Ok => {}
        problem => panic!("couldn't embed the window icon: {problem}"),
    }
}
