fn main() {
    // Embed the app icon and version info into the Windows executable (Explorer, taskbar, shortcuts).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("driftlab.rc", embed_resource::NONE).manifest_optional().ok();
    }
    println!("cargo:rerun-if-changed=driftlab.rc");
    println!("cargo:rerun-if-changed=../../assets/icons/driftlab.ico");
}
