//! The Windows executable's version information, icon and manifest
//! (rea-launcher-build): a named, versioned program with the launcher's
//! icon instead of an anonymous one that antivirus heuristics distrust.
fn main() {
    rea_launcher_build::Resources::new(
        "Nintendo Arcade Launcher",
        "One launcher for F-Zero AX, Mario Kart Arcade GP and Luigi's Mansion Arcade",
    )
    .company("AkitaOnRails")
    .file_name("nintendo-launcher.exe")
    .icon("../../packaging/appimage/nintendo-launcher.png")
    .embed()
    .unwrap();
}
