fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/macos/mic.m");
        cc::Build::new()
            .file("src/macos/mic.m")
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .compile("diddle_macos");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    tauri_build::build()
}
