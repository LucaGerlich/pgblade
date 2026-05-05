use std::time::Instant;

use gpui::{AppContext, Application};

fn main() {
    let launch_start = Instant::now();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pgblade=debug,gpui=warn".into()),
        )
        .init();

    tracing::info!("PgBlade starting");

    Application::new().run(move |cx| {
        pgblade_ui::init(cx);

        cx.open_window(pgblade_ui::window_options(), |_window, cx| {
            cx.new(pgblade_ui::Workspace::new)
        })
        .expect("failed to open main window");

        let startup_ms = launch_start.elapsed().as_millis();
        tracing::info!(startup_ms, "PgBlade window opened");
    });
}
