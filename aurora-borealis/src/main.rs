fn main() {
    println!("The Aurora BorealiuS Toolset v{}", env!("CARGO_PKG_VERSION"));
    println!("Starting...");
    #[cfg(not(target_arch = "wasm32"))]
    aurora_borealis::run_native();
}
