//! Prints the fine-grained role catalog, one `resource:action` name per line.
//!
//! For the CI check (AUTH-4) that compares the catalog with the client roles in the Keycloak
//! realm export, as sorted sets:
//! `cargo run -p econ-graph-auth --bin econ-graph-roles -- --list`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--list" => {
            print!("{}", econ_graph_auth::role_list());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: econ-graph-roles --list");
            ExitCode::from(2)
        }
    }
}
