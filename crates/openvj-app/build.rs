fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint UI failed to compile");

    // Find Syphon.framework when running from `target/` (see openvj-media's build.rs), or
    // in Contents/Frameworks once packaged as an app.
    println!("cargo:rerun-if-env-changed=DEP_OPENVJ_MEDIA_SYPHON_FRAMEWORK_DIR");
    if let Ok(dir) = std::env::var("DEP_OPENVJ_MEDIA_SYPHON_FRAMEWORK_DIR") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{dir}");
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path/../Frameworks");
    }
}
