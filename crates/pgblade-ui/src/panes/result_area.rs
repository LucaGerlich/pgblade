use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::error::QueryError;
use pgblade_core::result::{CellValue, ColumnMeta};

/// The result grid displays query results or error messages.
pub struct ResultArea {
    state: ResultState,
}

enum ResultState {
    Empty,
    Loading,
    Success {
        columns: Vec<ColumnMeta>,
        rows: Vec<Vec<CellValue>>,
        duration_ms: i64,
    },
    Error(QueryError),
}

impl ResultArea {
    pub fn new() -> Self {
        Self {
            state: ResultState::Empty,
        }
    }

    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.state = ResultState::Loading;
        cx.notify();
    }

    pub fn set_results(
        &mut self,
        columns: Vec<ColumnMeta>,
        rows: Vec<Vec<CellValue>>,
        duration_ms: i64,
        cx: &mut Context<Self>,
    ) {
        self.state = ResultState::Success {
            columns,
            rows,
            duration_ms,
        };
        cx.notify();
    }

    pub fn set_error(&mut self, error: QueryError, cx: &mut Context<Self>) {
        self.state = ResultState::Error(error);
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.state = ResultState::Empty;
        cx.notify();
    }

    fn render_empty(&self) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(0x555555))
            .text_sm()
            .child("Run a query to see results")
    }

    fn render_loading(&self) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(0x4fc1ff))
            .text_sm()
            .child("Executing...")
    }

    fn render_error(&self, error: &QueryError) -> impl IntoElement {
        let (title, detail) = match error {
            QueryError::Postgres {
                code,
                severity,
                message,
                hint,
                ..
            } => {
                let title = format!("{severity} [{code}]: {message}");
                let detail = hint.clone().unwrap_or_default();
                (title, detail)
            }
            QueryError::Cancelled => ("Query cancelled".to_string(), String::new()),
            QueryError::NotConnected => ("Not connected to a database".to_string(), String::new()),
            QueryError::ReadOnlyViolation => (
                "Write blocked: connection is in read-only mode".to_string(),
                String::new(),
            ),
            QueryError::Other { message } => (message.clone(), String::new()),
        };

        div()
            .size_full()
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xf14c4c))
                    .child(title),
            )
            .when(!detail.is_empty(), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xcccccc))
                        .child(format!("Hint: {detail}")),
                )
            })
    }

    fn render_results(
        &self,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        duration_ms: i64,
    ) -> impl IntoElement {
        let row_count = rows.len();

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Column headers
            .child(self.render_header(columns))
            // Data rows
            .child(
                div().flex_1().overflow_hidden().children(
                    rows.iter()
                        .enumerate()
                        .map(|(i, row)| self.render_row(i, row, columns)),
                ),
            )
            // Footer
            .child(
                div()
                    .h(px(24.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(0x333333))
                    .bg(rgb(0x1e1e1e))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x888888))
                            .child(if row_count == 1 {
                                format!("1 row in {duration_ms}ms")
                            } else {
                                format!("{row_count} rows in {duration_ms}ms")
                            }),
                    ),
            )
    }

    fn render_header(&self, columns: &[ColumnMeta]) -> impl IntoElement {
        div()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .bg(rgb(0x252525))
            .border_b_1()
            .border_color(rgb(0x333333))
            .children(columns.iter().map(|col| {
                div()
                    .w(px(150.0))
                    .px_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xaaaaaa))
                    .overflow_hidden()
                    .child(col.name.clone())
            }))
    }

    fn render_row(
        &self,
        idx: usize,
        row: &[CellValue],
        _columns: &[ColumnMeta],
    ) -> impl IntoElement {
        let bg = if idx.is_multiple_of(2) {
            rgb(0x1a1a1a)
        } else {
            rgb(0x1e1e1e)
        };

        div()
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .bg(bg)
            .children(row.iter().map(|cell| {
                let (text, color) = match cell {
                    CellValue::Null => ("NULL".to_string(), rgb(0x555555)),
                    other => (other.display(), rgb(0xcccccc)),
                };

                div()
                    .w(px(150.0))
                    .px_2()
                    .text_xs()
                    .text_color(color)
                    .overflow_hidden()
                    .child(text)
            }))
    }
}

impl Render for ResultArea {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.state {
            ResultState::Empty => self.render_empty().into_any_element(),
            ResultState::Loading => self.render_loading().into_any_element(),
            ResultState::Error(error) => self.render_error(error).into_any_element(),
            ResultState::Success {
                columns,
                rows,
                duration_ms,
            } => self
                .render_results(columns, rows, *duration_ms)
                .into_any_element(),
        };

        div()
            .id("result-area")
            .size_full()
            .bg(rgb(0x1a1a1a))
            .child(content)
    }
}
