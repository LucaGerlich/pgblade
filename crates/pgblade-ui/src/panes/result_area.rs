use gpui::prelude::FluentBuilder;
use gpui::*;

use pgblade_core::error::QueryError;
use pgblade_core::result::{CellValue, ColumnMeta};

/// The result grid displays query results or error messages.
pub struct ResultArea {
    state: ResultState,
    selected_row: Option<usize>,
    focus_handle: FocusHandle,
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
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: ResultState::Empty,
            selected_row: None,
            focus_handle: cx.focus_handle(),
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
        self.selected_row = None;
        cx.notify();
    }

    pub fn set_error(&mut self, error: QueryError, cx: &mut Context<Self>) {
        self.state = ResultState::Error(error);
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.state = ResultState::Empty;
        self.selected_row = None;
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key.as_str() == "c"
            && event.keystroke.modifiers.platform
            && let Some(row_idx) = self.selected_row
            && let ResultState::Success { rows, .. } = &self.state
            && let Some(row) = rows.get(row_idx)
        {
            let tsv = row
                .iter()
                .map(|cell| cell.display())
                .collect::<Vec<_>>()
                .join("\t");
            cx.write_to_clipboard(ClipboardItem::new_string(tsv));
        }
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
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row_count = rows.len();
        let rows_for_list = rows.to_vec();
        let selected_row = self.selected_row;
        let entity = cx.entity().clone();
        let focus_handle = self.focus_handle.clone();
        let col_widths = calculate_column_widths(columns, rows);

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Column headers
            .child(self.render_header(columns, &col_widths))
            // Data rows via uniform_list for virtualized scrolling
            .child(
                div().flex_1().child(
                    uniform_list("result-rows", row_count, {
                        let columns_for_list = columns.to_vec();
                        let widths = col_widths.clone();
                        move |range, _window, _cx| {
                            rows_for_list[range.clone()]
                                .iter()
                                .enumerate()
                                .map(|(local_idx, row)| {
                                    let idx = range.start + local_idx;
                                    let entity_for_click = entity.clone();
                                    let focus_for_click = focus_handle.clone();
                                    render_data_row(
                                        idx,
                                        row,
                                        &columns_for_list,
                                        selected_row,
                                        entity_for_click,
                                        focus_for_click,
                                        &widths,
                                    )
                                })
                                .collect()
                        }
                    })
                    .flex_1(),
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
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .text_xs()
                            .text_color(rgb(0x555555))
                            .child(
                                selected_row
                                    .map(|r| {
                                        format!("Row {} selected \u{2014} Cmd+C to copy", r + 1)
                                    })
                                    .unwrap_or_default(),
                            ),
                    ),
            )
    }

    fn render_header(&self, columns: &[ColumnMeta], col_widths: &[f32]) -> impl IntoElement {
        div()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .bg(rgb(0x252525))
            .border_b_1()
            .border_color(rgb(0x333333))
            // Row number column header (empty)
            .child(
                div()
                    .w(px(ROW_NUMBER_WIDTH))
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x555555))
                    .child("#"),
            )
            .children(columns.iter().enumerate().map(|(i, col)| {
                let width = col_widths.get(i).copied().unwrap_or(DEFAULT_COL_WIDTH);
                div()
                    .w(px(width))
                    .px_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xaaaaaa))
                    .overflow_hidden()
                    .child(col.name.clone())
            }))
    }
}

/// Width of the row number gutter column.
const ROW_NUMBER_WIDTH: f32 = 48.0;

/// Default column width when auto-sizing information is unavailable.
const DEFAULT_COL_WIDTH: f32 = 150.0;

/// Calculate column widths based on header names and cell content.
///
/// Samples up to the first 50 rows to determine the widest content
/// per column, then clamps the result between `min_width` and `max_width`.
fn calculate_column_widths(columns: &[ColumnMeta], rows: &[Vec<CellValue>]) -> Vec<f32> {
    let char_width: f32 = 7.5; // approximate monospace char width at text_xs
    let min_width: f32 = 60.0;
    let max_width: f32 = 300.0;
    let padding: f32 = 16.0;

    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let header_len = col.name.len();
            let max_data_len = rows
                .iter()
                .take(50)
                .map(|row| row.get(i).map(|v| v.display().len()).unwrap_or(0))
                .max()
                .unwrap_or(0);

            let char_count = header_len.max(max_data_len).max(4);
            (char_count as f32 * char_width + padding).clamp(min_width, max_width)
        })
        .collect()
}

/// Renders a single data row for the result grid.
///
/// Extracted as a free function so it can be used inside the
/// `uniform_list` closure which cannot capture `&self`.
fn render_data_row(
    idx: usize,
    row: &[CellValue],
    _columns: &[ColumnMeta],
    selected_row: Option<usize>,
    entity: Entity<ResultArea>,
    focus_handle: FocusHandle,
    col_widths: &[f32],
) -> Stateful<Div> {
    let bg = if Some(idx) == selected_row {
        rgb(0x264f78) // VS Code selection blue
    } else if idx.is_multiple_of(2) {
        rgb(0x1a1a1a)
    } else {
        rgb(0x1e1e1e)
    };

    div()
        .id(SharedString::from(format!("row-{idx}")))
        .h(px(24.0))
        .flex()
        .flex_row()
        .items_center()
        .px_2()
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x2a2a2a)))
        .on_click(move |_, window, cx| {
            focus_handle.focus(window);
            entity.update(cx, |this, cx| {
                this.selected_row = Some(idx);
                cx.notify();
            });
        })
        // Row number column
        .child(
            div()
                .w(px(ROW_NUMBER_WIDTH))
                .px_2()
                .text_xs()
                .text_color(rgb(0x555555))
                .flex()
                .justify_end()
                .child((idx + 1).to_string()),
        )
        // Data cells
        .children(row.iter().enumerate().map(|(i, cell)| {
            let width = col_widths.get(i).copied().unwrap_or(DEFAULT_COL_WIDTH);

            match cell {
                CellValue::Null => div()
                    .w(px(width))
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .italic()
                    .overflow_hidden()
                    .child("NULL"),
                other => div()
                    .w(px(width))
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0xcccccc))
                    .overflow_hidden()
                    .child(other.display()),
            }
        }))
}

impl Render for ResultArea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.state {
            ResultState::Empty => self.render_empty().into_any_element(),
            ResultState::Loading => self.render_loading().into_any_element(),
            ResultState::Error(error) => self.render_error(error).into_any_element(),
            ResultState::Success {
                columns,
                rows,
                duration_ms,
            } => self
                .render_results(columns, rows, *duration_ms, cx)
                .into_any_element(),
        };

        div()
            .id("result-area")
            .size_full()
            .bg(rgb(0x1a1a1a))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(content)
    }
}
