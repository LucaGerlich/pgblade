use gpui::*;

#[derive(Debug, Clone)]
pub enum WriteConfirmEvent {
    Confirmed(String), // The SQL to execute
    Cancelled,
}

impl EventEmitter<WriteConfirmEvent> for WriteConfirmModal {}

pub struct WriteConfirmModal {
    sql: String,
    classification: String,
    focus_handle: FocusHandle,
}

impl WriteConfirmModal {
    pub fn new(sql: String, classification: String, cx: &mut Context<Self>) -> Self {
        Self {
            sql,
            classification,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for WriteConfirmModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sql_preview = if self.sql.len() > 100 {
            format!("{}...", &self.sql[..100])
        } else {
            self.sql.clone()
        };

        div()
            .id("write-confirm-backdrop")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa))
            .child(
                div()
                    .id("write-confirm-modal")
                    .track_focus(&self.focus_handle)
                    .w(px(450.0))
                    .p_6()
                    .bg(rgb(0x252525))
                    .border_1()
                    .border_color(rgb(0xf14c4c))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    // Warning header
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xf14c4c))
                            .mb_2()
                            .child(format!(
                                "{} on PRODUCTION",
                                self.classification.to_uppercase()
                            )),
                    )
                    .child(div().text_sm().text_color(rgb(0xcccccc)).mb_4().child(
                        "This query will modify data on a production database. \
                                 Are you sure?",
                    ))
                    // SQL preview
                    .child(
                        div()
                            .p_2()
                            .mb_4()
                            .bg(rgb(0x1a1a1a))
                            .border_1()
                            .border_color(rgb(0x333333))
                            .rounded_sm()
                            .text_xs()
                            .font_family("Monaco")
                            .text_color(rgb(0xaaaaaa))
                            .child(sql_preview),
                    )
                    // Buttons
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("write-cancel-btn")
                                    .px_4()
                                    .py_2()
                                    .bg(rgb(0x333333))
                                    .text_color(rgb(0xcccccc))
                                    .text_sm()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|_this, _, _window, cx| {
                                        cx.emit(WriteConfirmEvent::Cancelled);
                                    }))
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("write-confirm-btn")
                                    .px_4()
                                    .py_2()
                                    .bg(rgb(0xf14c4c))
                                    .text_color(rgb(0xffffff))
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        cx.emit(WriteConfirmEvent::Confirmed(this.sql.clone()));
                                    }))
                                    .child("Execute Anyway"),
                            ),
                    ),
            )
    }
}
