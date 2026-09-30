// Build-time feature flags: cfg(flag_mcp) for each `build` flag on in FLAGS_PROFILE.
fn main() {
    econ_graph_flags_build::emit();
}
