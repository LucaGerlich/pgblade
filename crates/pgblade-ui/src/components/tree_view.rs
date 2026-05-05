use gpui::*;

/// A node in the tree. Each node has a label, optional icon, children, and expand state.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub icon: Option<&'static str>,
    pub children: Vec<TreeNode>,
    pub depth: usize,
}

/// Events emitted by the tree view.
#[derive(Debug, Clone)]
pub enum TreeViewEvent {
    /// A leaf node was clicked.
    NodeClicked(String),
}

pub struct TreeView {
    nodes: Vec<TreeNode>,
    expanded: std::collections::HashSet<String>,
}

impl EventEmitter<TreeViewEvent> for TreeView {}

impl Default for TreeView {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeView {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            expanded: std::collections::HashSet::new(),
        }
    }

    pub fn set_nodes(&mut self, nodes: Vec<TreeNode>, cx: &mut Context<Self>) {
        self.nodes = nodes;
        cx.notify();
    }

    fn toggle_expand(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.expanded.contains(id) {
            self.expanded.remove(id);
        } else {
            self.expanded.insert(id.to_string());
        }
        cx.notify();
    }

    fn render_node(&self, node: &TreeNode, cx: &mut Context<Self>) -> Div {
        let id = node.id.clone();
        let has_children = !node.children.is_empty();
        let is_expanded = self.expanded.contains(&node.id);
        let indent = node.depth as f32 * 16.0;

        let chevron = if has_children {
            if is_expanded { "v " } else { "> " }
        } else {
            "  "
        };

        let icon_text = node.icon.unwrap_or("");
        let label = node.label.clone();

        let mut container = div().w_full().flex().flex_col();

        // Node row
        let row_id = SharedString::from(format!("tree-{}", node.id));
        let row = div()
            .id(row_id)
            .h(px(22.0))
            .flex()
            .flex_row()
            .items_center()
            .pl(px(indent + 4.0))
            .pr_2()
            .text_xs()
            .text_color(rgb(0xcccccc))
            .hover(|s| s.bg(rgb(0x2a2a2a)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _window, cx| {
                if has_children {
                    this.toggle_expand(&id, cx);
                } else {
                    cx.emit(TreeViewEvent::NodeClicked(id.clone()));
                }
            }))
            .child(div().text_color(rgb(0x666666)).child(chevron.to_string()))
            .child(
                div()
                    .mr_1()
                    .text_color(rgb(0x888888))
                    .child(icon_text.to_string()),
            )
            .child(div().child(label));

        container = container.child(row);

        // Render children if expanded
        if is_expanded {
            for child in &node.children {
                container = container.child(self.render_node(child, cx));
            }
        }

        container
    }
}

impl Render for TreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut container = div()
            .id("tree-view")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden();

        for node in self.nodes.clone() {
            container = container.child(self.render_node(&node, cx));
        }

        container
    }
}
