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
        ToggleCommandPalette,
        FocusEditor,
        FocusResults,
        SaveConnection,
    ]
);

// Query commands
actions!(
    pgblade_query,
    [
        ExecuteQuery,
        ExecuteSelection,
        CancelQuery,
        PreviewTable,
        ShowHistory,
    ]
);

// Safety commands
actions!(pgblade_safety, [ToggleReadOnly, ConfirmWrite, RejectWrite,]);
