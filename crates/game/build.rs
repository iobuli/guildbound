fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // The default MSVC executable reserves only 1 MiB for the main thread.
        // This beta reproduced a stack overflow on visible startup. Give the
        // ECS/UI/render call paths 8 MiB; Windows commits pages as needed.
        println!("cargo::rustc-link-arg-bin=guildbound=/STACK:8388608");
    }
}
