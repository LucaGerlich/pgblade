pub mod actions;
pub mod components;
pub mod controller;
pub mod modals;
mod panes;
pub mod workspace;

pub use controller::AppController;
pub use workspace::Workspace;

use gpui::*;

/// Register all PgBlade actions and key bindings.
/// Must be called during app initialization.
pub fn init(cx: &mut App) {
    cx.on_action::<actions::Quit>(|_, cx| {
        cx.quit();
    });

    // Register key bindings
    cx.bind_keys([
        KeyBinding::new("cmd-enter", actions::ExecuteQuery, Some("Workspace")),
        KeyBinding::new("cmd-n", actions::NewConnection, Some("Workspace")),
        KeyBinding::new("cmd-b", actions::ToggleSidebar, Some("Workspace")),
        KeyBinding::new("cmd-k", actions::ToggleCommandPalette, Some("Workspace")),
        KeyBinding::new("cmd-t", actions::NewTab, Some("Workspace")),
        KeyBinding::new("cmd-w", actions::CloseTab, Some("Workspace")),
    ]);
}

/// Configure the main window options.
pub fn window_options() -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("PgBlade".into()),
            ..Default::default()
        }),
        focus: true,
        window_min_size: Some(size(px(800.0), px(600.0))),
        ..Default::default()
    }
}
