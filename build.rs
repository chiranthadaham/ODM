use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app.res");
    if let Some(zig) = env::var_os("ODM_ZIG") {
        let status = Command::new(zig)
            .args(["rc", "/i", "assets", "/fo"])
            .arg(&out)
            .arg("assets/app.rc")
            .status()
            .expect("run Zig resource compiler");
        assert!(status.success(), "resource compilation failed");
    } else {
        let status = Command::new("rc.exe")
            .arg("/nologo")
            .arg("/iassets")
            .arg(format!("/fo{}", out.display()))
            .arg("assets/app.rc")
            .status()
            .expect("Install Windows SDK or set ODM_ZIG");
        assert!(status.success(), "resource compilation failed");
    }
    println!("cargo:rustc-link-arg={}", out.display());
}
