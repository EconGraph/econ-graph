fn main() {
    // embed_migrations! reads these directories during macro expansion. Track SQL-only
    // changes and newly added migrations too, so restored Cargo caches cannot embed an
    // earlier migration set when no Rust source changed.
    println!("cargo:rerun-if-changed=../../migrations/app");
    println!("cargo:rerun-if-changed=../../migrations/data");
}
