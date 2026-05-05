pub(crate) mod editor_area;
mod result_area;
mod sidebar;
mod status_bar;
pub(crate) mod tab_bar;
mod toolbar;

pub use editor_area::EditorArea;
pub use result_area::ResultArea;
pub use sidebar::{SchemaSidebar, SidebarEvent};
pub use status_bar::StatusBar;
pub use toolbar::Toolbar;
