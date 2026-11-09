//! Syphon.framework (shipped by `syphon-core`) is linked as `@rpath/Syphon.framework`, so
//! every binary that links it needs an rpath to it. Cargo only gives the framework's
//! location to direct dependents, so it is set here for this crate's tests and passed on
//! to openvj-app as `DEP_OPENVJ_MEDIA_SYPHON_FRAMEWORK_DIR`.
fn main() {
    println!("cargo:rerun-if-env-changed=DEP_SYPHON_FRAMEWORK_DIR");
    if let Ok(dir) = std::env::var("DEP_SYPHON_FRAMEWORK_DIR") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
        println!("cargo:syphon_framework_dir={dir}");
    }
}
