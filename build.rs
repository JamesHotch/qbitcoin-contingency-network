fn main() {
    println!("cargo:rustc-check-cfg=cfg(enable_unaudited_mainnet)");
    println!("cargo:rerun-if-env-changed=ENABLE_UNAUDITED_MAINNET");

    if std::env::var("ENABLE_UNAUDITED_MAINNET").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=enable_unaudited_mainnet");
        println!(
            "cargo:warning=ENABLE_UNAUDITED_MAINNET=1: this build remains unaudited and unsafe for value"
        );
    }
}
