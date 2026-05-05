use gpui::*;
use uuid::Uuid;

use pgblade_editor::{SqlEditor, SqlEditorEvent};

use crate::panes::tab_bar::{TabBar, TabBarEvent, TabInfo};

struct QueryTab {
    id: Uuid,
    editor: Entity<SqlEditor>,
}

pub struct EditorArea {
    tabs: Vec<QueryTab>,
    active_tab: usize,
    tab_bar: Entity<TabBar>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorEvent {
    TabChanged(Uuid),
    Execute(String),
}

impl EventEmitter<EditorEvent> for EditorArea {}

impl EditorArea {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tab_bar = cx.new(|_| TabBar::new());
        cx.subscribe(&tab_bar, Self::handle_tab_event).detach();

        let first_tab = Self::create_tab(cx);
        let area = Self {
            tabs: vec![first_tab],
            active_tab: 0,
            tab_bar,
        };
        area.sync_tab_bar(cx);
        area
    }

    fn create_tab(cx: &mut Context<Self>) -> QueryTab {
        let editor = cx.new(SqlEditor::new);
        cx.subscribe(
            &editor,
            |this, _editor, event: &SqlEditorEvent, cx| match event {
                SqlEditorEvent::Execute(sql) => {
                    cx.emit(EditorEvent::Execute(sql.clone()));
                }
                SqlEditorEvent::Changed => {
                    this.sync_tab_bar(cx);
                }
            },
        )
        .detach();
        QueryTab {
            id: Uuid::new_v4(),
            editor,
        }
    }

    pub fn text(&self, cx: &App) -> String {
        self.tabs[self.active_tab].editor.read(cx).text()
    }

    pub fn active_tab_id(&self) -> Uuid {
        self.tabs[self.active_tab].id
    }

    #[allow(dead_code)]
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.tabs[self.active_tab].editor.read(cx).focus(window);
    }

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let tab = Self::create_tab(cx);
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.sync_tab_bar(cx);
        cx.emit(EditorEvent::TabChanged(self.tabs[self.active_tab].id));
        cx.notify();
    }

    pub fn close_tab(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return; // Don't close the last tab
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.sync_tab_bar(cx);
            cx.emit(EditorEvent::TabChanged(self.tabs[self.active_tab].id));
            cx.notify();
        }
    }

    fn select_tab(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab = idx;
            self.sync_tab_bar(cx);
            cx.emit(EditorEvent::TabChanged(id));
            cx.notify();
        }
    }

    fn sync_tab_bar(&self, cx: &mut Context<Self>) {
        let infos: Vec<TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let label = tab
                    .editor
                    .read(cx)
                    .text()
                    .lines()
                    .next()
                    .unwrap_or("Untitled")
                    .to_string();
                let label = if label.is_empty() || label.starts_with("--") {
                    "Untitled".to_string()
                } else if label.len() > 20 {
                    format!("{}...", &label[..20])
                } else {
                    label
                };
                TabInfo {
                    id: tab.id,
                    label,
                    is_active: i == self.active_tab,
                }
            })
            .collect();

        self.tab_bar.update(cx, |bar, cx| bar.set_tabs(infos, cx));
    }

    fn handle_tab_event(
        &mut self,
        _bar: Entity<TabBar>,
        event: &TabBarEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TabBarEvent::Select(id) => self.select_tab(*id, cx),
            TabBarEvent::Close(id) => self.close_tab(*id, cx),
            TabBarEvent::New => self.new_tab(cx),
        }
    }
}

impl Render for EditorArea {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active_editor = self.tabs[self.active_tab].editor.clone();

        div()
            .id("editor-area")
            .size_full()
            .flex()
            .flex_col()
            .child(self.tab_bar.clone())
            .child(div().flex_1().child(active_editor))
    }
}
