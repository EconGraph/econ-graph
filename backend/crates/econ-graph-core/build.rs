fn main() {
    // embed_migrations! reads these directories during macro expansion. Cargo cannot
    // otherwise see newly added migrations when reusing a compiled core crate.
    println!("cargo:rerun-if-changed=../../migrations/app");
    println!("cargo:rerun-if-changed=../../migrations/data");
}
