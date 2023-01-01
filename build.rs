fn main() {
    let deps = system_deps::Config::new().probe().unwrap();
    for p in deps.all_link_paths() {
        println!("cargo:rustc-link-arg=-Wl,-rpath={}", p.display());
    }
}
