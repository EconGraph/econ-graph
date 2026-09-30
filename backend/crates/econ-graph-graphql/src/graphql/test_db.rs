//! Shared helpers for this crate's DB-backed tests.

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// Run migrations once per test binary. `run_migrations` takes no lock, so two test modules
/// migrating a fresh database at the same time would race on "relation already exists".
pub(crate) async fn migrate_once(url: &str) {
    MIGRATED
        .get_or_init(|| async {
            econ_graph_core::database::run_migrations(url)
                .await
                .expect("migrations");
        })
        .await;
}
