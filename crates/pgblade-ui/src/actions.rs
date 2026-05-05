use gpui::actions;

// Application-level commands
actions!(
    pgblade,
    [
        Quit,
        NewConnection,
        OpenConnection,
        DisconnectConnection,
        ToggleSidebar,
        FocusEditor,
        FocusResults,
    ]
);

// Query commands
actions!(
    pgblade_query,
    [ExecuteQuery, ExecuteSelection, CancelQuery,]
);

// Safety commands
actions!(pgblade_safety, [ToggleReadOnly, ConfirmWrite, RejectWrite,]);
