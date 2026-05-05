use gpui::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TabInfo {
    pub id: Uuid,
    pub label: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub enum TabBarEvent {
    Select(Uuid),
    Close(Uuid),
    New,
}

impl EventEmitter<TabBarEvent> for TabBar {}

pub struct TabBar {
    tabs: Vec<TabInfo>,
}

impl TabBar {
    pub fn new() -> Self {
        Self { tabs: Vec::new() }
    }

    pub fn set_tabs(&mut self, tabs: Vec<TabInfo>, cx: &mut Context<Self>) {
        self.tabs = tabs;
        cx.notify();
    }
}

impl Render for TabBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tab-bar")
            .h(px(32.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(rgb(0x1a1a1a))
            .border_b_1()
            .border_color(rgb(0x333333))
            .overflow_hidden()
            .children(self.tabs.iter().map(|tab| {
                let tab_id = tab.id;
                let is_active = tab.is_active;
                let bg = if is_active {
                    rgb(0x252525)
                } else {
                    rgb(0x1a1a1a)
                };
                let border_bottom = if is_active {
                    rgb(0x4fc1ff)
                } else {
                    rgb(0x1a1a1a)
                };
                let text_color = if is_active {
                    rgb(0xeeeeee)
                } else {
                    rgb(0x888888)
                };

                div()
                    .id(SharedString::from(format!("tab-{tab_id}")))
                    .h_full()
                    .px_3()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .bg(bg)
                    .border_b_2()
                    .border_color(border_bottom)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x2a2a2a)))
                    .on_click(cx.listener(move |_this, _, _window, cx| {
                        cx.emit(TabBarEvent::Select(tab_id));
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_color)
                            .max_w(px(120.0))
                            .overflow_hidden()
                            .child(tab.label.clone()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("close-{tab_id}")))
                            .text_xs()
                            .text_color(rgb(0x666666))
                            .hover(|s| s.text_color(rgb(0xeeeeee)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                cx.emit(TabBarEvent::Close(tab_id));
                            }))
                            .child("x"),
                    )
            }))
            // New tab button
            .child(
                div()
                    .id("new-tab-btn")
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .hover(|s| s.text_color(rgb(0xeeeeee)))
                    .cursor_pointer()
                    .on_click(cx.listener(|_this, _, _window, cx| {
                        cx.emit(TabBarEvent::New);
                    }))
                    .child("+"),
            )
    }
}
