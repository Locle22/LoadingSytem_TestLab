fn main() {
    // Merge our sdkconfig.defaults into the ESP-IDF build
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let sdkconfig_defaults = std::path::Path::new(&manifest_dir).join("sdkconfig.defaults");
    if sdkconfig_defaults.exists() {
        println!(
            "cargo:rustc-env=ESP_IDF_SDKCONFIG_DEFAULTS={}",
            sdkconfig_defaults.display()
        );
        std::env::set_var("ESP_IDF_SDKCONFIG_DEFAULTS", sdkconfig_defaults.to_str().unwrap());
    }
    embuild::espidf::sysenv::output();
}
