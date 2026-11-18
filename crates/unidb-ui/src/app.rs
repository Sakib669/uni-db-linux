//! The application shell: a libadwaita window with a sidebar, a Connections
//! workflow, a history and saved queries page, settings, a Security page from
//! live store state, and a startup diagnostic page. The dynamic parts are built
//! in Rust.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};
use sourceview5::prelude::*;
use uuid::Uuid;

use unidb_app::{
    AppService, AuditEntry, BundleImportReport, BundlePreview, CheckStatus, GitRepoStatus,
    HistoryEntry, ImportOptions, ImportReport, MigrationFile, MigrationReport, MultiTargetSummary,
    ReleaseInfo, ResultDiff, SavedQuery, SecurityReport, TableSnapshot, git_dir_status,
};
use unidb_core::{
    ChartPoint, ColumnDesign, ConnectionProfile, Engine, ErdLayout, ErdTable, ErrorCategory,
    FilterOp, QueryBuilder, QueryFilter, QueryResult, QueryResult as ResultSet, SqlDialect,
    SslMode, TableColumnInfo, TableDesign, TransferFormat, UniDbError, alter_table_sql,
    build_query, chart_series, create_table_sql, diff_results, erd_layout, explain_statement,
};

const ABOUT_NAME: &str = "UniDB";

struct ColumnWidgets {
    name: gtk::Entry,
    type_name: gtk::Entry,
    primary_key: gtk::CheckButton,
    not_null: gtk::CheckButton,
    unique: gtk::CheckButton,
    auto_increment: gtk::CheckButton,
    default: gtk::Entry,
}

struct FormValues {
    name: String,
    engine: Engine,
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
    ssl_mode: SslMode,
    read_only: bool,
    url_mode: bool,
    url_text: String,
    editing: Option<Uuid>,
}

struct AppModel {
    handle: tokio::runtime::Handle,
    service: Option<Arc<AppService>>,
    error: Option<String>,
    section: usize,
    busy: bool,
    security_loaded: bool,
    connections: Vec<ConnectionProfile>,
    connected: HashSet<Uuid>,
    editing: Option<Uuid>,
    root_stack: gtk::Stack,
    toasts: adw::ToastOverlay,
    content_page: adw::NavigationPage,
    page_stack: gtk::Stack,
    sidebar: gtk::ListBox,
    window_title: adw::WindowTitle,
    checks_group: adw::PreferencesGroup,
    checks_rows: Vec<adw::ActionRow>,
    audit_group: adw::PreferencesGroup,
    audit_rows: Vec<adw::ActionRow>,
    backup_button: gtk::Button,
    refresh_button: gtk::Button,
    diagnostic_page: adw::StatusPage,
    connections_group: adw::PreferencesGroup,
    connection_rows: Vec<adw::ActionRow>,
    connections_stack: gtk::Stack,
    details_group: adw::PreferencesGroup,
    name_entry: adw::EntryRow,
    engine_combo: adw::ComboRow,
    host_entry: adw::EntryRow,
    port_spin: adw::SpinRow,
    database_entry: adw::EntryRow,
    username_entry: adw::EntryRow,
    password_entry: adw::PasswordEntryRow,
    tls_combo: adw::ComboRow,
    readonly_switch: adw::SwitchRow,
    url_switch: adw::SwitchRow,
    url_entry: adw::EntryRow,
    save_button: gtk::Button,
    test_button: gtk::Button,
    navigator_stack: gtk::Stack,
    navigator_combo: adw::ComboRow,
    navigator_databases_list: gtk::ListBox,
    navigator_tables_list: gtk::ListBox,
    navigator_ids: Vec<Uuid>,
    navigator_connection: Option<Uuid>,
    navigator_database: Option<String>,
    editor_connection: Option<Uuid>,
    editor_ids: Vec<Uuid>,
    editor_busy: bool,
    editor_dropdown: gtk::DropDown,
    editor_view: sourceview5::View,
    editor_status: gtk::Label,
    editor_error: gtk::Label,
    editor_grid: gtk::Grid,
    run_button: gtk::Button,
    cancel_button: gtk::Button,
    clear_button: gtk::Button,
    explain_button: gtk::Button,
    chart_button: gtk::Button,
    editor_results_stack: gtk::Stack,
    chart_area: gtk::DrawingArea,
    plan_revealer: gtk::Revealer,
    plan_buffer: sourceview5::Buffer,
    editor_last_result: Option<QueryResult>,
    editor_baseline: Option<QueryResult>,
    chart_data: Rc<RefCell<Vec<ChartPoint>>>,
    designer_stack: gtk::Stack,
    designer_conn_dropdown: gtk::DropDown,
    designer_table_dropdown: gtk::DropDown,
    designer_name_entry: gtk::Entry,
    designer_columns_box: gtk::Box,
    designer_preview_buffer: sourceview5::Buffer,
    designer_status: gtk::Label,
    designer_ids: Vec<Uuid>,
    designer_connection: Option<Uuid>,
    designer_database: String,
    designer_table: Option<String>,
    designer_tables: Vec<String>,
    designer_original: Vec<TableColumnInfo>,
    designer_columns: Vec<ColumnDesign>,
    designer_widgets: Vec<ColumnWidgets>,
    designer_statements: Vec<String>,
    transfer_stack: gtk::Stack,
    transfer_conn_dropdown: gtk::DropDown,
    transfer_table_dropdown: gtk::DropDown,
    transfer_format_dropdown: gtk::DropDown,
    transfer_headers_check: gtk::CheckButton,
    transfer_path_entry: gtk::Entry,
    transfer_status: gtk::Label,
    transfer_ids: Vec<Uuid>,
    transfer_connection: Option<Uuid>,
    transfer_database: String,
    transfer_tables: Vec<String>,
    history_search_entry: gtk::Entry,
    history_status: gtk::Label,
    history_list: gtk::ListBox,
    saved_list: gtk::ListBox,
    saved_name_entry: gtk::Entry,
    history_entries: Vec<HistoryEntry>,
    history_rows: Vec<gtk::ListBoxRow>,
    saved_rows: Vec<gtk::ListBoxRow>,
    settings_combo: adw::ComboRow,
    settings_loaded: bool,
    update_button: gtk::Button,
    update_status: gtk::Label,
    update_feed: Option<String>,
    schema_stack: gtk::Stack,
    schema_conn_dropdown: gtk::DropDown,
    schema_status: gtk::Label,
    schema_canvas: gtk::DrawingArea,
    schema_tables: Rc<RefCell<Vec<ErdTable>>>,
    schema_ids: Vec<Uuid>,
    schema_connection: Option<Uuid>,
    schema_database: String,
    builder_stack: gtk::Stack,
    builder_conn_dropdown: gtk::DropDown,
    builder_table_dropdown: gtk::DropDown,
    builder_columns_box: gtk::Box,
    builder_filters_box: gtk::Box,
    builder_filter_column: gtk::DropDown,
    builder_filter_op: gtk::DropDown,
    builder_filter_value: gtk::Entry,
    builder_order_dropdown: gtk::DropDown,
    builder_order_asc: gtk::Switch,
    builder_limit_spin: gtk::SpinButton,
    builder_preview_buffer: sourceview5::Buffer,
    builder_status: gtk::Label,
    builder_ids: Vec<Uuid>,
    builder_connection: Option<Uuid>,
    builder_database: String,
    builder_tables: Vec<String>,
    builder_columns: Vec<String>,
    builder_selected: Vec<String>,
    builder_filters: Vec<QueryFilter>,
    builder_table: Option<String>,
    fake_count_spin: gtk::SpinButton,
    migrations_stack: gtk::Stack,
    migrations_conn_dropdown: gtk::DropDown,
    migrations_dir_entry: gtk::Entry,
    migrations_scan_button: gtk::Button,
    migrations_apply_button: gtk::Button,
    migrations_status: gtk::Label,
    migrations_list: gtk::ListBox,
    migrations_preview_buffer: sourceview5::Buffer,
    migrations_ids: Vec<Uuid>,
    migrations_connection: Option<Uuid>,
    migrations_files: Vec<MigrationFile>,
    migrations_applied: Vec<String>,
    migrations_warnings: Vec<String>,
    snapshots_conn_dropdown: gtk::DropDown,
    snapshots_table_entry: gtk::Entry,
    snapshots_note_entry: gtk::Entry,
    snapshots_create_button: gtk::Button,
    snapshots_refresh_button: gtk::Button,
    snapshots_status: gtk::Label,
    snapshots_list: gtk::ListBox,
    snapshots_ids: Vec<Uuid>,
    snapshots_connection: Option<Uuid>,
    snapshots_items: Vec<TableSnapshot>,
    snapshots_rows: Vec<gtk::ListBoxRow>,
    run_multi_button: gtk::Button,
    bundle_path_entry: gtk::Entry,
    bundle_export_button: gtk::Button,
    bundle_inspect_button: gtk::Button,
    bundle_status: gtk::Label,
    migrations_git_status: GitRepoStatus,
}

enum AppMsg {
    SelectSection(usize),
    Refresh,
    Backup,
    Toast(String),
    SecurityLoaded(Box<(SecurityReport, Vec<AuditEntry>)>),
    SecurityFailed(String),
    RetryBootstrap,
    BootstrapOk(Arc<AppService>),
    BootstrapFailed(String),
    About,
    Quit,
    ConnectionsLoaded(Box<Vec<ConnectionProfile>>),
    ConnectionsFailed(String),
    ShowNewConnection,
    EditConnection(Uuid),
    SaveForm,
    TestForm,
    CancelForm,
    FormSaved,
    FormFailed(String),
    TestFinished(String),
    Connect(Uuid),
    Disconnect(Uuid),
    ConnectionState(Uuid, bool),
    ConnectionFailed(String),
    DeleteConnection(Uuid),
    ConnectionDeleted(Uuid),
    NavigatorConnectionSelected(u32),
    NavigatorDatabases(Uuid, Box<Vec<String>>),
    NavigatorTables(String, Box<Vec<String>>),
    NavigatorSelectDatabase(String),
    NavigatorFailed(String),
    EditorConnectionSelected(u32),
    RunQuery,
    CancelQuery,
    QueryFinished(Box<unidb_core::QueryResult>),
    ExplainQuery,
    ExplainFinished(Box<unidb_core::QueryResult>),
    ExplainFailed(String),
    ToggleChart,
    CompareResults,
    QueryFailed(String),
    ClearResults,
    DesignerConnectionSelected(u32),
    DesignerTables(Box<Vec<String>>),
    DesignerTableSelected(u32),
    DesignerDescribed(String, Box<Vec<TableColumnInfo>>),
    DesignerAddColumn,
    DesignerRemoveColumn(usize),
    DesignerPreview,
    DesignerApply,
    DesignerApplied(usize),
    DesignerFailed(String),
    TransferConnectionSelected(u32),
    TransferTables(Box<Vec<String>>),
    TransferTableSelected(u32),
    TransferPathChanged(String),
    TransferBrowse,
    TransferImport,
    TransferFinished(Box<ImportReport>),
    TransferFailed(String),
    ExportQuery(TransferFormat),
    ExportReady(Uuid, TransferFormat, std::path::PathBuf, String),
    ExportQueryMasked(TransferFormat),
    ExportReadyMasked(Uuid, TransferFormat, std::path::PathBuf, String),
    LoadHistory,
    HistoryLoaded(Box<Vec<HistoryEntry>>),
    SavedQueriesLoaded(Box<Vec<SavedQuery>>),
    SavedQuerySaved,
    ClearHistoryRequested,
    ClearHistoryConfirmed,
    SaveCurrentQuery,
    LoadSql(String),
    DeleteSavedQuery(Uuid),
    HistoryFailed(String),
    SettingsLoaded(Option<String>),
    SetColorScheme(u32),
    CheckUpdates,
    UpdateResult(Option<Box<ReleaseInfo>>),
    UpdateFailed(String),
    SchemaRefresh,
    SchemaConnectionSelected(u32),
    SchemaLoaded(Box<Vec<ErdTable>>),
    SchemaFailed(String),
    BuilderConnectionSelected(u32),
    BuilderTables(Box<Vec<String>>),
    BuilderTableSelected(u32),
    BuilderDescribed(Box<Vec<TableColumnInfo>>),
    BuilderToggleColumn(String, bool),
    BuilderAddFilter,
    BuilderRemoveFilter(usize),
    BuilderPreview,
    BuilderOpenInEditor,
    BuilderFailed(String),
    GenerateFakeData,
    FakeRowsGenerated(usize),
    FakeRowsFailed(String),
    MigrationsConnChanged(usize),
    MigrationsScan,
    MigrationsScanned(Box<(Vec<MigrationFile>, Vec<String>, Vec<String>, GitRepoStatus)>),
    MigrationsScanFailed(String),
    MigrationsSelectFile(usize),
    MigrationsApply,
    MigrationsApplied(Box<MigrationReport>),
    MigrationsApplyFailed(String),
    SnapshotsConnChanged(usize),
    SnapshotsRefresh,
    SnapshotsLoaded(Vec<TableSnapshot>),
    SnapshotsFailed(String),
    TakeSnapshot,
    SnapshotTaken(Box<TableSnapshot>),
    CompareSnapshot(Uuid),
    SnapshotDiffed(Box<(String, ResultDiff)>),
    RestoreSnapshot(Uuid),
    SnapshotRestored(usize, String),
    DeleteSnapshot(Uuid),
    SnapshotDeleted,
    RunMultiQuery,
    MultiQueryFinished(Vec<MultiTargetSummary>, Box<QueryResult>),
    BundlePathChanged(String),
    BundleBrowse,
    ExportTeamBundle,
    TeamBundleExported(std::path::PathBuf, usize, usize, String),
    InspectTeamBundle,
    TeamBundleInspected(std::path::PathBuf, Box<BundlePreview>),
    ConfirmImportTeamBundle(std::path::PathBuf),
    TeamBundleImported(Box<BundleImportReport>),
    TeamBundleFailed(String),
}

impl std::fmt::Debug for AppMsg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SelectSection(_) => "SelectSection",
            Self::Refresh => "Refresh",
            Self::Backup => "Backup",
            Self::Toast(_) => "Toast",
            Self::SecurityLoaded(_) => "SecurityLoaded",
            Self::SecurityFailed(_) => "SecurityFailed",
            Self::RetryBootstrap => "RetryBootstrap",
            Self::BootstrapOk(_) => "BootstrapOk",
            Self::BootstrapFailed(_) => "BootstrapFailed",
            Self::About => "About",
            Self::Quit => "Quit",
            Self::ConnectionsLoaded(_) => "ConnectionsLoaded",
            Self::ConnectionsFailed(_) => "ConnectionsFailed",
            Self::ShowNewConnection => "ShowNewConnection",
            Self::EditConnection(_) => "EditConnection",
            Self::SaveForm => "SaveForm",
            Self::TestForm => "TestForm",
            Self::CancelForm => "CancelForm",
            Self::FormSaved => "FormSaved",
            Self::FormFailed(_) => "FormFailed",
            Self::TestFinished(_) => "TestFinished",
            Self::Connect(_) => "Connect",
            Self::Disconnect(_) => "Disconnect",
            Self::ConnectionState(_, _) => "ConnectionState",
            Self::ConnectionFailed(_) => "ConnectionFailed",
            Self::DeleteConnection(_) => "DeleteConnection",
            Self::ConnectionDeleted(_) => "ConnectionDeleted",
            Self::NavigatorConnectionSelected(_) => "NavigatorConnectionSelected",
            Self::NavigatorDatabases(_, _) => "NavigatorDatabases",
            Self::NavigatorTables(_, _) => "NavigatorTables",
            Self::NavigatorSelectDatabase(_) => "NavigatorSelectDatabase",
            Self::NavigatorFailed(_) => "NavigatorFailed",
            Self::EditorConnectionSelected(_) => "EditorConnectionSelected",
            Self::RunQuery => "RunQuery",
            Self::CancelQuery => "CancelQuery",
            Self::QueryFinished(_) => "QueryFinished",
            Self::ExplainQuery => "ExplainQuery",
            Self::ExplainFinished(_) => "ExplainFinished",
            Self::ExplainFailed(_) => "ExplainFailed",
            Self::ToggleChart => "ToggleChart",
            Self::CompareResults => "CompareResults",
            Self::QueryFailed(_) => "QueryFailed",
            Self::ClearResults => "ClearResults",
            Self::DesignerConnectionSelected(_) => "DesignerConnectionSelected",
            Self::DesignerTables(_) => "DesignerTables",
            Self::DesignerTableSelected(_) => "DesignerTableSelected",
            Self::DesignerDescribed(_, _) => "DesignerDescribed",
            Self::DesignerAddColumn => "DesignerAddColumn",
            Self::DesignerRemoveColumn(_) => "DesignerRemoveColumn",
            Self::DesignerPreview => "DesignerPreview",
            Self::DesignerApply => "DesignerApply",
            Self::DesignerApplied(_) => "DesignerApplied",
            Self::DesignerFailed(_) => "DesignerFailed",
            Self::TransferConnectionSelected(_) => "TransferConnectionSelected",
            Self::TransferTables(_) => "TransferTables",
            Self::TransferTableSelected(_) => "TransferTableSelected",
            Self::TransferPathChanged(_) => "TransferPathChanged",
            Self::TransferBrowse => "TransferBrowse",
            Self::TransferImport => "TransferImport",
            Self::TransferFinished(_) => "TransferFinished",
            Self::TransferFailed(_) => "TransferFailed",
            Self::ExportQuery(_) => "ExportQuery",
            Self::ExportReady(_, _, _, _) => "ExportReady",
            Self::ExportQueryMasked(_) => "ExportQueryMasked",
            Self::ExportReadyMasked(_, _, _, _) => "ExportReadyMasked",
            Self::LoadHistory => "LoadHistory",
            Self::HistoryLoaded(_) => "HistoryLoaded",
            Self::SavedQueriesLoaded(_) => "SavedQueriesLoaded",
            Self::SavedQuerySaved => "SavedQuerySaved",
            Self::ClearHistoryRequested => "ClearHistoryRequested",
            Self::ClearHistoryConfirmed => "ClearHistoryConfirmed",
            Self::SaveCurrentQuery => "SaveCurrentQuery",
            Self::LoadSql(_) => "LoadSql",
            Self::DeleteSavedQuery(_) => "DeleteSavedQuery",
            Self::HistoryFailed(_) => "HistoryFailed",
            Self::SettingsLoaded(_) => "SettingsLoaded",
            Self::SetColorScheme(_) => "SetColorScheme",
            Self::CheckUpdates => "CheckUpdates",
            Self::UpdateResult(_) => "UpdateResult",
            Self::UpdateFailed(_) => "UpdateFailed",
            Self::SchemaRefresh => "SchemaRefresh",
            Self::SchemaConnectionSelected(_) => "SchemaConnectionSelected",
            Self::SchemaLoaded(_) => "SchemaLoaded",
            Self::SchemaFailed(_) => "SchemaFailed",
            Self::BuilderConnectionSelected(_) => "BuilderConnectionSelected",
            Self::BuilderTables(_) => "BuilderTables",
            Self::BuilderTableSelected(_) => "BuilderTableSelected",
            Self::BuilderDescribed(_) => "BuilderDescribed",
            Self::BuilderToggleColumn(_, _) => "BuilderToggleColumn",
            Self::BuilderAddFilter => "BuilderAddFilter",
            Self::BuilderRemoveFilter(_) => "BuilderRemoveFilter",
            Self::BuilderPreview => "BuilderPreview",
            Self::BuilderOpenInEditor => "BuilderOpenInEditor",
            Self::BuilderFailed(_) => "BuilderFailed",
            Self::GenerateFakeData => "GenerateFakeData",
            Self::FakeRowsGenerated(_) => "FakeRowsGenerated",
            Self::FakeRowsFailed(_) => "FakeRowsFailed",
            Self::MigrationsConnChanged(_) => "MigrationsConnChanged",
            Self::MigrationsScan => "MigrationsScan",
            Self::MigrationsScanned(_) => "MigrationsScanned",
            Self::MigrationsScanFailed(_) => "MigrationsScanFailed",
            Self::MigrationsSelectFile(_) => "MigrationsSelectFile",
            Self::MigrationsApply => "MigrationsApply",
            Self::MigrationsApplied(_) => "MigrationsApplied",
            Self::MigrationsApplyFailed(_) => "MigrationsApplyFailed",
            Self::SnapshotsConnChanged(_) => "SnapshotsConnChanged",
            Self::SnapshotsRefresh => "SnapshotsRefresh",
            Self::SnapshotsLoaded(_) => "SnapshotsLoaded",
            Self::SnapshotsFailed(_) => "SnapshotsFailed",
            Self::TakeSnapshot => "TakeSnapshot",
            Self::SnapshotTaken(_) => "SnapshotTaken",
            Self::CompareSnapshot(_) => "CompareSnapshot",
            Self::SnapshotDiffed(_) => "SnapshotDiffed",
            Self::RestoreSnapshot(_) => "RestoreSnapshot",
            Self::SnapshotRestored(_, _) => "SnapshotRestored",
            Self::DeleteSnapshot(_) => "DeleteSnapshot",
            Self::SnapshotDeleted => "SnapshotDeleted",
            Self::RunMultiQuery => "RunMultiQuery",
            Self::MultiQueryFinished(_, _) => "MultiQueryFinished",
            Self::BundlePathChanged(_) => "BundlePathChanged",
            Self::BundleBrowse => "BundleBrowse",
            Self::ExportTeamBundle => "ExportTeamBundle",
            Self::TeamBundleExported(_, _, _, _) => "TeamBundleExported",
            Self::InspectTeamBundle => "InspectTeamBundle",
            Self::TeamBundleInspected(_, _) => "TeamBundleInspected",
            Self::ConfirmImportTeamBundle(_) => "ConfirmImportTeamBundle",
            Self::TeamBundleImported(_) => "TeamBundleImported",
            Self::TeamBundleFailed(_) => "TeamBundleFailed",
        })
    }
}

struct ShellWidgets {
    root_stack: gtk::Stack,
    toasts: adw::ToastOverlay,
    content_page: adw::NavigationPage,
    page_stack: gtk::Stack,
    sidebar: gtk::ListBox,
    window_title: adw::WindowTitle,
    checks_group: adw::PreferencesGroup,
    audit_group: adw::PreferencesGroup,
    backup_button: gtk::Button,
    refresh_button: gtk::Button,
    diagnostic_page: adw::StatusPage,
    connections_group: adw::PreferencesGroup,
    connections_stack: gtk::Stack,
    details_group: adw::PreferencesGroup,
    name_entry: adw::EntryRow,
    engine_combo: adw::ComboRow,
    host_entry: adw::EntryRow,
    port_spin: adw::SpinRow,
    database_entry: adw::EntryRow,
    username_entry: adw::EntryRow,
    password_entry: adw::PasswordEntryRow,
    tls_combo: adw::ComboRow,
    readonly_switch: adw::SwitchRow,
    url_switch: adw::SwitchRow,
    url_entry: adw::EntryRow,
    save_button: gtk::Button,
    test_button: gtk::Button,
    navigator_stack: gtk::Stack,
    navigator_combo: adw::ComboRow,
    navigator_databases_list: gtk::ListBox,
    navigator_tables_list: gtk::ListBox,
    editor_dropdown: gtk::DropDown,
    editor_view: sourceview5::View,
    editor_status: gtk::Label,
    editor_error: gtk::Label,
    editor_grid: gtk::Grid,
    run_button: gtk::Button,
    cancel_button: gtk::Button,
    clear_button: gtk::Button,
    explain_button: gtk::Button,
    chart_button: gtk::Button,
    editor_results_stack: gtk::Stack,
    chart_area: gtk::DrawingArea,
    plan_revealer: gtk::Revealer,
    plan_buffer: sourceview5::Buffer,
    chart_data: Rc<RefCell<Vec<ChartPoint>>>,
    designer_stack: gtk::Stack,
    designer_conn_dropdown: gtk::DropDown,
    designer_table_dropdown: gtk::DropDown,
    designer_name_entry: gtk::Entry,
    designer_columns_box: gtk::Box,
    designer_preview_buffer: sourceview5::Buffer,
    designer_status: gtk::Label,
    transfer_stack: gtk::Stack,
    transfer_conn_dropdown: gtk::DropDown,
    transfer_table_dropdown: gtk::DropDown,
    transfer_format_dropdown: gtk::DropDown,
    transfer_headers_check: gtk::CheckButton,
    transfer_path_entry: gtk::Entry,
    transfer_status: gtk::Label,
    history_search_entry: gtk::Entry,
    history_status: gtk::Label,
    history_list: gtk::ListBox,
    saved_list: gtk::ListBox,
    saved_name_entry: gtk::Entry,
    settings_combo: adw::ComboRow,
    update_button: gtk::Button,
    update_status: gtk::Label,
    schema_stack: gtk::Stack,
    schema_conn_dropdown: gtk::DropDown,
    schema_status: gtk::Label,
    schema_canvas: gtk::DrawingArea,
    schema_tables: Rc<RefCell<Vec<ErdTable>>>,
    builder_stack: gtk::Stack,
    builder_conn_dropdown: gtk::DropDown,
    builder_table_dropdown: gtk::DropDown,
    builder_columns_box: gtk::Box,
    builder_filters_box: gtk::Box,
    builder_filter_column: gtk::DropDown,
    builder_filter_op: gtk::DropDown,
    builder_filter_value: gtk::Entry,
    builder_order_dropdown: gtk::DropDown,
    builder_order_asc: gtk::Switch,
    builder_limit_spin: gtk::SpinButton,
    builder_preview_buffer: sourceview5::Buffer,
    builder_status: gtk::Label,
    fake_count_spin: gtk::SpinButton,
    migrations_stack: gtk::Stack,
    migrations_conn_dropdown: gtk::DropDown,
    migrations_dir_entry: gtk::Entry,
    migrations_scan_button: gtk::Button,
    migrations_apply_button: gtk::Button,
    migrations_status: gtk::Label,
    migrations_list: gtk::ListBox,
    migrations_preview_buffer: sourceview5::Buffer,
    snapshots_conn_dropdown: gtk::DropDown,
    snapshots_table_entry: gtk::Entry,
    snapshots_note_entry: gtk::Entry,
    snapshots_create_button: gtk::Button,
    snapshots_refresh_button: gtk::Button,
    snapshots_status: gtk::Label,
    snapshots_list: gtk::ListBox,
    run_multi_button: gtk::Button,
    bundle_path_entry: gtk::Entry,
    bundle_export_button: gtk::Button,
    bundle_inspect_button: gtk::Button,
    bundle_status: gtk::Label,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = (tokio::runtime::Handle, Result<Arc<AppService>, String>);
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "window"]
        adw::ApplicationWindow {
            set_title: Some(ABOUT_NAME),
            set_default_width: 1120,
            set_default_height: 760,
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        crate::load_stylesheet();
        let (handle, startup) = init;
        let (service, error) = match startup {
            Ok(service) => (Some(service), None),
            Err(message) => (None, Some(message)),
        };

        let widgets = build_shell(&root, &sender);
        let mut model = AppModel {
            handle,
            service,
            error,
            section: 0,
            busy: false,
            security_loaded: false,
            connections: Vec::new(),
            connected: HashSet::new(),
            editing: None,
            root_stack: widgets.root_stack,
            toasts: widgets.toasts,
            content_page: widgets.content_page,
            page_stack: widgets.page_stack,
            sidebar: widgets.sidebar,
            window_title: widgets.window_title,
            checks_group: widgets.checks_group,
            checks_rows: Vec::new(),
            audit_group: widgets.audit_group,
            audit_rows: Vec::new(),
            backup_button: widgets.backup_button,
            refresh_button: widgets.refresh_button,
            diagnostic_page: widgets.diagnostic_page,
            connections_group: widgets.connections_group,
            connection_rows: Vec::new(),
            connections_stack: widgets.connections_stack,
            details_group: widgets.details_group,
            name_entry: widgets.name_entry,
            engine_combo: widgets.engine_combo,
            host_entry: widgets.host_entry,
            port_spin: widgets.port_spin,
            database_entry: widgets.database_entry,
            username_entry: widgets.username_entry,
            password_entry: widgets.password_entry,
            tls_combo: widgets.tls_combo,
            readonly_switch: widgets.readonly_switch,
            url_switch: widgets.url_switch,
            url_entry: widgets.url_entry,
            save_button: widgets.save_button,
            test_button: widgets.test_button,
            navigator_stack: widgets.navigator_stack,
            navigator_combo: widgets.navigator_combo,
            navigator_databases_list: widgets.navigator_databases_list,
            navigator_tables_list: widgets.navigator_tables_list,
            navigator_ids: Vec::new(),
            navigator_connection: None,
            navigator_database: None,
            editor_connection: None,
            editor_ids: Vec::new(),
            editor_busy: false,
            editor_dropdown: widgets.editor_dropdown,
            editor_view: widgets.editor_view,
            editor_status: widgets.editor_status,
            editor_error: widgets.editor_error,
            editor_grid: widgets.editor_grid,
            run_button: widgets.run_button,
            cancel_button: widgets.cancel_button,
            clear_button: widgets.clear_button,
            explain_button: widgets.explain_button,
            chart_button: widgets.chart_button,
            editor_results_stack: widgets.editor_results_stack,
            chart_area: widgets.chart_area,
            plan_revealer: widgets.plan_revealer,
            plan_buffer: widgets.plan_buffer,
            editor_last_result: None,
            editor_baseline: None,
            chart_data: widgets.chart_data,
            designer_stack: widgets.designer_stack,
            designer_conn_dropdown: widgets.designer_conn_dropdown,
            designer_table_dropdown: widgets.designer_table_dropdown,
            designer_name_entry: widgets.designer_name_entry,
            designer_columns_box: widgets.designer_columns_box,
            designer_preview_buffer: widgets.designer_preview_buffer,
            designer_status: widgets.designer_status,
            designer_ids: Vec::new(),
            designer_connection: None,
            designer_database: "main".to_string(),
            designer_table: None,
            designer_tables: Vec::new(),
            designer_original: Vec::new(),
            designer_columns: Vec::new(),
            designer_widgets: Vec::new(),
            designer_statements: Vec::new(),
            transfer_stack: widgets.transfer_stack,
            transfer_conn_dropdown: widgets.transfer_conn_dropdown,
            transfer_table_dropdown: widgets.transfer_table_dropdown,
            transfer_format_dropdown: widgets.transfer_format_dropdown,
            transfer_headers_check: widgets.transfer_headers_check,
            transfer_path_entry: widgets.transfer_path_entry,
            transfer_status: widgets.transfer_status,
            transfer_ids: Vec::new(),
            transfer_connection: None,
            transfer_database: "main".to_string(),
            transfer_tables: Vec::new(),
            history_search_entry: widgets.history_search_entry,
            history_status: widgets.history_status,
            history_list: widgets.history_list,
            saved_list: widgets.saved_list,
            saved_name_entry: widgets.saved_name_entry,
            history_entries: Vec::new(),
            history_rows: Vec::new(),
            saved_rows: Vec::new(),
            settings_combo: widgets.settings_combo,
            settings_loaded: false,
            update_button: widgets.update_button,
            update_status: widgets.update_status,
            update_feed: std::env::var("UNIDB_UPDATE_FEED").ok(),
            schema_stack: widgets.schema_stack,
            schema_conn_dropdown: widgets.schema_conn_dropdown,
            schema_status: widgets.schema_status,
            schema_canvas: widgets.schema_canvas,
            schema_tables: widgets.schema_tables,
            schema_ids: Vec::new(),
            schema_connection: None,
            schema_database: "main".to_string(),
            builder_stack: widgets.builder_stack,
            builder_conn_dropdown: widgets.builder_conn_dropdown,
            builder_table_dropdown: widgets.builder_table_dropdown,
            builder_columns_box: widgets.builder_columns_box,
            builder_filters_box: widgets.builder_filters_box,
            builder_filter_column: widgets.builder_filter_column,
            builder_filter_op: widgets.builder_filter_op,
            builder_filter_value: widgets.builder_filter_value,
            builder_order_dropdown: widgets.builder_order_dropdown,
            builder_order_asc: widgets.builder_order_asc,
            builder_limit_spin: widgets.builder_limit_spin,
            builder_preview_buffer: widgets.builder_preview_buffer,
            builder_status: widgets.builder_status,
            builder_ids: Vec::new(),
            builder_connection: None,
            builder_database: "main".to_string(),
            builder_tables: Vec::new(),
            builder_columns: Vec::new(),
            builder_selected: Vec::new(),
            builder_filters: Vec::new(),
            builder_table: None,
            fake_count_spin: widgets.fake_count_spin,
            migrations_stack: widgets.migrations_stack,
            migrations_conn_dropdown: widgets.migrations_conn_dropdown,
            migrations_dir_entry: widgets.migrations_dir_entry,
            migrations_scan_button: widgets.migrations_scan_button,
            migrations_apply_button: widgets.migrations_apply_button,
            migrations_status: widgets.migrations_status,
            migrations_list: widgets.migrations_list,
            migrations_preview_buffer: widgets.migrations_preview_buffer,
            migrations_ids: Vec::new(),
            migrations_connection: None,
            migrations_files: Vec::new(),
            migrations_applied: Vec::new(),
            migrations_warnings: Vec::new(),
            snapshots_conn_dropdown: widgets.snapshots_conn_dropdown,
            snapshots_table_entry: widgets.snapshots_table_entry,
            snapshots_note_entry: widgets.snapshots_note_entry,
            snapshots_create_button: widgets.snapshots_create_button,
            snapshots_refresh_button: widgets.snapshots_refresh_button,
            snapshots_status: widgets.snapshots_status,
            snapshots_list: widgets.snapshots_list,
            snapshots_ids: Vec::new(),
            snapshots_connection: None,
            snapshots_items: Vec::new(),
            snapshots_rows: Vec::new(),
            run_multi_button: widgets.run_multi_button,
            bundle_path_entry: widgets.bundle_path_entry,
            bundle_export_button: widgets.bundle_export_button,
            bundle_inspect_button: widgets.bundle_inspect_button,
            bundle_status: widgets.bundle_status,
            migrations_git_status: GitRepoStatus::default(),
        };

        if let Some(error) = &model.error {
            model.show_diagnostic(error);
        } else {
            model.root_stack.set_visible_child_name("shell");
            model.select_section(0);
            model.load_connections(&sender);
            model.load_security(&sender);
            model.load_color_scheme(&sender);
        }
        model.update_controls();

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::SelectSection(index) => {
                self.select_section(index);
                if index == 1 {
                    self.refresh_navigator_connections(&sender);
                }
                if index == 2 {
                    self.refresh_editor_connections(&sender);
                }
                if index == 3 {
                    self.refresh_designer(&sender);
                }
                if index == 4 {
                    self.refresh_transfer(&sender);
                }
                if index == 5 {
                    self.load_history(&sender);
                    self.refresh_snapshots_connections(&sender);
                }
                if index == 7 {
                    self.refresh_schema(&sender);
                }
                if index == 8 {
                    self.refresh_builder(&sender);
                }
                if index == 9 {
                    self.refresh_migrations(&sender);
                }
                if index == 10 && !self.security_loaded {
                    self.load_security(&sender);
                }
            }
            AppMsg::Refresh => {
                self.load_security(&sender);
                self.load_connections(&sender);
            }
            AppMsg::Backup => {
                if let Some(service) = &self.service {
                    self.busy = true;
                    self.update_controls();
                    let service = Arc::clone(service);
                    let sender = sender.clone();
                    self.handle.spawn(async move {
                        match service.backup_now().await {
                            Ok(path) => sender.input(AppMsg::Toast(format!(
                                "Backup written to {}",
                                path.display()
                            ))),
                            Err(error) => sender.input(AppMsg::Toast(format!(
                                "Backup failed: {}",
                                error.user_message()
                            ))),
                        }
                    });
                }
            }
            AppMsg::Toast(message) => {
                self.busy = false;
                self.update_controls();
                self.show_toast(&message);
            }
            AppMsg::SecurityLoaded(payload) => {
                let (report, audit) = *payload;
                self.populate_security(&report, &audit);
                self.security_loaded = true;
                self.update_controls();
            }
            AppMsg::SecurityFailed(message) => {
                self.update_controls();
                self.show_toast(&format!("Could not build the security report: {message}"));
            }
            AppMsg::RetryBootstrap => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.update_controls();
                self.diagnostic_page
                    .set_description(Some("Retrying startup..."));
                let handle = self.handle.clone();
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match AppService::bootstrap(handle).await {
                        Ok(service) => sender.input(AppMsg::BootstrapOk(Arc::new(service))),
                        Err(error) => sender.input(AppMsg::BootstrapFailed(format!(
                            "{} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::BootstrapOk(service) => {
                self.busy = false;
                self.error = None;
                self.service = Some(service);
                self.root_stack.set_visible_child_name("shell");
                self.update_controls();
                self.load_connections(&sender);
                self.load_security(&sender);
                self.show_toast("The app store opened.");
            }
            AppMsg::BootstrapFailed(message) => {
                self.busy = false;
                self.error = Some(message.clone());
                self.show_diagnostic(&message);
                self.update_controls();
            }
            AppMsg::About => {
                let about = adw::AboutWindow::builder()
                    .application_name(ABOUT_NAME)
                    .application_icon("io.github.sakib669.UniDB")
                    .developer_name("Sakib")
                    .version(env!("CARGO_PKG_VERSION"))
                    .comments("UniDB is a native database studio for Linux.")
                    .build();
                about.present();
            }
            AppMsg::Quit => relm4::main_application().quit(),
            AppMsg::ConnectionsLoaded(profiles) => {
                self.connections = *profiles;
                self.populate_connections(&sender);
            }
            AppMsg::ConnectionsFailed(message) => {
                self.show_toast(&format!("Could not load connections: {message}"));
            }
            AppMsg::ShowNewConnection => {
                self.open_form(None);
            }
            AppMsg::EditConnection(id) => {
                let profile = self.connections.iter().find(|item| item.id == id).cloned();
                if let Some(profile) = profile {
                    self.open_form(Some(&profile));
                }
            }
            AppMsg::CancelForm => {
                self.editing = None;
                self.connections_stack.set_visible_child_name("list");
            }
            AppMsg::SaveForm => {
                let Some(values) = self.form_values() else {
                    return;
                };
                let request = self.build_profile(&values);
                let (profile, password) = match request {
                    Ok(value) => value,
                    Err(error) => {
                        self.show_toast(error.user_message());
                        return;
                    }
                };
                let Some(service) = &self.service else {
                    return;
                };
                self.busy = true;
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.save_connection(&profile, password.as_deref()).await {
                        Ok(()) => sender.input(AppMsg::FormSaved),
                        Err(error) => sender.input(AppMsg::FormFailed(format!(
                            "{} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::FormSaved => {
                self.busy = false;
                self.editing = None;
                self.connections_stack.set_visible_child_name("list");
                self.update_controls();
                self.load_connections(&sender);
                self.show_toast("Connection saved.");
            }
            AppMsg::FormFailed(message) => {
                self.busy = false;
                self.update_controls();
                self.show_toast(&format!("Could not save the connection: {message}"));
            }
            AppMsg::TestForm => {
                let Some(values) = self.form_values() else {
                    return;
                };
                let (profile, password) = match self.build_profile(&values) {
                    Ok(value) => value,
                    Err(error) => {
                        self.show_toast(error.user_message());
                        return;
                    }
                };
                let Some(service) = &self.service else {
                    return;
                };
                self.busy = true;
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.test_connection(&profile, password.as_deref()).await {
                        Ok(()) => {
                            sender.input(AppMsg::TestFinished("Connection works.".to_string()));
                        }
                        Err(error) => sender.input(AppMsg::TestFinished(format!(
                            "Connection failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::TestFinished(message) => {
                self.busy = false;
                self.update_controls();
                self.show_toast(&message);
            }
            AppMsg::Connect(id) => {
                let Some(profile) = self.connections.iter().find(|item| item.id == id).cloned()
                else {
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                self.busy = true;
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.open_connection(&profile).await {
                        Ok(()) => sender.input(AppMsg::ConnectionState(id, true)),
                        Err(error) => sender.input(AppMsg::ConnectionFailed(format!(
                            "Could not connect: {} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::Disconnect(id) => {
                let Some(service) = &self.service else {
                    return;
                };
                self.busy = true;
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    let _ = service.close_connection(id).await;
                    sender.input(AppMsg::ConnectionState(id, false));
                });
            }
            AppMsg::ConnectionState(id, live) => {
                self.busy = false;
                if live {
                    self.connected.insert(id);
                    self.show_toast("Connected.");
                } else {
                    self.connected.remove(&id);
                    self.show_toast("Disconnected.");
                }
                self.update_controls();
                self.populate_connections(&sender);
                self.refresh_navigator_connections(&sender);
                self.refresh_editor_connections(&sender);
                self.refresh_designer(&sender);
                self.refresh_transfer(&sender);
                self.refresh_schema(&sender);
                self.refresh_builder(&sender);
            }
            AppMsg::ConnectionFailed(message) => {
                self.busy = false;
                self.update_controls();
                self.show_toast(&message);
            }
            AppMsg::DeleteConnection(id) => {
                let Some(service) = &self.service else {
                    return;
                };
                self.busy = true;
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    let _ = service.close_connection(id).await;
                    match service.delete_connection(id).await {
                        Ok(_) => sender.input(AppMsg::ConnectionDeleted(id)),
                        Err(error) => sender.input(AppMsg::ConnectionFailed(format!(
                            "Could not delete: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::ConnectionDeleted(id) => {
                self.busy = false;
                self.connected.remove(&id);
                self.connections.retain(|profile| profile.id != id);
                self.update_controls();
                self.populate_connections(&sender);
                self.refresh_navigator_connections(&sender);
                self.show_toast("Connection deleted.");
            }
            AppMsg::NavigatorConnectionSelected(index) => {
                if let Some(id) = self.navigator_ids.get(index as usize).copied() {
                    self.navigator_connection = Some(id);
                    self.navigator_database = None;
                    self.load_navigator_databases(id, &sender);
                }
            }
            AppMsg::NavigatorDatabases(id, databases) => {
                self.navigator_connection = Some(id);
                self.populate_databases(&databases, &sender);
                if let Some(first) = databases.first().cloned() {
                    self.navigator_database = Some(first.clone());
                    self.load_navigator_tables(id, &first, &sender);
                } else {
                    self.navigator_database = None;
                    self.populate_tables(&[]);
                }
            }
            AppMsg::NavigatorTables(database, tables) => {
                self.navigator_database = Some(database);
                self.populate_tables(&tables);
            }
            AppMsg::NavigatorSelectDatabase(database) => {
                if let Some(id) = self.navigator_connection {
                    self.load_navigator_tables(id, &database, &sender);
                }
            }
            AppMsg::NavigatorFailed(message) => {
                self.show_toast(&message);
                self.populate_tables(&[]);
            }
            AppMsg::EditorConnectionSelected(index) => {
                if let Some(id) = self.editor_ids.get(index as usize).copied() {
                    self.editor_connection = Some(id);
                }
            }
            AppMsg::RunQuery => self.run_query(&sender),
            AppMsg::CancelQuery => {
                let Some(id) = self.editor_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.cancel(id).await {
                        Ok(()) => {
                            sender.input(AppMsg::QueryFailed("Cancel requested.".to_string()))
                        }
                        Err(error) => sender.input(AppMsg::QueryFailed(format!(
                            "Could not cancel: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::QueryFinished(result) => {
                self.editor_busy = false;
                self.editor_last_result = Some((*result).clone());
                self.chart_data.borrow_mut().clear();
                self.editor_results_stack.set_visible_child_name("grid");
                self.chart_button.set_label("Chart");
                self.update_controls();
                self.render_result(&result);
            }
            AppMsg::ExplainQuery => {
                let Some(connection_id) = self.editor_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let engine = self
                    .connections
                    .iter()
                    .find(|profile| profile.id == connection_id)
                    .map(|profile| profile.engine);
                let Some(engine) = engine else {
                    return;
                };
                let buffer = self.editor_view.buffer();
                let sql = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                let statement = match explain_statement(engine, &sql) {
                    Ok(statement) => statement,
                    Err(error) => {
                        self.show_toast(error.user_message());
                        return;
                    }
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.editor_status.set_label("Explaining...");
                self.handle.spawn(async move {
                    match service.execute(connection_id, &statement).await {
                        Ok(result) => sender.input(AppMsg::ExplainFinished(Box::new(result))),
                        Err(error) => sender.input(AppMsg::ExplainFailed(format!(
                            "Explain failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::ExplainFinished(result) => {
                let plan = format_plan(&result);
                self.plan_buffer.set_text(&plan);
                self.plan_revealer.set_reveal_child(true);
                self.editor_status.set_label("Plan ready.");
            }
            AppMsg::ExplainFailed(message) => {
                self.editor_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::ToggleChart => {
                let showing_chart =
                    self.editor_results_stack.visible_child_name().as_deref() == Some("chart");
                if showing_chart {
                    self.editor_results_stack.set_visible_child_name("grid");
                    self.chart_button.set_label("Chart");
                    return;
                }
                let series = self
                    .editor_last_result
                    .as_ref()
                    .map(|result| chart_series(&result.columns, &result.rows, 50))
                    .unwrap_or_default();
                if series.is_empty() {
                    self.show_toast("The result has no numeric column to chart.");
                    return;
                }
                *self.chart_data.borrow_mut() = series;
                self.chart_area.queue_draw();
                self.editor_results_stack.set_visible_child_name("chart");
                self.chart_button.set_label("Table");
            }
            AppMsg::CompareResults => {
                let Some(current) = self.editor_last_result.clone() else {
                    self.show_toast("Run a statement before comparing.");
                    return;
                };
                match self.editor_baseline.take() {
                    None => {
                        self.editor_baseline = Some(current);
                        self.editor_status
                            .set_label("Baseline captured. Run the statement again, then compare.");
                        self.show_toast("Baseline captured.");
                    }
                    Some(baseline) => {
                        let diff = diff_results(&baseline, &current, 0, 50);
                        let mut lines = vec![format!(
                            "Added: {}  Removed: {}  Changed: {}  Unchanged: {}",
                            diff.added, diff.removed, diff.changed, diff.unchanged
                        )];
                        for row in &diff.details {
                            let before = row
                                .before
                                .iter()
                                .map(unidb_app::value_text)
                                .collect::<Vec<String>>()
                                .join(", ");
                            let after = row
                                .after
                                .iter()
                                .map(unidb_app::value_text)
                                .collect::<Vec<String>>()
                                .join(", ");
                            lines.push(format!(
                                "{} key={} before=[{}] after=[{}]",
                                row.change.label(),
                                row.key,
                                before,
                                after
                            ));
                        }
                        self.plan_buffer.set_text(&lines.join("\n"));
                        self.plan_revealer.set_reveal_child(true);
                        if diff.has_changes() {
                            self.editor_status.set_label(&format!(
                                "Diff: {} added, {} removed, {} changed.",
                                diff.added, diff.removed, diff.changed
                            ));
                            self.show_toast("Comparison complete.");
                        } else {
                            self.editor_status.set_label("No differences.");
                            self.show_toast("No differences.");
                        }
                    }
                }
            }
            AppMsg::QueryFailed(message) => {
                self.editor_busy = false;
                self.update_controls();
                self.editor_error.set_label(&message);
                self.editor_error.set_visible(true);
                self.editor_status.set_label("Query failed.");
            }
            AppMsg::ClearResults => {
                self.clear_grid();
                self.editor_last_result = None;
                self.chart_data.borrow_mut().clear();
                self.editor_results_stack.set_visible_child_name("grid");
                self.chart_button.set_label("Chart");
                self.plan_revealer.set_reveal_child(false);
                self.plan_buffer.set_text("");
                self.editor_baseline = None;
                self.editor_status.set_label("Ready.");
                self.editor_error.set_visible(false);
                self.update_controls();
            }
            AppMsg::DesignerConnectionSelected(index) => {
                if let Some(id) = self.designer_ids.get(index as usize).copied() {
                    self.designer_connection = Some(id);
                    self.designer_database = self.designer_database_for(id);
                    self.designer_table = None;
                    self.designer_original.clear();
                    self.load_designer_tables(&sender);
                }
            }
            AppMsg::DesignerTables(tables) => {
                self.designer_tables = *tables;
                let mut names = vec!["New table".to_string()];
                names.extend(self.designer_tables.iter().cloned());
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                let model = gtk::StringList::new(&refs);
                self.designer_table_dropdown.set_model(Some(&model));
                let selected = self
                    .designer_table
                    .as_ref()
                    .and_then(|table| self.designer_tables.iter().position(|item| item == table))
                    .map(|position| position as u32 + 1)
                    .unwrap_or(0);
                self.designer_table_dropdown.set_selected(selected);
            }
            AppMsg::DesignerTableSelected(index) => {
                if index == 0 {
                    self.new_designer_table(&sender);
                    return;
                }
                let Some(table) = self.designer_tables.get(index as usize - 1).cloned() else {
                    return;
                };
                let Some(id) = self.designer_connection else {
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                let database = self.designer_database.clone();
                self.handle.spawn(async move {
                    match service.describe_table(id, &database, &table).await {
                        Ok(columns) => {
                            sender.input(AppMsg::DesignerDescribed(table, Box::new(columns)));
                        }
                        Err(error) => sender.input(AppMsg::DesignerFailed(format!(
                            "Could not read the table: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::DesignerDescribed(table, columns) => {
                self.designer_table = Some(table.clone());
                self.designer_name_entry.set_text(&table);
                self.designer_original = *columns;
                self.designer_columns = self
                    .designer_original
                    .iter()
                    .map(TableColumnInfo::to_design)
                    .collect();
                self.rebuild_designer_columns(&sender);
                self.designer_statements.clear();
                self.designer_preview_buffer.set_text("");
                self.designer_status.set_label(&format!(
                    "Loaded {} column{}.",
                    self.designer_columns.len(),
                    plural(self.designer_columns.len())
                ));
            }
            AppMsg::DesignerAddColumn => {
                self.sync_designer_columns();
                let name = format!("column_{}", self.designer_columns.len() + 1);
                self.designer_columns.push(ColumnDesign::new(name, "TEXT"));
                self.rebuild_designer_columns(&sender);
            }
            AppMsg::DesignerRemoveColumn(index) => {
                self.sync_designer_columns();
                if self.designer_columns.len() > 1 && index < self.designer_columns.len() {
                    self.designer_columns.remove(index);
                    self.rebuild_designer_columns(&sender);
                }
            }
            AppMsg::DesignerPreview => match self.preview_designer() {
                Ok(statements) => {
                    let count = statements.len();
                    self.designer_statements = statements;
                    self.designer_status
                        .set_label(&format!("Preview: {count} statement{}.", plural(count)));
                }
                Err(error) => self.show_toast(error.user_message()),
            },
            AppMsg::DesignerApply => {
                let statements = match self.preview_designer() {
                    Ok(statements) => statements,
                    Err(error) => {
                        self.show_toast(error.user_message());
                        return;
                    }
                };
                self.designer_preview_buffer.set_text("");
                for statement in &statements {
                    self.designer_preview_buffer
                        .insert_at_cursor(&format!("{statement};\n\n"));
                }
                let Some(id) = self.designer_connection else {
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                self.designer_statements = statements.clone();
                let count = statements.len();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.apply_statements(id, &statements).await {
                        Ok(()) => sender.input(AppMsg::DesignerApplied(count)),
                        Err(error) => sender.input(AppMsg::DesignerFailed(format!(
                            "Could not apply the change: {} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::DesignerApplied(count) => {
                self.designer_status
                    .set_label(&format!("Applied {count} statement{}", plural(count)));
                self.show_toast("Table change applied.");
                self.designer_statements.clear();
                if self.designer_table.is_none() {
                    let name = self.designer_name_entry.text().trim().to_string();
                    if !name.is_empty() {
                        self.designer_table = Some(name);
                    }
                }
                self.load_designer_tables(&sender);
            }
            AppMsg::DesignerFailed(message) => {
                self.designer_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::TransferConnectionSelected(index) => {
                if let Some(id) = self.transfer_ids.get(index as usize).copied() {
                    self.transfer_connection = Some(id);
                    self.transfer_database = self.designer_database_for(id);
                    self.load_transfer_tables(&sender);
                }
            }
            AppMsg::TransferTables(tables) => {
                self.transfer_tables = *tables;
                let refs: Vec<&str> = self.transfer_tables.iter().map(String::as_str).collect();
                let model = gtk::StringList::new(&refs);
                self.transfer_table_dropdown.set_model(Some(&model));
                self.transfer_table_dropdown.set_selected(0);
            }
            AppMsg::TransferTableSelected(index) => {
                if let Some(table) = self.transfer_tables.get(index as usize) {
                    self.transfer_status
                        .set_label(&format!("Ready to import into {table}."));
                }
            }
            AppMsg::TransferPathChanged(path) => self.transfer_path_entry.set_text(&path),
            AppMsg::TransferBrowse => {
                let dialog = gtk::FileDialog::builder()
                    .title("Choose a file to import")
                    .build();
                let sender = sender.clone();
                dialog.open(
                    relm4::main_application().active_window().as_ref(),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let Ok(file) = result
                            && let Some(path) = file.path()
                        {
                            sender.input(AppMsg::TransferPathChanged(path.display().to_string()));
                        }
                    },
                );
            }
            AppMsg::TransferImport => {
                let Some(connection_id) = self.transfer_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let Some(table) = self
                    .transfer_tables
                    .get(self.transfer_table_dropdown.selected() as usize)
                    .cloned()
                else {
                    self.show_toast("Choose a table first.");
                    return;
                };
                let path = self.transfer_path_entry.text().to_string();
                if path.trim().is_empty() {
                    self.show_toast("Choose a CSV or JSON file first.");
                    return;
                }
                let format = match self.transfer_format_dropdown.selected() {
                    1 => TransferFormat::Json,
                    _ => TransferFormat::Csv,
                };
                let options = ImportOptions {
                    database: self.transfer_database.clone(),
                    table,
                    path: std::path::PathBuf::from(path),
                    format,
                    has_headers: self.transfer_headers_check.is_active(),
                    delimiter: b',',
                    null_marker: None,
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.transfer_status.set_label("Importing...");
                self.handle.spawn(async move {
                    match service.import_file(connection_id, &options).await {
                        Ok(report) => sender.input(AppMsg::TransferFinished(Box::new(report))),
                        Err(error) => sender.input(AppMsg::TransferFailed(format!(
                            "Import failed: {} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::TransferFinished(report) => {
                let mut text = format!(
                    "Imported {} of {} rows ({} rejected, {} batch{}).",
                    report.rows_inserted,
                    report.rows_read,
                    report.rows_rejected,
                    report.batches,
                    plural(report.batches)
                );
                if let Some(warning) = report.warnings.first() {
                    text.push_str(&format!(" {warning}"));
                }
                self.transfer_status.set_label(&text);
                self.show_toast(&text);
            }
            AppMsg::TransferFailed(message) => {
                self.transfer_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::ExportQuery(format) => {
                let Some(connection_id) = self.editor_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let buffer = self.editor_view.buffer();
                let sql = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if sql.trim().is_empty() {
                    self.show_toast("Write a statement first.");
                    return;
                }
                let dialog = gtk::FileDialog::builder()
                    .title("Export result")
                    .initial_name(match format {
                        TransferFormat::Csv => "result.csv",
                        TransferFormat::Json => "result.json",
                    })
                    .build();
                let sender = sender.clone();
                dialog.save(
                    relm4::main_application().active_window().as_ref(),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        let Ok(file) = result else { return };
                        let Some(path) = file.path() else { return };
                        sender.input(AppMsg::ExportReady(
                            connection_id,
                            format,
                            path,
                            sql.clone(),
                        ));
                    },
                );
            }
            AppMsg::ExportQueryMasked(format) => {
                let Some(connection_id) = self.editor_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let buffer = self.editor_view.buffer();
                let sql = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if sql.trim().is_empty() {
                    self.show_toast("Write a statement first.");
                    return;
                }
                let dialog = gtk::FileDialog::builder()
                    .title("Export masked result")
                    .initial_name(match format {
                        TransferFormat::Csv => "result-masked.csv",
                        TransferFormat::Json => "result-masked.json",
                    })
                    .build();
                let sender = sender.clone();
                dialog.save(
                    relm4::main_application().active_window().as_ref(),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        let Ok(file) = result else { return };
                        let Some(path) = file.path() else { return };
                        sender.input(AppMsg::ExportReadyMasked(
                            connection_id,
                            format,
                            path,
                            sql.clone(),
                        ));
                    },
                );
            }
            AppMsg::ExportReadyMasked(connection_id, format, path, sql) => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service
                        .export_query_masked(connection_id, &sql, format, &path)
                        .await
                    {
                        Ok(report) => sender.input(AppMsg::Toast(format!(
                            "Exported {} masked rows to {}",
                            report.rows,
                            report.path.display()
                        ))),
                        Err(error) => sender.input(AppMsg::Toast(format!(
                            "Masked export failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::LoadHistory => self.load_history(&sender),
            AppMsg::HistoryLoaded(entries) => {
                self.history_entries = *entries;
                self.populate_history(&sender);
                self.history_status.set_label(&format!(
                    "{} statement{}.",
                    self.history_entries.len(),
                    plural(self.history_entries.len())
                ));
            }
            AppMsg::SavedQueriesLoaded(queries) => {
                self.populate_saved(&queries, &sender);
            }
            AppMsg::ClearHistoryRequested => {
                let dialog = adw::AlertDialog::builder()
                    .heading("Clear the query history?")
                    .body("Every recorded statement is deleted. Saved queries stay.")
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("clear", "Clear history");
                dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
                let sender = sender.clone();
                dialog.choose(
                    relm4::main_application().active_window().as_ref(),
                    None::<&gtk::gio::Cancellable>,
                    move |response| {
                        if response == "clear" {
                            sender.input(AppMsg::ClearHistoryConfirmed);
                        }
                    },
                );
            }
            AppMsg::ClearHistoryConfirmed => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.clear_history().await {
                        Ok(count) => {
                            sender.input(AppMsg::Toast(format!("Cleared {count} history rows.")));
                            sender.input(AppMsg::LoadHistory);
                        }
                        Err(error) => sender.input(AppMsg::HistoryFailed(format!(
                            "Could not clear history: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SaveCurrentQuery => {
                let name = self.saved_name_entry.text().trim().to_string();
                if name.is_empty() {
                    self.show_toast("Give the query a name first.");
                    return;
                }
                let buffer = self.editor_view.buffer();
                let sql = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if sql.trim().is_empty() {
                    self.show_toast("The editor is empty.");
                    return;
                }
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.save_saved_query(&name, &sql, None).await {
                        Ok(_) => sender.input(AppMsg::SavedQuerySaved),
                        Err(error) => sender.input(AppMsg::HistoryFailed(format!(
                            "Could not save the query: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SavedQuerySaved => {
                self.saved_name_entry.set_text("");
                self.show_toast("Query saved.");
                self.load_history(&sender);
            }
            AppMsg::LoadSql(sql) => {
                let buffer = self.editor_view.buffer();
                buffer.set_text(&sql);
                self.select_section(2);
                self.show_toast("Loaded into the editor.");
            }
            AppMsg::DeleteSavedQuery(id) => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.delete_saved_query(id).await {
                        Ok(_) => {
                            sender.input(AppMsg::Toast("Saved query deleted.".to_string()));
                            sender.input(AppMsg::LoadHistory);
                        }
                        Err(error) => sender.input(AppMsg::HistoryFailed(format!(
                            "Could not delete the query: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::HistoryFailed(message) => {
                self.history_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::SettingsLoaded(scheme) => {
                let index = match scheme.as_deref() {
                    Some("light") => 1,
                    Some("dark") => 2,
                    _ => 0,
                };
                self.settings_combo.set_selected(index);
                self.apply_color_scheme(index);
                self.settings_loaded = true;
            }
            AppMsg::SetColorScheme(index) => {
                if !self.settings_loaded {
                    return;
                }
                self.apply_color_scheme(index);
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let value = match index {
                    1 => "light",
                    2 => "dark",
                    _ => "system",
                };
                let sender = sender.clone();
                self.handle.spawn(async move {
                    if let Err(error) = service
                        .set_setting("color_scheme", serde_json::json!(value))
                        .await
                    {
                        sender.input(AppMsg::HistoryFailed(format!(
                            "Could not save the setting: {}",
                            error.user_message()
                        )));
                    }
                });
            }
            AppMsg::CheckUpdates => {
                let Some(feed) = self.update_feed.clone() else {
                    self.show_toast("Set UNIDB_UPDATE_FEED to enable the update check.");
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.update_status.set_label("Checking...");
                self.update_button.set_sensitive(false);
                self.handle.spawn(async move {
                    match service.check_updates(&feed).await {
                        Ok(Some(release)) => {
                            sender.input(AppMsg::UpdateResult(Some(Box::new(release))));
                        }
                        Ok(None) => sender.input(AppMsg::UpdateResult(None)),
                        Err(error) => sender.input(AppMsg::UpdateFailed(format!(
                            "Update check failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::UpdateResult(release) => match release {
                Some(release) => {
                    let text = format!(
                        "UniDB {} is available: {}",
                        release.version,
                        release.url.as_deref().unwrap_or("the release page")
                    );
                    self.update_status.set_label(&text);
                    self.update_button.set_sensitive(true);
                    self.show_toast(&text);
                }
                None => {
                    let text = format!("UniDB {} is up to date.", env!("CARGO_PKG_VERSION"));
                    self.update_status.set_label(&text);
                    self.update_button.set_sensitive(true);
                    self.show_toast(&text);
                }
            },
            AppMsg::UpdateFailed(message) => {
                self.update_status.set_label(&message);
                self.update_button.set_sensitive(true);
                self.show_toast(&message);
            }
            AppMsg::SchemaRefresh => {
                self.load_schema(&sender);
            }
            AppMsg::SchemaConnectionSelected(index) => {
                if let Some(id) = self.schema_ids.get(index as usize).copied() {
                    self.schema_connection = Some(id);
                    self.schema_database = self.designer_database_for(id);
                    self.load_schema(&sender);
                }
            }
            AppMsg::SchemaLoaded(tables) => {
                let count = tables.len();
                if count == 0 {
                    self.schema_status
                        .set_label("No tables in this database to draw.");
                    self.schema_tables.borrow_mut().clear();
                    self.schema_canvas.set_size_request(800, 200);
                    self.schema_canvas.queue_draw();
                } else {
                    let layout = erd_layout(&tables, 1000.0);
                    self.schema_status.set_label(&format!(
                        "{} table{}, {} relationship{}.",
                        count,
                        plural(count),
                        layout.edges.len(),
                        plural(layout.edges.len())
                    ));
                    self.schema_canvas
                        .set_size_request(layout.width as i32, layout.height as i32);
                    *self.schema_tables.borrow_mut() = *tables;
                    self.schema_canvas.queue_draw();
                }
            }
            AppMsg::SchemaFailed(message) => {
                self.schema_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::BuilderConnectionSelected(index) => {
                if let Some(id) = self.builder_ids.get(index as usize).copied() {
                    self.builder_connection = Some(id);
                    self.builder_database = self.designer_database_for(id);
                    self.load_builder_tables(&sender);
                }
            }
            AppMsg::BuilderTables(tables) => {
                self.builder_tables = *tables;
                let refs: Vec<&str> = self.builder_tables.iter().map(String::as_str).collect();
                self.builder_table_dropdown
                    .set_model(Some(&gtk::StringList::new(&refs)));
                self.builder_table_dropdown.set_selected(0);
            }
            AppMsg::BuilderTableSelected(index) => {
                let Some(table) = self.builder_tables.get(index as usize).cloned() else {
                    return;
                };
                self.builder_table = Some(table.clone());
                let Some(id) = self.builder_connection else {
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                let database = self.builder_database.clone();
                self.handle.spawn(async move {
                    match service.describe_table(id, &database, &table).await {
                        Ok(columns) => sender.input(AppMsg::BuilderDescribed(Box::new(columns))),
                        Err(error) => sender.input(AppMsg::BuilderFailed(format!(
                            "Could not read the columns: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::BuilderDescribed(columns) => {
                self.builder_columns = columns.iter().map(|column| column.name.clone()).collect();
                self.builder_selected = self.builder_columns.clone();
                self.builder_filters.clear();
                let column_refs: Vec<&str> =
                    self.builder_columns.iter().map(String::as_str).collect();
                self.builder_filter_column
                    .set_model(Some(&gtk::StringList::new(&column_refs)));
                self.builder_order_dropdown
                    .set_model(Some(&gtk::StringList::new(&column_refs)));
                self.builder_filter_column.set_selected(0);
                self.builder_order_dropdown.set_selected(0);
                self.rebuild_builder_columns(&sender);
                self.populate_builder_filters(&sender);
                self.builder_status.set_label(&format!(
                    "{} column{} available.",
                    self.builder_columns.len(),
                    plural(self.builder_columns.len())
                ));
            }
            AppMsg::BuilderToggleColumn(name, active) => {
                if active {
                    if !self.builder_selected.contains(&name) {
                        self.builder_selected.push(name);
                        self.builder_selected.sort_by_key(|column| {
                            self.builder_columns.iter().position(|item| item == column)
                        });
                    }
                } else {
                    self.builder_selected.retain(|column| *column != name);
                }
            }
            AppMsg::BuilderAddFilter => {
                let column = self
                    .builder_columns
                    .get(self.builder_filter_column.selected() as usize)
                    .cloned();
                let Some(column) = column else {
                    return;
                };
                let operator = FilterOp::ALL
                    .get(self.builder_filter_op.selected() as usize)
                    .copied()
                    .unwrap_or(FilterOp::Equals);
                let value = self.builder_filter_value.text().to_string();
                self.builder_filters.push(QueryFilter {
                    column,
                    operator,
                    value,
                });
                self.builder_filter_value.set_text("");
                self.populate_builder_filters(&sender);
            }
            AppMsg::BuilderRemoveFilter(index) => {
                if index < self.builder_filters.len() {
                    self.builder_filters.remove(index);
                    self.populate_builder_filters(&sender);
                }
            }
            AppMsg::BuilderPreview => match self.preview_builder() {
                Ok(sql) => {
                    self.builder_preview_buffer.set_text(&sql);
                    self.builder_status.set_label("Preview ready.");
                }
                Err(error) => {
                    self.builder_status.set_label(error.user_message());
                    self.show_toast(error.user_message());
                }
            },
            AppMsg::BuilderOpenInEditor => match self.preview_builder() {
                Ok(sql) => {
                    let buffer = self.editor_view.buffer();
                    buffer.set_text(&sql);
                    self.select_section(2);
                    self.show_toast("Query loaded into the editor.");
                }
                Err(error) => {
                    self.builder_status.set_label(error.user_message());
                    self.show_toast(error.user_message());
                }
            },
            AppMsg::BuilderFailed(message) => {
                self.builder_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::GenerateFakeData => {
                let Some(connection_id) = self.transfer_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let Some(table) = self
                    .transfer_tables
                    .get(self.transfer_table_dropdown.selected() as usize)
                    .cloned()
                else {
                    self.show_toast("Choose a table first.");
                    return;
                };
                let rows = self.fake_count_spin.value_as_int().max(1) as usize;
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_nanos() as u64)
                    .unwrap_or(1);
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                let database = self.transfer_database.clone();
                self.transfer_status.set_label("Generating fake rows...");
                self.handle.spawn(async move {
                    match service
                        .generate_fake_rows(connection_id, &database, &table, rows, seed)
                        .await
                    {
                        Ok(count) => sender.input(AppMsg::FakeRowsGenerated(count)),
                        Err(error) => sender.input(AppMsg::FakeRowsFailed(format!(
                            "Could not generate fake rows: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::FakeRowsGenerated(count) => {
                let message = format!("Generated {count} fake rows.");
                self.transfer_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::FakeRowsFailed(message) => {
                self.transfer_status.set_label(&message);
                self.show_toast(&message);
            }
            AppMsg::ExportReady(connection_id, format, path, sql) => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service
                        .export_query(connection_id, &sql, format, &path)
                        .await
                    {
                        Ok(report) => sender.input(AppMsg::Toast(format!(
                            "Exported {} rows to {}",
                            report.rows,
                            report.path.display()
                        ))),
                        Err(error) => sender.input(AppMsg::Toast(format!(
                            "Export failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::MigrationsConnChanged(index) => {
                if let Some(id) = self.migrations_ids.get(index) {
                    self.migrations_connection = Some(*id);
                    self.scan_migrations(&sender);
                }
            }
            AppMsg::MigrationsScan => {
                self.scan_migrations(&sender);
            }
            AppMsg::MigrationsScanned(boxed) => {
                let (files, applied, warnings, git_status) = *boxed;
                self.migrations_files = files;
                self.migrations_applied = applied;
                self.migrations_warnings = warnings;
                self.migrations_git_status = git_status;
                self.populate_migrations_list();
            }
            AppMsg::MigrationsScanFailed(error) => {
                self.migrations_status.set_label(&error);
                self.show_toast(&error);
            }
            AppMsg::MigrationsSelectFile(index) => {
                if let Some(file) = self.migrations_files.get(index) {
                    self.migrations_preview_buffer.set_text(&file.sql);
                }
            }
            AppMsg::MigrationsApply => {
                let Some(connection_id) = self.migrations_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let Some(service) = &self.service else {
                    return;
                };
                let dir_str = self.migrations_dir_entry.text().to_string();
                let dir = std::path::PathBuf::from(dir_str);
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.migrations_status
                    .set_label("Applying pending migrations...");
                self.handle.spawn(async move {
                    match service.apply_migrations(connection_id, &dir).await {
                        Ok(report) => sender.input(AppMsg::MigrationsApplied(Box::new(report))),
                        Err(error) => sender.input(AppMsg::MigrationsApplyFailed(format!(
                            "Migration failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::MigrationsApplied(report) => {
                let msg = format!(
                    "Applied {} migrations, skipped {}",
                    report.applied.len(),
                    report.skipped
                );
                self.migrations_status.set_label(&msg);
                self.show_toast(&msg);
                self.scan_migrations(&sender);
            }
            AppMsg::MigrationsApplyFailed(error) => {
                self.migrations_status.set_label(&error);
                self.show_toast(&error);
            }
            AppMsg::SnapshotsConnChanged(index) => {
                if let Some(id) = self.snapshots_ids.get(index) {
                    self.snapshots_connection = Some(*id);
                    self.update_controls();
                    self.load_snapshots(&sender);
                }
            }
            AppMsg::SnapshotsRefresh => {
                self.load_snapshots(&sender);
            }
            AppMsg::SnapshotsLoaded(items) => {
                self.snapshots_items = items;
                self.populate_snapshots(&sender);
                self.snapshots_status.set_label(&format!(
                    "{} snapshot{} stored.",
                    self.snapshots_items.len(),
                    plural(self.snapshots_items.len())
                ));
            }
            AppMsg::SnapshotsFailed(error) => {
                self.snapshots_status.set_label(&error);
                self.show_toast(&error);
            }
            AppMsg::TakeSnapshot => {
                let Some(connection_id) = self.snapshots_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let table = self.snapshots_table_entry.text().trim().to_string();
                if table.is_empty() {
                    self.show_toast("Enter a table name first.");
                    return;
                }
                let note_text = self.snapshots_note_entry.text().trim().to_string();
                let note = if note_text.is_empty() {
                    None
                } else {
                    Some(note_text)
                };
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.snapshots_status
                    .set_label(&format!("Capturing snapshot for {table}..."));
                self.handle.spawn(async move {
                    match service
                        .create_snapshot(connection_id, "main", &table, note)
                        .await
                    {
                        Ok(snapshot) => sender.input(AppMsg::SnapshotTaken(Box::new(snapshot))),
                        Err(error) => sender.input(AppMsg::SnapshotsFailed(format!(
                            "Snapshot failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SnapshotTaken(snapshot) => {
                self.snapshots_note_entry.set_text("");
                self.show_toast(&format!(
                    "Snapshot captured for {} ({} rows).",
                    snapshot.table_name, snapshot.row_count
                ));
                self.load_snapshots(&sender);
            }
            AppMsg::CompareSnapshot(id) => {
                let Some(connection_id) = self.snapshots_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let table_name = self
                    .snapshots_items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.table_name.clone())
                    .unwrap_or_else(|| "table".to_string());
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.snapshots_status
                    .set_label(&format!("Comparing snapshot for {table_name}..."));
                self.handle.spawn(async move {
                    match service.diff_snapshot(connection_id, id).await {
                        Ok(diff) => {
                            sender.input(AppMsg::SnapshotDiffed(Box::new((table_name, diff))))
                        }
                        Err(error) => sender.input(AppMsg::SnapshotsFailed(format!(
                            "Comparison failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SnapshotDiffed(boxed) => {
                let (table, diff) = *boxed;
                let summary = format!(
                    "Comparison for {table}: {} unchanged, {} changed, {} added, {} removed",
                    diff.unchanged, diff.changed, diff.added, diff.removed
                );
                self.snapshots_status.set_label(&summary);
                let mut lines = vec![summary.clone()];
                for row in &diff.details {
                    let before = row
                        .before
                        .iter()
                        .map(unidb_app::value_text)
                        .collect::<Vec<String>>()
                        .join(", ");
                    let after = row
                        .after
                        .iter()
                        .map(unidb_app::value_text)
                        .collect::<Vec<String>>()
                        .join(", ");
                    lines.push(format!(
                        "{} key={} before=[{}] after=[{}]",
                        row.change.label(),
                        row.key,
                        before,
                        after
                    ));
                }
                self.plan_buffer.set_text(&lines.join("\n"));
                self.plan_revealer.set_reveal_child(true);
                self.show_toast(&summary);
            }
            AppMsg::RestoreSnapshot(id) => {
                let Some(connection_id) = self.snapshots_connection else {
                    self.show_toast("Choose a live connection first.");
                    return;
                };
                let table_name = self
                    .snapshots_items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.table_name.clone())
                    .unwrap_or_else(|| "table".to_string());
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.snapshots_status
                    .set_label(&format!("Restoring {table_name} from snapshot..."));
                self.handle.spawn(async move {
                    match service.restore_snapshot(connection_id, id).await {
                        Ok(count) => sender.input(AppMsg::SnapshotRestored(count, table_name)),
                        Err(error) => sender.input(AppMsg::SnapshotsFailed(format!(
                            "Restore failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SnapshotRestored(count, table) => {
                let msg = format!("Restored {count} rows to {table}.");
                self.snapshots_status.set_label(&msg);
                self.show_toast(&msg);
            }
            AppMsg::DeleteSnapshot(id) => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.delete_snapshot(id).await {
                        Ok(_) => sender.input(AppMsg::SnapshotDeleted),
                        Err(error) => sender.input(AppMsg::SnapshotsFailed(format!(
                            "Delete failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::SnapshotDeleted => {
                self.show_toast("Snapshot deleted.");
                self.load_snapshots(&sender);
            }
            AppMsg::RunMultiQuery => {
                if self.editor_busy {
                    return;
                }
                let ids: Vec<Uuid> = self
                    .connections
                    .iter()
                    .filter(|profile| self.connected.contains(&profile.id))
                    .map(|profile| profile.id)
                    .collect();
                if ids.is_empty() {
                    self.show_toast("Connect to at least one database first.");
                    return;
                }
                let buffer = self.editor_view.buffer();
                let sql = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if sql.trim().is_empty() {
                    self.show_toast("Write a statement first.");
                    return;
                }
                let Some(service) = &self.service else {
                    return;
                };
                self.editor_busy = true;
                self.editor_error.set_visible(false);
                self.editor_status
                    .set_label(&format!("Running across {} live connections...", ids.len()));
                self.update_controls();
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.execute_multi(&ids, &sql).await {
                        Ok((summaries, merged)) => {
                            sender.input(AppMsg::MultiQueryFinished(summaries, Box::new(merged)))
                        }
                        Err(error) => sender.input(AppMsg::QueryFailed(format!(
                            "{} ({})",
                            error.user_message(),
                            error.code()
                        ))),
                    }
                });
            }
            AppMsg::MultiQueryFinished(summaries, merged) => {
                self.editor_busy = false;
                self.render_result(&merged);
                self.editor_last_result = Some((*merged).clone());
                self.editor_results_stack.set_visible_child_name("grid");
                self.chart_button.set_label("Chart");

                let mut plan_lines = vec![format!(
                    "Multi database execution across {} targets:",
                    summaries.len()
                )];
                for s in &summaries {
                    match &s.error {
                        None => plan_lines.push(format!(
                            "OK   {} ({})  |  {} rows  |  {} ms",
                            s.connection_name, s.engine, s.row_count, s.elapsed_ms
                        )),
                        Some(err) => plan_lines.push(format!(
                            "ERR  {} ({})  |  {}  |  {} ms",
                            s.connection_name, s.engine, err, s.elapsed_ms
                        )),
                    }
                }
                self.plan_buffer.set_text(&plan_lines.join("\n"));
                self.plan_revealer.set_reveal_child(true);
                self.editor_status.set_label(&format!(
                    "Multi query: {} rows across {} connections.",
                    merged.rows.len(),
                    summaries.len()
                ));
                self.update_controls();
            }
            AppMsg::BundlePathChanged(path) => {
                self.bundle_path_entry.set_text(&path);
            }
            AppMsg::BundleBrowse => {
                let dialog = gtk::FileDialog::builder()
                    .title("Choose a team bundle JSON file")
                    .build();
                let sender = sender.clone();
                dialog.open(
                    relm4::main_application().active_window().as_ref(),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let Ok(file) = result
                            && let Some(path) = file.path()
                        {
                            sender.input(AppMsg::BundlePathChanged(path.display().to_string()));
                        }
                    },
                );
            }
            AppMsg::ExportTeamBundle => {
                let Some(service) = &self.service else {
                    return;
                };
                let raw_path = self.bundle_path_entry.text().trim().to_string();
                let path = if raw_path.is_empty() {
                    let default_path = std::env::temp_dir().join("unidb-team-bundle.json");
                    self.bundle_path_entry
                        .set_text(&default_path.display().to_string());
                    default_path
                } else {
                    std::path::PathBuf::from(raw_path)
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.bundle_status
                    .set_label("Exporting signed team bundle...");
                self.handle.spawn(async move {
                    match service
                        .export_team_bundle("Workspace Team", "UniDB", &path)
                        .await
                    {
                        Ok((bundle, _sig_path)) => {
                            let fp = bundle.signature_sha256.chars().take(12).collect::<String>();
                            sender.input(AppMsg::TeamBundleExported(
                                path,
                                bundle.connections.len(),
                                bundle.saved_queries.len(),
                                fp,
                            ))
                        }
                        Err(error) => sender.input(AppMsg::TeamBundleFailed(format!(
                            "Bundle export failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::TeamBundleExported(path, conns, queries, fingerprint) => {
                let msg = format!(
                    "Exported {} connection{}, {} quer{} to {} (sig {}...)",
                    conns,
                    plural(conns),
                    queries,
                    if queries == 1 { "y" } else { "ies" },
                    path.display(),
                    fingerprint
                );
                self.bundle_status.set_label(&msg);
                self.show_toast("Team bundle and .sig exported (no passwords included).");
            }
            AppMsg::InspectTeamBundle => {
                let Some(service) = &self.service else {
                    return;
                };
                let raw_path = self.bundle_path_entry.text().trim().to_string();
                if raw_path.is_empty() {
                    self.show_toast("Enter a team bundle path first.");
                    return;
                }
                let path = std::path::PathBuf::from(raw_path);
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.bundle_status.set_label("Verifying team bundle...");
                self.handle.spawn(async move {
                    match service.inspect_team_bundle(&path).await {
                        Ok(preview) => {
                            sender.input(AppMsg::TeamBundleInspected(path, Box::new(preview)))
                        }
                        Err(error) => sender.input(AppMsg::TeamBundleFailed(format!(
                            "Bundle inspection failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::TeamBundleInspected(path, preview) => {
                let status_line = format!(
                    "{}: {} connections, {} queries | {}",
                    preview.bundle.bundle_name,
                    preview.bundle.connections.len(),
                    preview.bundle.saved_queries.len(),
                    preview.verification.message
                );
                self.bundle_status.set_label(&status_line);
                let body = format!(
                    "Author: {}\nConnections: {} (credentials excluded)\nSaved queries: {}\nSignature: {} ({})",
                    preview.bundle.created_by,
                    preview.bundle.connections.len(),
                    preview.bundle.saved_queries.len(),
                    if preview.verification.signature_valid {
                        "VERIFIED"
                    } else {
                        "INVALID"
                    },
                    preview.verification.fingerprint
                );
                let dialog = adw::AlertDialog::builder()
                    .heading(format!("Team bundle: {}", preview.bundle.bundle_name))
                    .body(body)
                    .build();
                dialog.add_response("cancel", "Cancel");
                if preview.verification.signature_valid {
                    dialog.add_response("import", "Import bundle");
                    dialog.set_response_appearance("import", adw::ResponseAppearance::Suggested);
                }
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let sender = sender.clone();
                dialog.connect_response(None, move |_, response| {
                    if response == "import" {
                        sender.input(AppMsg::ConfirmImportTeamBundle(path.clone()));
                    }
                });
                if let Some(window) = relm4::main_application().active_window() {
                    dialog.present(Some(&window));
                }
            }
            AppMsg::ConfirmImportTeamBundle(path) => {
                let Some(service) = &self.service else {
                    return;
                };
                let service = Arc::clone(service);
                let sender = sender.clone();
                self.handle.spawn(async move {
                    match service.import_team_bundle(&path, true).await {
                        Ok(report) => sender.input(AppMsg::TeamBundleImported(Box::new(report))),
                        Err(error) => sender.input(AppMsg::TeamBundleFailed(format!(
                            "Bundle import failed: {}",
                            error.user_message()
                        ))),
                    }
                });
            }
            AppMsg::TeamBundleImported(report) => {
                let msg = format!(
                    "Imported {} connection{}, {} skipped, {} quer{}.",
                    report.connections_imported,
                    plural(report.connections_imported),
                    report.connections_skipped,
                    report.queries_imported,
                    if report.queries_imported == 1 {
                        "y"
                    } else {
                        "ies"
                    }
                );
                self.bundle_status.set_label(&msg);
                self.show_toast(&msg);
                self.load_connections(&sender);
            }
            AppMsg::TeamBundleFailed(error) => {
                self.bundle_status.set_label(&error);
                self.show_toast(&error);
            }
        }
    }
}

impl AppModel {
    fn show_toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    fn show_diagnostic(&self, message: &str) {
        self.diagnostic_page.set_description(Some(message));
        self.root_stack.set_visible_child_name("diagnostic");
    }

    fn select_section(&mut self, index: usize) {
        self.section = index;
        let (name, title) = match index {
            0 => ("connections", "Connections"),
            1 => ("navigator", "Navigator"),
            2 => ("editor", "Editor"),
            3 => ("designer", "Designer"),
            4 => ("transfer", "Transfer"),
            5 => ("history", "History"),
            6 => ("settings", "Settings"),
            7 => ("schema", "Schema"),
            8 => ("builder", "Builder"),
            9 => ("migrations", "Migrations"),
            _ => ("security", "Security"),
        };
        self.page_stack.set_visible_child_name(name);
        self.content_page.set_title(title);
        self.window_title.set_subtitle(title);
        if let Some(row) = self.sidebar.row_at_index(index as i32) {
            if self.sidebar.selected_row().as_ref() != Some(&row) {
                self.sidebar.select_row(Some(&row));
            }
        }
    }

    fn update_controls(&self) {
        let ready = self.service.is_some();
        self.backup_button.set_sensitive(ready && !self.busy);
        self.refresh_button.set_sensitive(ready && !self.busy);
        self.save_button.set_sensitive(ready && !self.busy);
        self.test_button.set_sensitive(ready && !self.busy);
        self.run_button.set_sensitive(ready && !self.editor_busy);
        self.run_multi_button
            .set_sensitive(ready && !self.editor_busy && !self.connected.is_empty());
        self.cancel_button.set_sensitive(ready && self.editor_busy);
        self.clear_button.set_sensitive(true);
        self.explain_button
            .set_sensitive(ready && !self.editor_busy);
        self.chart_button
            .set_sensitive(self.editor_last_result.is_some());
        self.migrations_scan_button
            .set_sensitive(ready && !self.busy);
        self.migrations_apply_button
            .set_sensitive(ready && !self.busy);
        self.snapshots_create_button
            .set_sensitive(ready && !self.busy && self.snapshots_connection.is_some());
        self.snapshots_refresh_button
            .set_sensitive(ready && !self.busy && self.snapshots_connection.is_some());
        self.bundle_export_button.set_sensitive(ready && !self.busy);
        self.bundle_inspect_button
            .set_sensitive(ready && !self.busy);
    }

    fn load_connections(&self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.list_connections().await {
                Ok(profiles) => sender.input(AppMsg::ConnectionsLoaded(Box::new(profiles))),
                Err(error) => sender.input(AppMsg::ConnectionsFailed(format!(
                    "{} ({})",
                    error.user_message(),
                    error.code()
                ))),
            }
        });
    }

    fn form_values(&self) -> Option<FormValues> {
        let engine_index = self.engine_combo.selected() as usize;
        let engine = *Engine::ALL.get(engine_index)?;
        Some(FormValues {
            name: self.name_entry.text().to_string(),
            engine,
            host: self.host_entry.text().to_string(),
            port: self.port_spin.value() as u16,
            database: self.database_entry.text().to_string(),
            username: self.username_entry.text().to_string(),
            password: self.password_entry.text().to_string(),
            ssl_mode: tls_from_index(self.tls_combo.selected()),
            read_only: self.readonly_switch.is_active(),
            url_mode: self.url_switch.is_active(),
            url_text: self.url_entry.text().to_string(),
            editing: self.editing,
        })
    }

    fn build_profile(
        &self,
        values: &FormValues,
    ) -> Result<(ConnectionProfile, Option<String>), unidb_core::UniDbError> {
        let (mut profile, password) = if values.url_mode && !values.url_text.trim().is_empty() {
            let parsed = ConnectionProfile::from_url(values.name.clone(), &values.url_text)?;
            (parsed.profile, parsed.password)
        } else {
            let mut profile = ConnectionProfile::new(
                values.name.clone(),
                values.engine,
                values.host.clone(),
                values.database.clone(),
            );
            profile.port = values.port;
            profile.username = values.username.clone();
            profile.ssl_mode = values.ssl_mode;
            profile.read_only = values.read_only;
            (profile, None)
        };
        if let Some(id) = values.editing {
            profile.id = id;
        }
        profile.validate()?;
        let password = password.or_else(|| {
            if values.password.is_empty() {
                None
            } else {
                Some(values.password.clone())
            }
        });
        Ok((profile, password))
    }

    fn populate_connections(&mut self, sender: &ComponentSender<Self>) {
        for row in self.connection_rows.drain(..) {
            self.connections_group.remove(&row);
        }
        if self.connections.is_empty() {
            let row = adw::ActionRow::builder()
                .title("No connections yet")
                .subtitle("Add your first database connection to get started.")
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("network-server-symbolic"));
            self.connections_group.add(&row);
            self.connection_rows.push(row);
            return;
        }
        let profiles = self.connections.clone();
        for profile in profiles {
            let live = self.connected.contains(&profile.id);
            let location = if profile.host.is_empty() {
                "local file".to_string()
            } else {
                format!("{}:{}", profile.host, profile.port)
            };
            let scope = if profile.is_cloud() { "cloud" } else { "local" };
            let subtitle = format!(
                "{}  |  {}  |  {}  |  {}",
                profile.engine.display(),
                location,
                scope,
                profile.database
            );
            let row = adw::ActionRow::builder()
                .title(&profile.name)
                .subtitle(subtitle)
                .build();
            let icon = gtk::Image::from_icon_name("network-server-symbolic");
            icon.set_tooltip_text(Some(profile.engine.display()));
            row.add_prefix(&icon);

            let suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            if live {
                let badge = gtk::Label::new(Some("LIVE"));
                badge.add_css_class("status-pass");
                suffix.append(&badge);
            }
            let connect_button =
                gtk::Button::with_label(if live { "Disconnect" } else { "Connect" });
            if !live {
                connect_button.add_css_class("suggested-action");
            }
            let id = profile.id;
            {
                let sender = sender.clone();
                connect_button.connect_clicked(move |_| {
                    sender.input(if live {
                        AppMsg::Disconnect(id)
                    } else {
                        AppMsg::Connect(id)
                    });
                });
            }
            suffix.append(&connect_button);

            let menu_button = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text("Connection actions")
                .build();
            let popover = gtk::Popover::new();
            let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
            menu_box.set_margin_all(6);
            let edit_button = gtk::Button::with_label("Edit");
            {
                let sender = sender.clone();
                edit_button.connect_clicked(move |_| sender.input(AppMsg::EditConnection(id)));
            }
            let delete_button = gtk::Button::with_label("Delete");
            delete_button.add_css_class("destructive-action");
            {
                let sender = sender.clone();
                delete_button.connect_clicked(move |_| sender.input(AppMsg::DeleteConnection(id)));
            }
            menu_box.append(&edit_button);
            menu_box.append(&delete_button);
            popover.set_child(Some(&menu_box));
            menu_button.set_popover(Some(&popover));
            suffix.append(&menu_button);
            row.add_suffix(&suffix);

            self.connections_group.add(&row);
            self.connection_rows.push(row);
        }
    }

    fn open_form(&mut self, profile: Option<&ConnectionProfile>) {
        self.editing = profile.map(|item| item.id);
        self.details_group.set_title(if profile.is_some() {
            "Edit connection"
        } else {
            "New connection"
        });
        self.name_entry
            .set_text(profile.map_or("", |item| item.name.as_str()));
        self.engine_combo.set_selected(
            profile
                .map(|item| {
                    Engine::ALL
                        .iter()
                        .position(|engine| *engine == item.engine)
                        .unwrap_or(0) as u32
                })
                .unwrap_or(0),
        );
        self.host_entry
            .set_text(profile.map_or("", |item| item.host.as_str()));
        self.port_spin
            .set_value(f64::from(profile.map_or(5432, |item| item.port)));
        self.database_entry
            .set_text(profile.map_or("", |item| item.database.as_str()));
        self.username_entry
            .set_text(profile.map_or("", |item| item.username.as_str()));
        self.password_entry.set_text("");
        self.tls_combo.set_selected(tls_index(
            profile.map_or(SslMode::Prefer, |item| item.ssl_mode),
        ));
        self.readonly_switch
            .set_active(profile.is_some_and(|item| item.read_only));
        self.url_switch.set_active(false);
        self.url_entry.set_text("");
        self.connections_stack.set_visible_child_name("form");
    }

    fn refresh_navigator_connections(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.navigator_ids = live.iter().map(|profile| profile.id).collect();
        for row in drain_rows(&self.navigator_databases_list) {
            self.navigator_databases_list.remove(&row);
        }
        for row in drain_rows(&self.navigator_tables_list) {
            self.navigator_tables_list.remove(&row);
        }
        if live.is_empty() {
            self.navigator_connection = None;
            self.navigator_database = None;
            self.navigator_combo
                .set_model(Some(&gtk::StringList::new(&[] as &[&str])));
            self.navigator_stack.set_visible_child_name("empty");
            return;
        }
        self.navigator_stack.set_visible_child_name("browse");
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        self.navigator_combo
            .set_model(Some(&gtk::StringList::new(&names)));
        let index = if self
            .navigator_connection
            .is_some_and(|current| self.navigator_ids.contains(&current))
        {
            self.navigator_ids
                .iter()
                .position(|id| Some(*id) == self.navigator_connection)
                .unwrap_or(0) as u32
        } else {
            0
        };
        self.navigator_combo.set_selected(index);
        if let Some(id) = self.navigator_ids.get(index as usize).copied() {
            self.navigator_connection = Some(id);
            self.load_navigator_databases(id, sender);
        }
    }

    fn load_navigator_databases(&mut self, id: Uuid, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.list_databases(id).await {
                Ok(databases) => {
                    sender.input(AppMsg::NavigatorDatabases(id, Box::new(databases)));
                }
                Err(error) => sender.input(AppMsg::NavigatorFailed(format!(
                    "Could not list databases: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn load_navigator_tables(&mut self, id: Uuid, database: &str, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        let database = database.to_string();
        self.handle.spawn(async move {
            match service.list_tables(id, &database).await {
                Ok(tables) => sender.input(AppMsg::NavigatorTables(database, Box::new(tables))),
                Err(error) => sender.input(AppMsg::NavigatorFailed(format!(
                    "Could not list tables: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn populate_databases(&mut self, databases: &[String], sender: &ComponentSender<Self>) {
        for row in drain_rows(&self.navigator_databases_list) {
            self.navigator_databases_list.remove(&row);
        }
        if databases.is_empty() {
            self.navigator_databases_list
                .append(&plain_row("No databases reported", "view-list-symbolic"));
        }
        for database in databases {
            let row = plain_row(database, "drive-harddisk-symbolic");
            let name = database.clone();
            let sender = sender.clone();
            self.navigator_databases_list
                .connect_row_activated(move |_, _| {
                    sender.input(AppMsg::NavigatorSelectDatabase(name.clone()));
                });
            self.navigator_databases_list.append(&row);
        }
    }

    fn populate_tables(&mut self, tables: &[String]) {
        for row in drain_rows(&self.navigator_tables_list) {
            self.navigator_tables_list.remove(&row);
        }
        if tables.is_empty() {
            self.navigator_tables_list.append(&plain_row(
                "No tables in this database",
                "view-list-symbolic",
            ));
        }
        for table in tables {
            self.navigator_tables_list
                .append(&plain_row(table, "x-office-spreadsheet-symbolic"));
        }
    }

    fn refresh_editor_connections(&mut self, _sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.editor_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        let model = gtk::StringList::new(&names);
        self.editor_dropdown.set_model(Some(&model));
        if live.is_empty() {
            self.editor_connection = None;
            self.editor_status.set_label("Connect to a database first.");
        } else {
            if self
                .editor_connection
                .is_none_or(|current| !self.editor_ids.contains(&current))
            {
                self.editor_connection = self.editor_ids.first().copied();
            }
            self.editor_dropdown.set_selected(0);
            self.editor_status.set_label("Ready to run a statement.");
        }
        self.update_controls();
    }

    fn run_query(&mut self, sender: &ComponentSender<Self>) {
        if self.editor_busy {
            return;
        }
        let Some(connection_id) = self.editor_connection else {
            self.show_toast("Choose a live connection first.");
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let buffer = self.editor_view.buffer();
        let sql = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if sql.trim().is_empty() {
            self.show_toast("Write a statement first.");
            return;
        }
        self.editor_busy = true;
        self.editor_error.set_visible(false);
        self.editor_status.set_label("Running...");
        self.update_controls();
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.execute(connection_id, &sql).await {
                Ok(result) => sender.input(AppMsg::QueryFinished(Box::new(result))),
                Err(error) => sender.input(AppMsg::QueryFailed(format!(
                    "{} ({})",
                    error.user_message(),
                    error.code()
                ))),
            }
        });
    }

    fn clear_grid(&self) {
        while let Some(child) = self.editor_grid.first_child() {
            self.editor_grid.remove(&child);
        }
    }

    fn render_result(&self, result: &unidb_core::QueryResult) {
        self.clear_grid();
        self.editor_error.set_visible(false);
        if result.columns.is_empty() {
            self.editor_status.set_label(&format!(
                "Statement completed. {} rows affected in {} ms.",
                result.affected_rows.unwrap_or(0),
                result.elapsed_ms
            ));
            return;
        }
        let shown = result.rows.len().min(500);
        for (column_index, column) in result.columns.iter().enumerate() {
            let header = gtk::Label::new(Some(&column.name));
            header.set_xalign(0.0);
            header.add_css_class("heading");
            header.set_tooltip_text(Some(&column.type_name));
            self.editor_grid
                .attach(&header, column_index as i32, 0, 1, 1);
        }
        for (row_index, row) in result.rows.iter().take(shown).enumerate() {
            for (column_index, value) in row.iter().enumerate() {
                let text = if value.is_null() {
                    "NULL".to_string()
                } else {
                    unidb_app::value_text(value)
                };
                let cell = gtk::Label::new(Some(&text));
                cell.set_xalign(0.0);
                cell.set_selectable(true);
                cell.add_css_class("unidb-mono");
                if value.is_null() {
                    cell.add_css_class("unidb-muted");
                }
                self.editor_grid
                    .attach(&cell, column_index as i32, (row_index + 1) as i32, 1, 1);
            }
        }
        let suffix = if result.rows.len() > shown {
            format!(" Showing the first {shown} of {} rows.", result.rows.len())
        } else {
            String::new()
        };
        self.editor_status.set_label(&format!(
            "{} row{}, {} column{} in {} ms.{}",
            result.rows.len(),
            plural(result.rows.len()),
            result.columns.len(),
            plural(result.columns.len()),
            result.elapsed_ms,
            suffix
        ));
    }

    fn designer_database_for(&self, id: Uuid) -> String {
        self.connections
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| {
                if profile.database.is_empty() {
                    "main".to_string()
                } else {
                    profile.database.clone()
                }
            })
            .unwrap_or_else(|| "main".to_string())
    }

    fn designer_dialect(&self) -> Option<SqlDialect> {
        let id = self.designer_connection?;
        let engine = self
            .connections
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.engine)?;
        SqlDialect::for_engine(engine)
    }

    fn refresh_designer(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.designer_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        let model = gtk::StringList::new(&names);
        self.designer_conn_dropdown.set_model(Some(&model));
        if live.is_empty() {
            self.designer_connection = None;
            self.designer_stack.set_visible_child_name("empty");
            return;
        }
        let current = self
            .designer_connection
            .filter(|id| self.designer_ids.contains(id))
            .unwrap_or(self.designer_ids[0]);
        self.designer_connection = Some(current);
        self.designer_database = self.designer_database_for(current);
        let index = self
            .designer_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.designer_conn_dropdown.set_selected(index as u32);
        if self.designer_dialect().is_none() {
            self.designer_stack.set_visible_child_name("unsupported");
            return;
        }
        self.designer_stack.set_visible_child_name("form");
        if self.designer_columns.is_empty() {
            self.new_designer_table(sender);
        }
        self.load_designer_tables(sender);
    }

    fn load_designer_tables(&mut self, sender: &ComponentSender<Self>) {
        let Some(id) = self.designer_connection else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        let database = self.designer_database.clone();
        self.handle.spawn(async move {
            match service.list_tables(id, &database).await {
                Ok(tables) => sender.input(AppMsg::DesignerTables(Box::new(tables))),
                Err(error) => sender.input(AppMsg::DesignerFailed(format!(
                    "Could not list tables: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn new_designer_table(&mut self, sender: &ComponentSender<Self>) {
        self.designer_table = None;
        self.designer_original.clear();
        self.designer_name_entry.set_text("");
        let mut id = ColumnDesign::new("id", "INTEGER");
        id.primary_key = true;
        id.auto_increment = true;
        id.nullable = false;
        self.designer_columns = vec![id];
        self.designer_statements.clear();
        self.designer_preview_buffer.set_text("");
        self.designer_status.set_label("New table.");
        self.rebuild_designer_columns(sender);
    }

    fn sync_designer_columns(&mut self) {
        if self.designer_widgets.len() != self.designer_columns.len() {
            return;
        }
        for (column, widgets) in self.designer_columns.iter_mut().zip(&self.designer_widgets) {
            column.name = widgets.name.text().to_string();
            column.type_name = widgets.type_name.text().to_string();
            column.primary_key = widgets.primary_key.is_active();
            column.nullable = !widgets.not_null.is_active();
            column.unique = widgets.unique.is_active();
            column.auto_increment = widgets.auto_increment.is_active();
            let default = widgets.default.text().to_string();
            column.default = if default.trim().is_empty() {
                None
            } else {
                Some(default)
            };
        }
    }

    fn rebuild_designer_columns(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.designer_columns_box.first_child() {
            self.designer_columns_box.remove(&child);
        }
        self.designer_widgets.clear();
        for (index, column) in self.designer_columns.iter().enumerate() {
            let name = gtk::Entry::builder()
                .text(&column.name)
                .placeholder_text("name")
                .width_chars(12)
                .build();
            let type_name = gtk::Entry::builder()
                .text(&column.type_name)
                .placeholder_text("type")
                .width_chars(12)
                .build();
            let primary_key = gtk::CheckButton::with_label("PK");
            primary_key.set_active(column.primary_key);
            primary_key.set_tooltip_text(Some("Primary key"));
            let not_null = gtk::CheckButton::with_label("NN");
            not_null.set_active(!column.nullable);
            not_null.set_tooltip_text(Some("Not null"));
            let unique = gtk::CheckButton::with_label("UQ");
            unique.set_active(column.unique);
            unique.set_tooltip_text(Some("Unique"));
            let auto_increment = gtk::CheckButton::with_label("AI");
            auto_increment.set_active(column.auto_increment);
            auto_increment.set_tooltip_text(Some("Auto increment"));
            let default = gtk::Entry::builder()
                .text(column.default.clone().unwrap_or_default())
                .placeholder_text("default")
                .width_chars(10)
                .build();
            let remove = gtk::Button::from_icon_name("list-remove-symbolic");
            remove.set_tooltip_text(Some("Remove column"));
            {
                let sender = sender.clone();
                remove.connect_clicked(move |_| sender.input(AppMsg::DesignerRemoveColumn(index)));
            }
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.append(&name);
            row.append(&type_name);
            row.append(&primary_key);
            row.append(&not_null);
            row.append(&unique);
            row.append(&auto_increment);
            row.append(&default);
            row.append(&remove);
            self.designer_columns_box.append(&row);
            self.designer_widgets.push(ColumnWidgets {
                name,
                type_name,
                primary_key,
                not_null,
                unique,
                auto_increment,
                default,
            });
        }
    }

    fn preview_designer(&mut self) -> Result<Vec<String>, unidb_core::UniDbError> {
        self.sync_designer_columns();
        let Some(dialect) = self.designer_dialect() else {
            return Err(unidb_core::UniDbError::new(
                "table_design_unsupported",
                unidb_core::ErrorCategory::Unsupported,
                "The table designer is not available for this engine",
            ));
        };
        let table_name = self.designer_name_entry.text().to_string();
        let design = TableDesign {
            name: table_name.clone(),
            columns: self.designer_columns.clone(),
        };
        let statements = match &self.designer_table {
            Some(table) if !self.designer_original.is_empty() => {
                alter_table_sql(dialect, table, &self.designer_original, &design)?
            }
            Some(table) => alter_table_sql(dialect, table, &self.designer_original, &design)?,
            None => create_table_sql(dialect, &design)?,
        };
        self.designer_preview_buffer.set_text("");
        for statement in &statements {
            self.designer_preview_buffer
                .insert_at_cursor(&format!("{statement};\n\n"));
        }
        Ok(statements)
    }

    fn refresh_transfer(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.transfer_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        let model = gtk::StringList::new(&names);
        self.transfer_conn_dropdown.set_model(Some(&model));
        if live.is_empty() {
            self.transfer_connection = None;
            self.transfer_stack.set_visible_child_name("empty");
            return;
        }
        self.transfer_stack.set_visible_child_name("form");
        let current = self
            .transfer_connection
            .filter(|id| self.transfer_ids.contains(id))
            .unwrap_or(self.transfer_ids[0]);
        self.transfer_connection = Some(current);
        self.transfer_database = self.designer_database_for(current);
        let index = self
            .transfer_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.transfer_conn_dropdown.set_selected(index as u32);
        self.load_transfer_tables(sender);
    }

    fn load_transfer_tables(&mut self, sender: &ComponentSender<Self>) {
        let Some(id) = self.transfer_connection else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        let database = self.transfer_database.clone();
        self.handle.spawn(async move {
            match service.list_tables(id, &database).await {
                Ok(tables) => sender.input(AppMsg::TransferTables(Box::new(tables))),
                Err(error) => sender.input(AppMsg::TransferFailed(format!(
                    "Could not list tables: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn apply_color_scheme(&self, index: u32) {
        let manager = adw::StyleManager::default();
        let scheme = match index {
            1 => adw::ColorScheme::ForceLight,
            2 => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        };
        manager.set_color_scheme(scheme);
    }

    fn load_color_scheme(&mut self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            let scheme = service
                .get_setting("color_scheme")
                .await
                .ok()
                .flatten()
                .and_then(|value| value.as_str().map(str::to_string));
            sender.input(AppMsg::SettingsLoaded(scheme));
        });
    }

    fn load_history(&mut self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let query = self.history_search_entry.text().to_string();
        let history_service = Arc::clone(service);
        let history_sender = sender.clone();
        self.handle.spawn(async move {
            match history_service.search_history(&query, 100).await {
                Ok(entries) => history_sender.input(AppMsg::HistoryLoaded(Box::new(entries))),
                Err(error) => history_sender.input(AppMsg::HistoryFailed(format!(
                    "Could not read history: {}",
                    error.user_message()
                ))),
            }
        });
        let saved_service = Arc::clone(service);
        let saved_sender = sender.clone();
        self.handle.spawn(async move {
            match saved_service.list_saved_queries().await {
                Ok(queries) => saved_sender.input(AppMsg::SavedQueriesLoaded(Box::new(queries))),
                Err(error) => saved_sender.input(AppMsg::HistoryFailed(format!(
                    "Could not read saved queries: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn populate_history(&mut self, sender: &ComponentSender<Self>) {
        for row in self.history_rows.drain(..) {
            self.history_list.remove(&row);
        }
        if self.history_entries.is_empty() {
            let row = plain_row(
                "No statements recorded yet",
                "document-open-recent-symbolic",
            );
            self.history_list.append(&row);
            self.history_rows.push(row);
            return;
        }
        let entries = self.history_entries.clone();
        for entry in entries {
            let preview = entry
                .sql
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(110)
                .collect::<String>();
            let row = gtk::ListBoxRow::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.set_margin_top(8);
            content.set_margin_bottom(8);
            content.set_margin_start(10);
            content.set_margin_end(10);
            content.append(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text_box.set_hexpand(true);
            let title = gtk::Label::new(Some(&preview));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let subtitle = gtk::Label::new(Some(&format!(
                "{}  |  {}  |  {} rows  |  {} ms",
                entry.started_at,
                entry.status,
                entry.row_count.unwrap_or(0),
                entry.duration_ms
            )));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("unidb-muted");
            text_box.append(&title);
            text_box.append(&subtitle);
            content.append(&text_box);
            row.set_child(Some(&content));
            row.set_tooltip_text(Some(&entry.sql));
            let sql = entry.sql.clone();
            let sender = sender.clone();
            row.connect_activate(move |_| sender.input(AppMsg::LoadSql(sql.clone())));
            self.history_list.append(&row);
            self.history_rows.push(row);
        }
    }

    fn populate_saved(&mut self, queries: &[SavedQuery], sender: &ComponentSender<Self>) {
        for row in self.saved_rows.drain(..) {
            self.saved_list.remove(&row);
        }
        if queries.is_empty() {
            let row = plain_row("No saved queries yet", "document-save-symbolic");
            self.saved_list.append(&row);
            self.saved_rows.push(row);
            return;
        }
        for query in queries {
            let preview = query
                .sql
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(80)
                .collect::<String>();
            let row = gtk::ListBoxRow::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.set_margin_top(8);
            content.set_margin_bottom(8);
            content.set_margin_start(10);
            content.set_margin_end(10);
            content.append(&gtk::Image::from_icon_name("document-save-symbolic"));
            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text_box.set_hexpand(true);
            let title = gtk::Label::new(Some(&query.name));
            title.set_xalign(0.0);
            let subtitle = gtk::Label::new(Some(&preview));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("unidb-muted");
            text_box.append(&title);
            text_box.append(&subtitle);
            content.append(&text_box);
            let delete_button = gtk::Button::from_icon_name("user-trash-symbolic");
            delete_button.set_tooltip_text(Some("Delete saved query"));
            delete_button.add_css_class("flat");
            let id = query.id;
            {
                let sender = sender.clone();
                delete_button.connect_clicked(move |_| sender.input(AppMsg::DeleteSavedQuery(id)));
            }
            content.append(&delete_button);
            row.set_child(Some(&content));
            row.set_tooltip_text(Some(&query.sql));
            let sql = query.sql.clone();
            let sender = sender.clone();
            row.connect_activate(move |_| sender.input(AppMsg::LoadSql(sql.clone())));
            self.saved_list.append(&row);
            self.saved_rows.push(row);
        }
    }

    fn refresh_schema(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.schema_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        self.schema_conn_dropdown
            .set_model(Some(&gtk::StringList::new(&names)));
        if live.is_empty() {
            self.schema_connection = None;
            self.schema_stack.set_visible_child_name("empty");
            return;
        }
        self.schema_stack.set_visible_child_name("form");
        let current = self
            .schema_connection
            .filter(|id| self.schema_ids.contains(id))
            .unwrap_or(self.schema_ids[0]);
        self.schema_connection = Some(current);
        self.schema_database = self.designer_database_for(current);
        let index = self
            .schema_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.schema_conn_dropdown.set_selected(index as u32);
        self.load_schema(sender);
    }

    fn load_schema(&mut self, sender: &ComponentSender<Self>) {
        let Some(connection_id) = self.schema_connection else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        let database = self.schema_database.clone();
        self.schema_status.set_label("Loading schema...");
        self.handle.spawn(async move {
            let tables = match service.list_tables(connection_id, &database).await {
                Ok(tables) => tables,
                Err(error) => {
                    sender.input(AppMsg::SchemaFailed(format!(
                        "Could not list tables: {}",
                        error.user_message()
                    )));
                    return;
                }
            };
            let mut erd_tables = Vec::new();
            let mut failures = 0usize;
            for table in tables.iter().take(24) {
                let columns = match service
                    .describe_table(connection_id, &database, table)
                    .await
                {
                    Ok(columns) => columns,
                    Err(_) => {
                        failures += 1;
                        continue;
                    }
                };
                let foreign_keys = service
                    .foreign_keys(connection_id, &database, table)
                    .await
                    .unwrap_or_default();
                erd_tables.push(ErdTable {
                    name: table.clone(),
                    columns: columns.iter().map(|column| column.name.clone()).collect(),
                    foreign_keys,
                });
            }
            if failures > 0 {
                eprintln!("{failures} tables could not be described for the ERD");
            }
            sender.input(AppMsg::SchemaLoaded(Box::new(erd_tables)));
        });
    }

    fn refresh_builder(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.builder_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        self.builder_conn_dropdown
            .set_model(Some(&gtk::StringList::new(&names)));
        if live.is_empty() {
            self.builder_connection = None;
            self.builder_stack.set_visible_child_name("empty");
            return;
        }
        self.builder_stack.set_visible_child_name("form");
        let current = self
            .builder_connection
            .filter(|id| self.builder_ids.contains(id))
            .unwrap_or(self.builder_ids[0]);
        self.builder_connection = Some(current);
        self.builder_database = self.designer_database_for(current);
        let index = self
            .builder_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.builder_conn_dropdown.set_selected(index as u32);
        self.load_builder_tables(sender);
    }

    fn load_builder_tables(&mut self, sender: &ComponentSender<Self>) {
        let Some(id) = self.builder_connection else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        let database = self.builder_database.clone();
        self.handle.spawn(async move {
            match service.list_tables(id, &database).await {
                Ok(tables) => sender.input(AppMsg::BuilderTables(Box::new(tables))),
                Err(error) => sender.input(AppMsg::BuilderFailed(format!(
                    "Could not list tables: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn rebuild_builder_columns(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.builder_columns_box.first_child() {
            self.builder_columns_box.remove(&child);
        }
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_column_spacing(10);
        flow.set_row_spacing(6);
        for column in &self.builder_columns {
            let check = gtk::CheckButton::with_label(column);
            check.set_active(self.builder_selected.contains(column));
            let name = column.clone();
            let sender = sender.clone();
            check.connect_toggled(move |button| {
                sender.input(AppMsg::BuilderToggleColumn(
                    name.clone(),
                    button.is_active(),
                ));
            });
            flow.insert(&check, -1);
        }
        self.builder_columns_box.append(&flow);
    }

    fn populate_builder_filters(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.builder_filters_box.first_child() {
            self.builder_filters_box.remove(&child);
        }
        if self.builder_filters.is_empty() {
            self.builder_filters_box
                .append(&plain_row("No filters yet", "view-filter-symbolic"));
            return;
        }
        let filters = self.builder_filters.clone();
        for (index, filter) in filters.iter().enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_start(6);
            row.set_margin_end(6);
            row.set_margin_top(2);
            row.set_margin_bottom(2);
            let label = gtk::Label::new(Some(&format!(
                "{} {} {}",
                filter.column,
                filter.operator.label(),
                filter.value
            )));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            row.append(&label);
            let remove = gtk::Button::from_icon_name("list-remove-symbolic");
            remove.add_css_class("flat");
            remove.set_tooltip_text(Some("Remove filter"));
            {
                let sender = sender.clone();
                remove.connect_clicked(move |_| sender.input(AppMsg::BuilderRemoveFilter(index)));
            }
            row.append(&remove);
            self.builder_filters_box.append(&row);
        }
    }

    fn preview_builder(&self) -> Result<String, UniDbError> {
        let Some(connection_id) = self.builder_connection else {
            return Err(UniDbError::new(
                "builder_invalid",
                ErrorCategory::Syntax,
                "Choose a live connection first",
            ));
        };
        let Some(engine) = self
            .connections
            .iter()
            .find(|profile| profile.id == connection_id)
            .map(|profile| profile.engine)
        else {
            return Err(UniDbError::new(
                "builder_invalid",
                ErrorCategory::Syntax,
                "The connection profile could not be loaded",
            ));
        };
        let Some(dialect) = SqlDialect::for_engine(engine) else {
            return Err(UniDbError::new(
                "builder_unsupported",
                ErrorCategory::Unsupported,
                "The query builder supports SQL engines only",
            ));
        };
        let table = self.builder_table.clone().unwrap_or_default();
        let order = self
            .builder_columns
            .get(self.builder_order_dropdown.selected() as usize)
            .cloned();
        let builder = QueryBuilder {
            table,
            columns: self.builder_selected.clone(),
            filters: self.builder_filters.clone(),
            order_by: order,
            ascending: self.builder_order_asc.is_active(),
            limit: Some(self.builder_limit_spin.value_as_int() as u32),
        };
        build_query(&builder, dialect)
    }

    fn refresh_migrations(&mut self, sender: &ComponentSender<Self>) {
        let live: Vec<ConnectionProfile> = self
            .connections
            .iter()
            .filter(|profile| self.connected.contains(&profile.id))
            .cloned()
            .collect();
        self.migrations_ids = live.iter().map(|profile| profile.id).collect();
        let names: Vec<&str> = live.iter().map(|profile| profile.name.as_str()).collect();
        self.migrations_conn_dropdown
            .set_model(Some(&gtk::StringList::new(&names)));
        if live.is_empty() {
            self.migrations_connection = None;
            self.migrations_stack.set_visible_child_name("empty");
            return;
        }
        self.migrations_stack.set_visible_child_name("form");
        let current = self
            .migrations_connection
            .filter(|id| self.migrations_ids.contains(id))
            .unwrap_or(self.migrations_ids[0]);
        self.migrations_connection = Some(current);
        let index = self
            .migrations_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.migrations_conn_dropdown.set_selected(index as u32);
        self.scan_migrations(sender);
    }

    fn scan_migrations(&mut self, sender: &ComponentSender<Self>) {
        let Some(connection_id) = self.migrations_connection else {
            self.migrations_status
                .set_label("Choose a live connection first.");
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let dir_str = self.migrations_dir_entry.text().to_string();
        let dir = std::path::PathBuf::from(dir_str);
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.migrations_status
            .set_label("Scanning migrations directory...");
        self.handle.spawn(async move {
            let git_status = git_dir_status(&dir);
            match service.plan_migrations(connection_id, &dir).await {
                Ok((files, applied, warnings)) => {
                    sender.input(AppMsg::MigrationsScanned(Box::new((
                        files, applied, warnings, git_status,
                    ))));
                }
                Err(error) => sender.input(AppMsg::MigrationsScanFailed(format!(
                    "Scan failed: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn populate_migrations_list(&mut self) {
        while let Some(row) = self.migrations_list.row_at_index(0) {
            self.migrations_list.remove(&row);
        }
        let git_note = self.migrations_git_status.summary();
        if self.migrations_files.is_empty() {
            let row = plain_row(
                "No numbered migration files found",
                "folder-download-symbolic",
            );
            self.migrations_list.append(&row);
            self.migrations_preview_buffer.set_text("");
            self.migrations_status.set_label(&format!(
                "No .sql migration files found in the directory. | {git_note}"
            ));
            return;
        }

        let applied_count = self
            .migrations_files
            .iter()
            .filter(|f| self.migrations_applied.contains(&f.name))
            .count();
        let pending_count = self.migrations_files.len() - applied_count;
        let warning_count = self.migrations_warnings.len();

        let status_summary = if warning_count > 0 {
            format!(
                "{} files: {} applied, {} pending, {} modified! | {git_note}",
                self.migrations_files.len(),
                applied_count,
                pending_count,
                warning_count
            )
        } else {
            format!(
                "{} files: {} applied, {} pending. | {git_note}",
                self.migrations_files.len(),
                applied_count,
                pending_count
            )
        };
        self.migrations_status.set_label(&status_summary);

        let files = self.migrations_files.clone();
        for file in &files {
            let is_applied = self.migrations_applied.contains(&file.name);
            let has_warning = self
                .migrations_warnings
                .iter()
                .any(|w| w.starts_with(&file.name));

            let (badge_text, badge_class, icon_name) = if has_warning {
                ("Modified", "status-fail", "dialog-warning-symbolic")
            } else if is_applied {
                ("Applied", "status-pass", "emblem-ok-symbolic")
            } else {
                ("Pending", "status-warn", "document-send-symbolic")
            };

            let row = gtk::ListBoxRow::new();
            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row_box.set_margin_all(8);

            let icon = gtk::Image::from_icon_name(icon_name);
            row_box.append(&icon);

            let label_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            label_box.set_hexpand(true);

            let title = gtk::Label::new(Some(&file.name));
            title.set_xalign(0.0);
            let check_prefix = if file.checksum.len() >= 8 {
                &file.checksum[..8]
            } else {
                &file.checksum
            };
            let subtitle = gtk::Label::new(Some(&format!("Checksum: {check_prefix}")));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("unidb-muted");
            label_box.append(&title);
            label_box.append(&subtitle);
            row_box.append(&label_box);

            let badge = gtk::Label::new(Some(badge_text));
            badge.add_css_class(badge_class);
            row_box.append(&badge);

            row.set_child(Some(&row_box));
            self.migrations_list.append(&row);
        }

        if let Some(first) = self.migrations_files.first() {
            self.migrations_preview_buffer.set_text(&first.sql);
            if let Some(row) = self.migrations_list.row_at_index(0) {
                self.migrations_list.select_row(Some(&row));
            }
        }
    }

    fn refresh_snapshots_connections(&mut self, sender: &ComponentSender<Self>) {
        self.snapshots_ids = self.connections.iter().map(|profile| profile.id).collect();
        let names: Vec<String> = self
            .connections
            .iter()
            .map(|profile| {
                if self.connected.contains(&profile.id) {
                    format!("{} (live)", profile.name)
                } else {
                    profile.name.clone()
                }
            })
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.snapshots_conn_dropdown
            .set_model(Some(&gtk::StringList::new(&refs)));
        if self.snapshots_ids.is_empty() {
            self.snapshots_connection = None;
            self.snapshots_status
                .set_label("Save a connection first to take or view snapshots.");
            self.update_controls();
            return;
        }
        let default_id = self
            .connections
            .iter()
            .find(|p| self.connected.contains(&p.id))
            .map(|p| p.id)
            .unwrap_or(self.snapshots_ids[0]);
        let current = self
            .snapshots_connection
            .filter(|id| self.snapshots_ids.contains(id))
            .unwrap_or(default_id);
        self.snapshots_connection = Some(current);
        let index = self
            .snapshots_ids
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        self.snapshots_conn_dropdown.set_selected(index as u32);
        self.update_controls();
        self.load_snapshots(sender);
    }

    fn load_snapshots(&mut self, sender: &ComponentSender<Self>) {
        let Some(connection_id) = self.snapshots_connection else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.list_snapshots(connection_id).await {
                Ok(items) => sender.input(AppMsg::SnapshotsLoaded(items)),
                Err(error) => sender.input(AppMsg::SnapshotsFailed(format!(
                    "Could not read snapshots: {}",
                    error.user_message()
                ))),
            }
        });
    }

    fn populate_snapshots(&mut self, sender: &ComponentSender<Self>) {
        for row in self.snapshots_rows.drain(..) {
            self.snapshots_list.remove(&row);
        }
        if self.snapshots_items.is_empty() {
            let row = plain_row("No table snapshots yet", "camera-photo-symbolic");
            self.snapshots_list.append(&row);
            self.snapshots_rows.push(row);
            return;
        }
        let items = self.snapshots_items.clone();
        for item in items {
            let row = gtk::ListBoxRow::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.set_margin_top(8);
            content.set_margin_bottom(8);
            content.set_margin_start(10);
            content.set_margin_end(10);
            content.append(&gtk::Image::from_icon_name("camera-photo-symbolic"));

            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text_box.set_hexpand(true);
            let title_text = match &item.note {
                Some(note) if !note.is_empty() => format!("{}  ({})", item.table_name, note),
                _ => item.table_name.clone(),
            };
            let title = gtk::Label::new(Some(&title_text));
            title.set_xalign(0.0);
            let subtitle = gtk::Label::new(Some(&format!(
                "{}  |  {} row{}  |  {}",
                item.database_name,
                item.row_count,
                plural(item.row_count),
                item.created_at
            )));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("unidb-muted");
            text_box.append(&title);
            text_box.append(&subtitle);
            content.append(&text_box);

            let compare_btn = gtk::Button::with_label("Compare");
            {
                let sender = sender.clone();
                let id = item.id;
                compare_btn.connect_clicked(move |_| sender.input(AppMsg::CompareSnapshot(id)));
            }
            content.append(&compare_btn);

            let restore_btn = gtk::Button::with_label("Restore");
            restore_btn.add_css_class("suggested-action");
            {
                let sender = sender.clone();
                let id = item.id;
                restore_btn.connect_clicked(move |_| sender.input(AppMsg::RestoreSnapshot(id)));
            }
            content.append(&restore_btn);

            let delete_btn = gtk::Button::with_label("Delete");
            delete_btn.add_css_class("destructive-action");
            {
                let sender = sender.clone();
                let id = item.id;
                delete_btn.connect_clicked(move |_| sender.input(AppMsg::DeleteSnapshot(id)));
            }
            content.append(&delete_btn);

            row.set_child(Some(&content));
            self.snapshots_list.append(&row);
            self.snapshots_rows.push(row);
        }
    }

    fn load_security(&mut self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.security_report().await {
                Ok(report) => {
                    let audit = service.recent_audit(10).await.unwrap_or_default();
                    sender.input(AppMsg::SecurityLoaded(Box::new((report, audit))));
                }
                Err(error) => sender.input(AppMsg::SecurityFailed(format!(
                    "{} ({})",
                    error.user_message(),
                    error.code()
                ))),
            }
        });
    }

    fn populate_security(&mut self, report: &SecurityReport, audit: &[AuditEntry]) {
        for row in self.checks_rows.drain(..) {
            self.checks_group.remove(&row);
        }
        if let Some(data_dir) = self.service.as_ref().and_then(|service| service.data_dir()) {
            let row = adw::ActionRow::builder()
                .title("Data directory")
                .subtitle(data_dir.display().to_string())
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
            self.checks_group.add(&row);
            self.checks_rows.push(row);
        }
        let summary = adw::ActionRow::builder()
            .title("Store contents")
            .subtitle(format!(
                "{} connections, {} encrypted secrets, {} audit entries",
                report.connection_count, report.secret_count, report.audit_count
            ))
            .build();
        summary.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));
        self.checks_group.add(&summary);
        self.checks_rows.push(summary);

        for check in &report.checks {
            let (icon, class) = status_style(check.status);
            let row = adw::ActionRow::builder()
                .title(check.label)
                .subtitle(&check.detail)
                .build();
            row.add_prefix(&gtk::Image::from_icon_name(icon));
            let pill = gtk::Label::new(Some(check.status.label()));
            pill.add_css_class(class);
            row.add_suffix(&pill);
            self.checks_group.add(&row);
            self.checks_rows.push(row);
        }

        for row in self.audit_rows.drain(..) {
            self.audit_group.remove(&row);
        }
        if audit.is_empty() {
            let row = adw::ActionRow::builder()
                .title("No audit entries yet")
                .subtitle("Connection and query actions appear here as they happen.")
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
            self.audit_group.add(&row);
            self.audit_rows.push(row);
        } else {
            for entry in audit {
                let row = adw::ActionRow::builder()
                    .title(&entry.action)
                    .subtitle(&entry.ts)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
                let outcome = gtk::Label::new(Some(&entry.outcome));
                outcome.add_css_class("unidb-mono");
                outcome.add_css_class("unidb-muted");
                row.add_suffix(&outcome);
                self.audit_group.add(&row);
                self.audit_rows.push(row);
            }
        }
    }
}

fn format_plan(result: &ResultSet) -> String {
    if result.columns.is_empty() {
        return "No plan rows returned.".to_string();
    }
    let mut lines = Vec::new();
    for row in &result.rows {
        if row.len() == 1 {
            let text = unidb_app::value_text(&row[0]);
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text)
                && (parsed.is_object() || parsed.is_array())
            {
                lines.push(serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| text.clone()));
                continue;
            }
        }
        lines.push(
            row.iter()
                .map(unidb_app::value_text)
                .collect::<Vec<String>>()
                .join(" | "),
        );
    }
    if lines.is_empty() {
        "No plan rows returned.".to_string()
    } else {
        lines.join("\n")
    }
}

fn draw_chart(
    _area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    points: &[ChartPoint],
) {
    let width = f64::from(width);
    let height = f64::from(height);
    let dark = adw::StyleManager::default().is_dark();
    let text = if dark {
        (0.9, 0.9, 0.9)
    } else {
        (0.15, 0.15, 0.15)
    };
    let teal = (0.05, 0.58, 0.53);
    let margin_left = 130.0;
    let margin_right = 70.0;
    let top = 10.0;
    let bottom = 10.0;
    if points.is_empty() {
        return;
    }
    let plot_width = (width - margin_left - margin_right).max(10.0);
    let row_height = ((height - top - bottom) / points.len() as f64).max(6.0);
    let bar_height = (row_height - 4.0).max(3.0);
    let max_value = points
        .iter()
        .map(|point| point.value.abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    context.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    context.set_font_size(11.0);
    for (index, point) in points.iter().enumerate() {
        let y = top + row_height * index as f64;
        let label: String = point.label.chars().take(16).collect();
        context.set_source_rgb(text.0, text.1, text.2);
        context.move_to(4.0, y + bar_height * 0.8);
        let _ = context.show_text(&label);
        let bar_width = (point.value.abs() / max_value) * plot_width;
        context.set_source_rgb(teal.0, teal.1, teal.2);
        context.rectangle(margin_left, y + 2.0, bar_width.max(1.0), bar_height);
        let _ = context.fill();
        context.set_source_rgb(text.0, text.1, text.2);
        context.move_to(margin_left + bar_width + 6.0, y + bar_height * 0.8);
        let _ = context.show_text(&format!("{:.2}", point.value));
    }
}

fn draw_erd(context: &gtk::cairo::Context, width: i32, height: i32, tables: &[ErdTable]) {
    let dark = adw::StyleManager::default().is_dark();
    let layout: ErdLayout = erd_layout(tables, f64::from(width).max(600.0));
    let text = if dark {
        (0.92, 0.92, 0.92)
    } else {
        (0.13, 0.13, 0.13)
    };
    let muted = if dark {
        (0.65, 0.65, 0.65)
    } else {
        (0.4, 0.4, 0.4)
    };
    let card_color = if dark {
        (0.16, 0.17, 0.18)
    } else {
        (0.99, 0.99, 0.99)
    };
    let border = if dark {
        (0.32, 0.34, 0.36)
    } else {
        (0.8, 0.82, 0.84)
    };
    let teal = (0.05, 0.58, 0.53);

    // Relationship arrows first so cards sit above their starts.
    for edge in &layout.edges {
        let (Some(from), Some(to)) = (
            layout.boxes.iter().find(|card| card.index == edge.from),
            layout.boxes.iter().find(|card| card.index == edge.to),
        ) else {
            continue;
        };
        let start_x = from.x + from.width;
        let start_y = from.y + from.height / 2.0;
        let end_x = to.x;
        let end_y = to.y + to.height / 2.0;
        context.set_source_rgb(teal.0, teal.1, teal.2);
        context.set_line_width(2.0);
        let control = ((start_x + end_x) / 2.0, (start_y + end_y) / 2.0);
        context.move_to(start_x, start_y);
        context.curve_to(control.0, start_y, control.0, end_y, end_x, end_y);
        let _ = context.stroke();
        context.move_to(end_x, end_y);
        context.line_to(end_x - 8.0, end_y - 5.0);
        context.line_to(end_x - 8.0, end_y + 5.0);
        context.close_path();
        let _ = context.fill();
        let _ = control;
    }

    for card in &layout.boxes {
        rounded_rectangle(context, card.x, card.y, card.width, card.height, 8.0);
        context.set_source_rgb(card_color.0, card_color.1, card_color.2);
        let _ = context.fill_preserve();
        context.set_source_rgb(border.0, border.1, border.2);
        context.set_line_width(1.2);
        let _ = context.stroke();

        context.set_source_rgb(text.0, text.1, text.2);
        context.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Bold,
        );
        context.set_font_size(13.0);
        context.move_to(card.x + 12.0, card.y + 20.0);
        let _ = context.show_text(&card.title);

        context.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Normal,
        );
        context.set_font_size(11.0);
        for (row, column) in card.columns.iter().enumerate() {
            let y = card.y + 40.0 + row as f64 * 18.0;
            let is_key = card.foreign_key_columns.contains(column);
            context.set_source_rgb(
                if is_key { teal.0 } else { muted.0 },
                if is_key { teal.1 } else { muted.1 },
                if is_key { teal.2 } else { muted.2 },
            );
            context.move_to(card.x + 12.0, y);
            let _ = context.show_text(column);
            if is_key {
                context.set_font_size(9.0);
                context.move_to(card.x + card.width - 30.0, y);
                let _ = context.show_text("FK");
                context.set_font_size(11.0);
            }
        }
    }

    // Relationship labels on top of the cards.
    context.select_font_face(
        "Sans",
        gtk::cairo::FontSlant::Normal,
        gtk::cairo::FontWeight::Normal,
    );
    context.set_font_size(10.0);
    for edge in &layout.edges {
        let (Some(from), Some(to)) = (
            layout.boxes.iter().find(|card| card.index == edge.from),
            layout.boxes.iter().find(|card| card.index == edge.to),
        ) else {
            continue;
        };
        let mid_x = (from.x + from.width + to.x) / 2.0;
        let mid_y = (from.y + from.height / 2.0 + to.y + to.height / 2.0) / 2.0;
        context.set_source_rgb(muted.0, muted.1, muted.2);
        context.move_to(mid_x - 16.0, mid_y - 6.0);
        let _ = context.show_text(&edge.label);
    }
    let _ = height;
}

fn rounded_rectangle(
    context: &gtk::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    context.new_path();
    context.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    context.arc(
        x + width - radius,
        y + radius,
        radius,
        1.5 * std::f64::consts::PI,
        0.0,
    );
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        0.5 * std::f64::consts::PI,
    );
    context.arc(
        x + radius,
        y + height - radius,
        radius,
        0.5 * std::f64::consts::PI,
        std::f64::consts::PI,
    );
    context.close_path();
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn status_style(status: CheckStatus) -> (&'static str, &'static str) {
    match status {
        CheckStatus::Pass => ("emblem-ok-symbolic", "status-pass"),
        CheckStatus::Warn => ("dialog-warning-symbolic", "status-warn"),
        CheckStatus::Fail => ("dialog-error-symbolic", "status-fail"),
    }
}

fn tls_index(mode: SslMode) -> u32 {
    match mode {
        SslMode::Disable => 0,
        SslMode::Prefer => 1,
        SslMode::Require => 2,
        SslMode::VerifyCa => 3,
        SslMode::VerifyFull => 4,
    }
}

fn tls_from_index(index: u32) -> SslMode {
    match index {
        0 => SslMode::Disable,
        2 => SslMode::Require,
        3 => SslMode::VerifyCa,
        4 => SslMode::VerifyFull,
        _ => SslMode::Prefer,
    }
}

fn sender_has_update_feed() -> bool {
    std::env::var("UNIDB_UPDATE_FEED").is_ok()
}

fn drain_rows(list: &gtk::ListBox) -> Vec<gtk::ListBoxRow> {
    let mut rows = Vec::new();
    let mut child = list.first_child();
    while let Some(widget) = child {
        if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>() {
            rows.push(row.clone());
        }
        child = widget.next_sibling();
    }
    rows
}

fn plain_row(label: &str, icon: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.append(&gtk::Image::from_icon_name(icon));
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&text);
    row.set_child(Some(&content));
    row
}

fn build_shell(root: &adw::ApplicationWindow, sender: &ComponentSender<AppModel>) -> ShellWidgets {
    let window_title = adw::WindowTitle::new(ABOUT_NAME, "Connections");
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&window_title));

    let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh_button.set_tooltip_text(Some("Refresh"));
    {
        let sender = sender.clone();
        refresh_button.connect_clicked(move |_| sender.input(AppMsg::Refresh));
    }
    let backup_button = gtk::Button::from_icon_name("document-save-symbolic");
    backup_button.set_tooltip_text(Some("Back up now"));
    {
        let sender = sender.clone();
        backup_button.connect_clicked(move |_| sender.input(AppMsg::Backup));
    }
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main menu")
        .build();
    let menu_popover = gtk::Popover::new();
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    menu_box.set_margin_all(6);
    let about_button = gtk::Button::with_label("About UniDB");
    {
        let sender = sender.clone();
        about_button.connect_clicked(move |_| sender.input(AppMsg::About));
    }
    let quit_button = gtk::Button::with_label("Quit");
    {
        let sender = sender.clone();
        quit_button.connect_clicked(move |_| sender.input(AppMsg::Quit));
    }
    menu_box.append(&about_button);
    menu_box.append(&quit_button);
    menu_popover.set_child(Some(&menu_box));
    menu_button.set_popover(Some(&menu_popover));
    header.pack_end(&menu_button);
    header.pack_end(&refresh_button);
    header.pack_end(&backup_button);

    let connections_group = adw::PreferencesGroup::builder()
        .title("Saved connections")
        .build();
    let new_button = gtk::Button::with_label("New connection");
    new_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        new_button.connect_clicked(move |_| sender.input(AppMsg::ShowNewConnection));
    }
    let list_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    list_toolbar.set_margin_all(12);
    list_toolbar.append(&new_button);
    let list_page_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    list_page_content.append(&list_toolbar);
    let bundle_path_entry = gtk::Entry::builder()
        .placeholder_text("Team bundle JSON path (e.g. /tmp/unidb-team-bundle.json)")
        .hexpand(true)
        .build();
    let bundle_browse_button = gtk::Button::with_label("Browse");
    {
        let sender = sender.clone();
        bundle_browse_button.connect_clicked(move |_| sender.input(AppMsg::BundleBrowse));
    }
    let bundle_export_button = gtk::Button::with_label("Export bundle");
    {
        let sender = sender.clone();
        bundle_export_button.connect_clicked(move |_| sender.input(AppMsg::ExportTeamBundle));
    }
    let bundle_inspect_button = gtk::Button::with_label("Verify & Import");
    bundle_inspect_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        bundle_inspect_button.connect_clicked(move |_| sender.input(AppMsg::InspectTeamBundle));
    }
    let bundle_controls_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bundle_controls_box.set_margin_bottom(6);
    bundle_controls_box.append(&bundle_path_entry);
    bundle_controls_box.append(&bundle_browse_button);
    bundle_controls_box.append(&bundle_export_button);
    bundle_controls_box.append(&bundle_inspect_button);
    let bundle_status = gtk::Label::builder()
        .label("Share signed JSON bundles without passwords or keys.")
        .xalign(0.0)
        .wrap(true)
        .build();
    bundle_status.add_css_class("unidb-muted");
    let bundle_group = adw::PreferencesGroup::builder()
        .title("Team bundles")
        .description("Signed workspace export and verified import without secrets")
        .build();
    bundle_group.add(&bundle_controls_box);
    bundle_group.add(&bundle_status);

    let connections_page = adw::PreferencesPage::new();
    connections_page.add(&connections_group);
    connections_page.add(&bundle_group);
    let connections_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&connections_page)
        .build();
    list_page_content.append(&connections_scroll);

    let details_group = adw::PreferencesGroup::builder()
        .title("New connection")
        .build();
    let name_entry = adw::EntryRow::new();
    name_entry.set_title("Name");
    let engine_names: Vec<&str> = Engine::ALL.iter().map(|engine| engine.display()).collect();
    let engine_combo = adw::ComboRow::builder()
        .title("Engine")
        .model(&gtk::StringList::new(&engine_names))
        .build();
    let host_entry = adw::EntryRow::new();
    host_entry.set_title("Host");
    let port_spin = adw::SpinRow::new(
        Some(&gtk::Adjustment::new(5432.0, 0.0, 65535.0, 1.0, 10.0, 0.0)),
        1.0,
        0,
    );
    port_spin.set_title("Port");
    let database_entry = adw::EntryRow::new();
    database_entry.set_title("Database or file");
    let username_entry = adw::EntryRow::new();
    username_entry.set_title("Username");
    let password_entry = adw::PasswordEntryRow::new();
    password_entry.set_title("Password");
    details_group.add(&name_entry);
    details_group.add(&engine_combo);
    details_group.add(&host_entry);
    details_group.add(&port_spin);
    details_group.add(&database_entry);
    details_group.add(&username_entry);
    details_group.add(&password_entry);

    let access_group = adw::PreferencesGroup::builder().title("Access").build();
    let tls_names = [
        "No TLS",
        "Prefer TLS",
        "Require TLS",
        "Verify CA",
        "Verify full",
    ];
    let tls_combo = adw::ComboRow::builder()
        .title("TLS mode")
        .model(&gtk::StringList::new(&tls_names))
        .build();
    let readonly_switch = adw::SwitchRow::builder()
        .title("Read only")
        .subtitle("Refuse writes before they reach the server")
        .build();
    access_group.add(&tls_combo);
    access_group.add(&readonly_switch);

    let url_group = adw::PreferencesGroup::builder().title("Use a URL").build();
    let url_switch = adw::SwitchRow::builder()
        .title("Paste a URL")
        .subtitle("When enabled, the URL fills the profile instead of the fields above")
        .build();
    let url_entry = adw::EntryRow::new();
    url_entry.set_title("URL");
    url_group.add(&url_switch);
    url_group.add(&url_entry);

    let form_page = adw::PreferencesPage::new();
    form_page.add(&details_group);
    form_page.add(&access_group);
    form_page.add(&url_group);
    let form_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&form_page)
        .build();
    let form_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    form_actions.set_halign(gtk::Align::End);
    form_actions.set_margin_all(12);
    let test_button = gtk::Button::with_label("Test");
    {
        let sender = sender.clone();
        test_button.connect_clicked(move |_| sender.input(AppMsg::TestForm));
    }
    let cancel_button = gtk::Button::with_label("Cancel");
    {
        let sender = sender.clone();
        cancel_button.connect_clicked(move |_| sender.input(AppMsg::CancelForm));
    }
    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        save_button.connect_clicked(move |_| sender.input(AppMsg::SaveForm));
    }
    form_actions.append(&test_button);
    form_actions.append(&cancel_button);
    form_actions.append(&save_button);
    let form_page_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    form_page_content.append(&form_scroll);
    form_page_content.append(&form_actions);

    let connections_stack = gtk::Stack::new();
    connections_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    connections_stack.add_named(&list_page_content, Some("list"));
    connections_stack.add_named(&form_page_content, Some("form"));

    // Transfer page.
    let transfer_status = gtk::Label::builder()
        .label("Choose a connection, table, and file.")
        .xalign(0.0)
        .wrap(true)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();
    let transfer_conn_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    transfer_conn_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        transfer_conn_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::TransferConnectionSelected(dropdown.selected()));
        });
    }
    let transfer_table_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    {
        let sender = sender.clone();
        transfer_table_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::TransferTableSelected(dropdown.selected()));
        });
    }
    let transfer_format_dropdown = gtk::DropDown::new(
        Some(gtk::StringList::new(&["CSV", "JSON"])),
        None::<gtk::Expression>,
    );
    let transfer_headers_check = gtk::CheckButton::with_label("First row is a header");
    transfer_headers_check.set_active(true);
    let transfer_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    transfer_toolbar.set_margin_all(10);
    transfer_toolbar.append(&transfer_conn_dropdown);
    transfer_toolbar.append(&transfer_table_dropdown);
    transfer_toolbar.append(&transfer_format_dropdown);
    transfer_toolbar.append(&transfer_headers_check);
    let transfer_path_entry = gtk::Entry::builder()
        .placeholder_text("Path to a CSV or JSON file")
        .hexpand(true)
        .build();
    let transfer_browse_button = gtk::Button::with_label("Browse");
    {
        let sender = sender.clone();
        transfer_browse_button.connect_clicked(move |_| sender.input(AppMsg::TransferBrowse));
    }
    let transfer_import_button = gtk::Button::with_label("Import");
    transfer_import_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        transfer_import_button.connect_clicked(move |_| sender.input(AppMsg::TransferImport));
    }
    let transfer_path_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    transfer_path_row.set_margin_start(10);
    transfer_path_row.set_margin_end(10);
    transfer_path_row.append(&transfer_path_entry);
    transfer_path_row.append(&transfer_browse_button);
    transfer_path_row.append(&transfer_import_button);
    let transfer_hint = gtk::Label::builder()
        .label("Rows are coerced to the table column types and inserted in transactional batches.")
        .xalign(0.0)
        .wrap(true)
        .margin_start(10)
        .margin_top(8)
        .build();
    transfer_hint.add_css_class("unidb-muted");
    let transfer_form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    transfer_form.append(&transfer_toolbar);
    transfer_form.append(&transfer_path_row);
    transfer_form.append(&transfer_hint);
    let fake_rows_label = gtk::Label::new(Some("Fake rows"));
    let fake_count_spin = gtk::SpinButton::with_range(1.0, 10000.0, 1.0);
    fake_count_spin.set_value(5.0);
    fake_count_spin.set_numeric(true);
    let fake_button = gtk::Button::with_label("Generate fake rows");
    {
        let sender = sender.clone();
        fake_button.connect_clicked(move |_| sender.input(AppMsg::GenerateFakeData));
    }
    let fake_row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    fake_row_box.set_margin_start(10);
    fake_row_box.set_margin_end(10);
    fake_row_box.set_margin_top(6);
    fake_row_box.append(&fake_rows_label);
    fake_row_box.append(&fake_count_spin);
    fake_row_box.append(&fake_button);
    transfer_form.append(&fake_row_box);
    transfer_form.append(&transfer_status);
    let transfer_empty = adw::StatusPage::builder()
        .icon_name("document-send-symbolic")
        .title("No live connection")
        .description("Connect to a database in Connections, then import a file here.")
        .build();
    let transfer_stack = gtk::Stack::new();
    transfer_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    transfer_stack.add_named(&transfer_empty, Some("empty"));
    transfer_stack.add_named(&transfer_form, Some("form"));
    transfer_stack.set_visible_child_name("empty");

    // Designer page.
    let designer_status = gtk::Label::builder()
        .label("Connect to a database first.")
        .xalign(0.0)
        .wrap(true)
        .build();
    let designer_columns_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    designer_columns_box.set_margin_all(8);
    let add_column_button = gtk::Button::with_label("Add column");
    {
        let sender = sender.clone();
        add_column_button.connect_clicked(move |_| sender.input(AppMsg::DesignerAddColumn));
    }
    let designer_name_entry = gtk::Entry::builder()
        .placeholder_text("Table name")
        .hexpand(true)
        .build();
    let designer_table_dropdown = gtk::DropDown::new(
        Some(gtk::StringList::new(&["New table"])),
        None::<gtk::Expression>,
    );
    {
        let sender = sender.clone();
        designer_table_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::DesignerTableSelected(dropdown.selected()));
        });
    }
    let designer_conn_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    designer_conn_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        designer_conn_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::DesignerConnectionSelected(dropdown.selected()));
        });
    }
    let designer_preview_button = gtk::Button::with_label("Preview SQL");
    {
        let sender = sender.clone();
        designer_preview_button.connect_clicked(move |_| sender.input(AppMsg::DesignerPreview));
    }
    let designer_apply_button = gtk::Button::with_label("Apply");
    designer_apply_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        designer_apply_button.connect_clicked(move |_| sender.input(AppMsg::DesignerApply));
    }
    let designer_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    designer_toolbar.set_margin_all(10);
    designer_toolbar.append(&designer_conn_dropdown);
    designer_toolbar.append(&designer_table_dropdown);
    designer_toolbar.append(&designer_preview_button);
    designer_toolbar.append(&designer_apply_button);
    let designer_name_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    designer_name_row.set_margin_start(10);
    designer_name_row.set_margin_end(10);
    designer_name_row.set_margin_top(4);
    designer_name_row.append(&designer_name_entry);
    designer_name_row.append(&add_column_button);
    let column_header = gtk::Label::builder()
        .label("Columns: name, type, primary key, not null, unique, auto increment, default")
        .xalign(0.0)
        .build();
    column_header.add_css_class("unidb-muted");
    column_header.set_margin_start(10);
    column_header.set_margin_top(8);
    let form_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    form_box.append(&designer_name_row);
    form_box.append(&column_header);
    form_box.append(&designer_columns_box);
    let form_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&form_box)
        .build();
    let designer_preview_buffer = sourceview5::Buffer::new(None::<&gtk::TextTagTable>);
    if let Some(language) = sourceview5::LanguageManager::default().language("sql") {
        designer_preview_buffer.set_language(Some(&language));
    }
    designer_preview_buffer.set_highlight_syntax(true);
    if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita") {
        designer_preview_buffer.set_style_scheme(Some(&scheme));
    }
    let designer_preview = sourceview5::View::with_buffer(&designer_preview_buffer);
    designer_preview.set_monospace(true);
    designer_preview.set_editable(false);
    designer_preview.set_cursor_visible(false);
    designer_preview.set_vexpand(true);
    let preview_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&designer_preview)
        .build();
    let preview_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let preview_label = gtk::Label::builder()
        .label("Preview")
        .xalign(0.0)
        .margin_start(10)
        .margin_top(6)
        .build();
    preview_label.add_css_class("heading");
    preview_box.append(&preview_label);
    preview_box.append(&preview_scroll);
    let designer_paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Vertical)
        .start_child(&form_scroll)
        .end_child(&preview_box)
        .position(300)
        .vexpand(true)
        .build();
    let designer_form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    designer_form.append(&designer_toolbar);
    designer_form.append(&designer_paned);
    designer_form.append(&designer_status);
    let designer_empty = adw::StatusPage::builder()
        .icon_name("x-office-spreadsheet-symbolic")
        .title("No live connection")
        .description("Connect to a saved SQL database in Connections, then open the designer.")
        .build();
    let designer_unsupported = adw::StatusPage::builder()
        .icon_name("x-office-spreadsheet-symbolic")
        .title("Designer not available")
        .description("The table designer supports SQLite, PostgreSQL, CockroachDB, MySQL, and MariaDB in this build.")
        .build();
    let designer_stack = gtk::Stack::new();
    designer_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    designer_stack.add_named(&designer_empty, Some("empty"));
    designer_stack.add_named(&designer_unsupported, Some("unsupported"));
    designer_stack.add_named(&designer_form, Some("form"));
    designer_stack.set_visible_child_name("empty");

    // Editor page.
    let editor_status = gtk::Label::builder()
        .label("Connect to a database first.")
        .xalign(0.0)
        .wrap(true)
        .build();
    let editor_error = gtk::Label::builder()
        .label("")
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .build();
    editor_error.add_css_class("status-fail");
    let editor_grid = gtk::Grid::new();
    editor_grid.set_row_spacing(6);
    editor_grid.set_column_spacing(18);
    editor_grid.set_margin_all(12);
    let editor_grid_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&editor_grid)
        .build();
    let chart_data: Rc<RefCell<Vec<ChartPoint>>> = Rc::new(RefCell::new(Vec::new()));
    let chart_area = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .height_request(240)
        .build();
    {
        let data = Rc::clone(&chart_data);
        chart_area.set_draw_func(move |area, context, width, height| {
            draw_chart(area, context, width, height, &data.borrow());
        });
    }
    let editor_results_stack = gtk::Stack::new();
    editor_results_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    editor_results_stack.set_vexpand(true);
    editor_results_stack.add_named(&editor_grid_scroll, Some("grid"));
    editor_results_stack.add_named(&chart_area, Some("chart"));
    editor_results_stack.set_visible_child_name("grid");
    let plan_buffer = sourceview5::Buffer::new(None::<&gtk::TextTagTable>);
    plan_buffer.set_highlight_syntax(false);
    let plan_view = sourceview5::View::with_buffer(&plan_buffer);
    plan_view.set_monospace(true);
    plan_view.set_editable(false);
    plan_view.set_cursor_visible(false);
    plan_view.set_top_margin(6);
    plan_view.set_bottom_margin(6);
    plan_view.set_left_margin(6);
    let plan_scroll = gtk::ScrolledWindow::builder()
        .height_request(180)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&plan_view)
        .build();
    let plan_revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .reveal_child(false)
        .child(&plan_scroll)
        .build();
    let run_multi_button = gtk::Button::with_label("Run on all");
    run_multi_button.set_tooltip_text(Some("Run this SQL across all open live connections"));
    {
        let sender = sender.clone();
        run_multi_button.connect_clicked(move |_| sender.input(AppMsg::RunMultiQuery));
    }
    editor_status.set_hexpand(true);
    let editor_status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    editor_status_bar.append(&editor_status);
    editor_status_bar.append(&run_multi_button);

    let results_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    results_box.set_margin_top(8);
    results_box.set_margin_bottom(8);
    results_box.set_margin_start(12);
    results_box.set_margin_end(12);
    results_box.append(&editor_status_bar);
    results_box.append(&editor_error);
    results_box.append(&editor_results_stack);
    results_box.append(&plan_revealer);
    let results_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&results_box)
        .build();

    let editor_buffer = sourceview5::Buffer::new(None::<&gtk::TextTagTable>);
    if let Some(language) = sourceview5::LanguageManager::default().language("sql") {
        editor_buffer.set_language(Some(&language));
    }
    editor_buffer.set_highlight_syntax(true);
    editor_buffer.set_highlight_matching_brackets(true);
    if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita") {
        editor_buffer.set_style_scheme(Some(&scheme));
    }
    let editor_view = sourceview5::View::with_buffer(&editor_buffer);
    editor_view.set_monospace(true);
    editor_view.set_show_line_numbers(true);
    editor_view.set_auto_indent(true);
    editor_view.set_tab_width(4);
    editor_view.set_wrap_mode(gtk::WrapMode::None);
    editor_view.set_top_margin(8);
    editor_view.set_bottom_margin(8);
    editor_view.set_left_margin(8);
    editor_view.set_hexpand(true);
    editor_view.set_vexpand(true);
    {
        let sender = sender.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, state| {
            if key == gtk::gdk::Key::Return && state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                sender.input(AppMsg::RunQuery);
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        editor_view.add_controller(keys);
    }
    let editor_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&editor_view)
        .build();
    let editor_paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Vertical)
        .start_child(&editor_scroll)
        .end_child(&results_scroll)
        .position(300)
        .resize_start_child(true)
        .shrink_start_child(false)
        .resize_end_child(true)
        .shrink_end_child(false)
        .vexpand(true)
        .build();

    let editor_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    editor_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        editor_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::EditorConnectionSelected(dropdown.selected()));
        });
    }
    let run_button = gtk::Button::with_label("Run");
    run_button.add_css_class("suggested-action");
    run_button.set_tooltip_text(Some("Run the statement (Ctrl+Enter)"));
    {
        let sender = sender.clone();
        run_button.connect_clicked(move |_| sender.input(AppMsg::RunQuery));
    }
    let cancel_button = gtk::Button::with_label("Cancel");
    cancel_button.set_sensitive(false);
    {
        let sender = sender.clone();
        cancel_button.connect_clicked(move |_| sender.input(AppMsg::CancelQuery));
    }
    let clear_button = gtk::Button::with_label("Clear results");
    {
        let sender = sender.clone();
        clear_button.connect_clicked(move |_| sender.input(AppMsg::ClearResults));
    }
    let explain_button = gtk::Button::with_label("Explain");
    explain_button.set_tooltip_text(Some("Run the engine explain statement for the editor SQL"));
    {
        let sender = sender.clone();
        explain_button.connect_clicked(move |_| sender.input(AppMsg::ExplainQuery));
    }
    let chart_button = gtk::Button::with_label("Chart");
    chart_button.set_sensitive(false);
    {
        let sender = sender.clone();
        chart_button.connect_clicked(move |_| sender.input(AppMsg::ToggleChart));
    }
    let compare_button = gtk::Button::with_label("Compare");
    compare_button.set_tooltip_text(Some(
        "Capture a baseline, then compare against the next run",
    ));
    {
        let sender = sender.clone();
        compare_button.connect_clicked(move |_| sender.input(AppMsg::CompareResults));
    }
    let export_button = gtk::MenuButton::builder()
        .label("Export")
        .tooltip_text("Export the current result")
        .build();
    let export_popover = gtk::Popover::new();
    let export_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    export_box.set_margin_all(6);
    let export_csv_button = gtk::Button::with_label("CSV");
    {
        let sender = sender.clone();
        export_csv_button
            .connect_clicked(move |_| sender.input(AppMsg::ExportQuery(TransferFormat::Csv)));
    }
    let export_json_button = gtk::Button::with_label("JSON");
    {
        let sender = sender.clone();
        export_json_button
            .connect_clicked(move |_| sender.input(AppMsg::ExportQuery(TransferFormat::Json)));
    }
    export_box.append(&export_csv_button);
    export_box.append(&export_json_button);
    let export_csv_masked = gtk::Button::with_label("CSV masked");
    {
        let sender = sender.clone();
        export_csv_masked
            .connect_clicked(move |_| sender.input(AppMsg::ExportQueryMasked(TransferFormat::Csv)));
    }
    let export_json_masked = gtk::Button::with_label("JSON masked");
    {
        let sender = sender.clone();
        export_json_masked.connect_clicked(move |_| {
            sender.input(AppMsg::ExportQueryMasked(TransferFormat::Json))
        });
    }
    export_box.append(&export_csv_masked);
    export_box.append(&export_json_masked);
    export_popover.set_child(Some(&export_box));
    export_button.set_popover(Some(&export_popover));
    let editor_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    editor_toolbar.set_margin_all(10);
    editor_toolbar.append(&editor_dropdown);
    editor_toolbar.append(&run_button);
    editor_toolbar.append(&cancel_button);
    editor_toolbar.append(&clear_button);
    editor_toolbar.append(&explain_button);
    editor_toolbar.append(&chart_button);
    editor_toolbar.append(&compare_button);
    editor_toolbar.append(&export_button);
    let editor_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_page.append(&editor_toolbar);
    editor_page.append(&editor_paned);

    // Navigator page.
    let navigator_stack = gtk::Stack::new();
    navigator_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    let navigator_empty = adw::StatusPage::builder()
        .icon_name("view-list-symbolic")
        .title("No live connection")
        .description("Connect to a saved database in Connections, then open the navigator.")
        .build();
    let navigator_combo = adw::ComboRow::builder().title("Connection").build();
    {
        let sender = sender.clone();
        navigator_combo.connect_selected_notify(move |combo| {
            sender.input(AppMsg::NavigatorConnectionSelected(combo.selected()));
        });
    }
    let databases_group = adw::PreferencesGroup::builder().title("Databases").build();
    let navigator_databases_list = gtk::ListBox::new();
    navigator_databases_list.set_selection_mode(gtk::SelectionMode::Single);
    navigator_databases_list.add_css_class("boxed-list");
    databases_group.add(&navigator_databases_list);
    let tables_group = adw::PreferencesGroup::builder().title("Tables").build();
    let navigator_tables_list = gtk::ListBox::new();
    navigator_tables_list.set_selection_mode(gtk::SelectionMode::None);
    navigator_tables_list.add_css_class("boxed-list");
    tables_group.add(&navigator_tables_list);
    let connection_group = adw::PreferencesGroup::builder().title("Connection").build();
    connection_group.add(&navigator_combo);
    let navigator_prefs = adw::PreferencesPage::new();
    navigator_prefs.add(&connection_group);
    navigator_prefs.add(&databases_group);
    navigator_prefs.add(&tables_group);
    let navigator_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&navigator_prefs)
        .build();
    let navigator_browse = gtk::Box::new(gtk::Orientation::Vertical, 0);
    navigator_browse.append(&navigator_scroll);
    navigator_stack.add_named(&navigator_empty, Some("empty"));
    navigator_stack.add_named(&navigator_browse, Some("browse"));
    navigator_stack.set_visible_child_name("empty");

    // History page.
    let history_search_entry = gtk::Entry::builder()
        .placeholder_text("Search history")
        .hexpand(true)
        .build();
    {
        let sender = sender.clone();
        history_search_entry.connect_activate(move |_| sender.input(AppMsg::LoadHistory));
    }
    let history_search_button = gtk::Button::with_label("Search");
    {
        let sender = sender.clone();
        history_search_button.connect_clicked(move |_| sender.input(AppMsg::LoadHistory));
    }
    let clear_history_button = gtk::Button::with_label("Clear history");
    clear_history_button.add_css_class("destructive-action");
    {
        let sender = sender.clone();
        clear_history_button.connect_clicked(move |_| sender.input(AppMsg::ClearHistoryRequested));
    }
    let history_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    history_toolbar.set_margin_all(10);
    history_toolbar.append(&history_search_entry);
    history_toolbar.append(&history_search_button);
    history_toolbar.append(&clear_history_button);
    let history_status = gtk::Label::builder()
        .label("No history loaded.")
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    history_status.add_css_class("unidb-muted");
    let history_list = gtk::ListBox::new();
    history_list.set_selection_mode(gtk::SelectionMode::None);
    history_list.add_css_class("boxed-list");
    let history_group = adw::PreferencesGroup::builder()
        .title("Recent statements")
        .build();
    history_group.add(&history_list);
    let saved_name_entry = gtk::Entry::builder()
        .placeholder_text("Name for the current statement")
        .hexpand(true)
        .build();
    let save_query_button = gtk::Button::with_label("Save current SQL");
    save_query_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        save_query_button.connect_clicked(move |_| sender.input(AppMsg::SaveCurrentQuery));
    }
    let saved_row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    saved_row_box.append(&saved_name_entry);
    saved_row_box.append(&save_query_button);
    let saved_list = gtk::ListBox::new();
    saved_list.set_selection_mode(gtk::SelectionMode::None);
    saved_list.add_css_class("boxed-list");
    let saved_group = adw::PreferencesGroup::builder()
        .title("Saved queries")
        .build();
    saved_group.add(&saved_row_box);
    saved_group.add(&saved_list);

    let snapshots_conn_dropdown = gtk::DropDown::builder()
        .tooltip_text("Select connection for table snapshots")
        .build();
    {
        let sender = sender.clone();
        snapshots_conn_dropdown.connect_selected_notify(move |dropdown| {
            let selected = dropdown.selected();
            if selected != gtk::INVALID_LIST_POSITION {
                sender.input(AppMsg::SnapshotsConnChanged(selected as usize));
            }
        });
    }
    let snapshots_table_entry = gtk::Entry::builder()
        .placeholder_text("Table name (e.g. users)")
        .hexpand(true)
        .build();
    let snapshots_note_entry = gtk::Entry::builder()
        .placeholder_text("Note (optional)")
        .hexpand(true)
        .build();
    let snapshots_create_button = gtk::Button::with_label("Take snapshot");
    snapshots_create_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        snapshots_create_button.connect_clicked(move |_| sender.input(AppMsg::TakeSnapshot));
    }
    let snapshots_refresh_button = gtk::Button::with_label("Refresh");
    {
        let sender = sender.clone();
        snapshots_refresh_button.connect_clicked(move |_| sender.input(AppMsg::SnapshotsRefresh));
    }
    let snapshots_row1 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    snapshots_row1.append(&snapshots_conn_dropdown);
    snapshots_row1.append(&snapshots_table_entry);
    snapshots_row1.append(&snapshots_refresh_button);

    let snapshots_row2 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    snapshots_row2.append(&snapshots_note_entry);
    snapshots_row2.append(&snapshots_create_button);

    let snapshots_controls_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    snapshots_controls_box.set_margin_bottom(6);
    snapshots_controls_box.append(&snapshots_row1);
    snapshots_controls_box.append(&snapshots_row2);

    let snapshots_status = gtk::Label::builder()
        .label("Select a connection to view or capture snapshots.")
        .xalign(0.0)
        .wrap(true)
        .margin_bottom(6)
        .build();
    snapshots_status.add_css_class("unidb-muted");

    let snapshots_list = gtk::ListBox::new();
    snapshots_list.set_selection_mode(gtk::SelectionMode::None);
    snapshots_list.add_css_class("boxed-list");

    let snapshots_group = adw::PreferencesGroup::builder()
        .title("Table snapshots")
        .description("Point in time backups with keyed diff and transactional restore")
        .build();
    snapshots_group.add(&snapshots_controls_box);
    snapshots_group.add(&snapshots_status);
    snapshots_group.add(&snapshots_list);

    let history_page = adw::PreferencesPage::new();
    history_page.add(&history_group);
    history_page.add(&saved_group);
    history_page.add(&snapshots_group);
    let history_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&history_page)
        .build();
    let history_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    history_box.append(&history_toolbar);
    history_box.append(&history_status);
    history_box.append(&history_scroll);

    // Settings page.
    let settings_combo = adw::ComboRow::builder()
        .title("Color scheme")
        .subtitle("Applies to the whole application")
        .model(&gtk::StringList::new(&["System", "Light", "Dark"]))
        .build();
    {
        let sender = sender.clone();
        settings_combo.connect_selected_notify(move |combo| {
            sender.input(AppMsg::SetColorScheme(combo.selected()));
        });
    }
    let appearance_group = adw::PreferencesGroup::builder().title("Appearance").build();
    appearance_group.add(&settings_combo);
    let about_group = adw::PreferencesGroup::builder().title("About").build();
    let version_row = adw::ActionRow::builder()
        .title("UniDB")
        .subtitle(format!("Version {}", env!("CARGO_PKG_VERSION")))
        .build();
    about_group.add(&version_row);
    let engine_row = adw::ActionRow::builder()
        .title("Engines")
        .subtitle(
            "SQLite, PostgreSQL, CockroachDB, MySQL, MariaDB, Redis, MongoDB, Cassandra, MSSQL",
        )
        .build();
    about_group.add(&engine_row);
    let updates_group = adw::PreferencesGroup::builder().title("Updates").build();
    let update_row = adw::ActionRow::builder()
        .title("Check for updates")
        .subtitle(if sender_has_update_feed() {
            "Asks the release feed whether a newer version exists"
        } else {
            "Set UNIDB_UPDATE_FEED to enable the check"
        })
        .build();
    let update_button = gtk::Button::with_label("Check");
    update_button.set_valign(gtk::Align::Center);
    update_button.set_sensitive(sender_has_update_feed());
    {
        let sender = sender.clone();
        update_button.connect_clicked(move |_| sender.input(AppMsg::CheckUpdates));
    }
    update_row.add_suffix(&update_button);
    updates_group.add(&update_row);
    let update_status = gtk::Label::builder()
        .label(format!("Running version {}", env!("CARGO_PKG_VERSION")))
        .xalign(0.0)
        .wrap(true)
        .build();
    update_status.add_css_class("unidb-muted");
    let update_status_row = adw::ActionRow::new();
    update_status_row.set_child(Some(&update_status));
    updates_group.add(&update_status_row);
    let settings_page = adw::PreferencesPage::new();
    settings_page.add(&appearance_group);
    settings_page.add(&updates_group);
    settings_page.add(&about_group);
    let settings_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&settings_page)
        .build();

    let checks_group = adw::PreferencesGroup::builder()
        .title("Security checks")
        .build();
    let audit_group = adw::PreferencesGroup::builder()
        .title("Recent activity")
        .build();
    let security_page = adw::PreferencesPage::new();
    security_page.add(&checks_group);
    security_page.add(&audit_group);
    let security_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&security_page)
        .build();

    // Builder page.
    let builder_status = gtk::Label::builder()
        .label("Choose a live connection.")
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    builder_status.add_css_class("unidb-muted");
    let builder_conn_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    builder_conn_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        builder_conn_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::BuilderConnectionSelected(dropdown.selected()));
        });
    }
    let builder_table_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    {
        let sender = sender.clone();
        builder_table_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::BuilderTableSelected(dropdown.selected()));
        });
    }
    let builder_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    builder_toolbar.set_margin_all(10);
    builder_toolbar.append(&builder_conn_dropdown);
    builder_toolbar.append(&builder_table_dropdown);

    let builder_columns_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    builder_columns_box.set_margin_all(6);
    let builder_columns_scroll = gtk::ScrolledWindow::builder()
        .height_request(120)
        .vexpand(false)
        .child(&builder_columns_box)
        .build();
    let builder_columns_group = adw::PreferencesGroup::builder().title("Columns").build();
    builder_columns_group.add(&builder_columns_scroll);

    let builder_filter_column =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    builder_filter_column.set_hexpand(true);
    let op_labels = [
        "=",
        "<>",
        ">",
        ">=",
        "<",
        "<=",
        "LIKE",
        "IS NULL",
        "IS NOT NULL",
    ];
    let builder_filter_op = gtk::DropDown::new(
        Some(gtk::StringList::new(&op_labels)),
        None::<gtk::Expression>,
    );
    let builder_filter_value = gtk::Entry::builder()
        .placeholder_text("value")
        .width_chars(12)
        .build();
    let builder_add_filter = gtk::Button::with_label("Add filter");
    {
        let sender = sender.clone();
        builder_add_filter.connect_clicked(move |_| sender.input(AppMsg::BuilderAddFilter));
    }
    let builder_filter_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    builder_filter_controls.set_margin_all(6);
    builder_filter_controls.append(&builder_filter_column);
    builder_filter_controls.append(&builder_filter_op);
    builder_filter_controls.append(&builder_filter_value);
    builder_filter_controls.append(&builder_add_filter);
    let builder_filters_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let builder_filters_group = adw::PreferencesGroup::builder().title("Filters").build();
    builder_filters_group.add(&builder_filter_controls);
    builder_filters_group.add(&builder_filters_box);

    let builder_order_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    builder_order_dropdown.set_hexpand(true);
    let builder_order_asc = gtk::Switch::new();
    builder_order_asc.set_active(true);
    let builder_limit_spin = gtk::SpinButton::with_range(1.0, 10000.0, 10.0);
    builder_limit_spin.set_value(100.0);
    builder_limit_spin.set_numeric(true);
    let builder_order_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    builder_order_box.set_margin_all(6);
    let order_label = gtk::Label::new(Some("Order by"));
    builder_order_box.append(&order_label);
    builder_order_box.append(&builder_order_dropdown);
    let asc_label = gtk::Label::new(Some("Ascending"));
    builder_order_box.append(&asc_label);
    builder_order_box.append(&builder_order_asc);
    let limit_label = gtk::Label::new(Some("Limit"));
    builder_order_box.append(&limit_label);
    builder_order_box.append(&builder_limit_spin);
    let builder_order_group = adw::PreferencesGroup::builder()
        .title("Order and limit")
        .build();
    builder_order_group.add(&builder_order_box);

    let builder_preview_button = gtk::Button::with_label("Preview SQL");
    {
        let sender = sender.clone();
        builder_preview_button.connect_clicked(move |_| sender.input(AppMsg::BuilderPreview));
    }
    let builder_editor_button = gtk::Button::with_label("Open in editor");
    builder_editor_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        builder_editor_button.connect_clicked(move |_| sender.input(AppMsg::BuilderOpenInEditor));
    }
    let builder_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    builder_actions.set_margin_all(6);
    builder_actions.set_halign(gtk::Align::End);
    builder_actions.append(&builder_preview_button);
    builder_actions.append(&builder_editor_button);
    let builder_actions_group = adw::PreferencesGroup::builder().build();
    builder_actions_group.add(&builder_actions);

    let builder_preview_buffer = sourceview5::Buffer::new(None::<&gtk::TextTagTable>);
    if let Some(language) = sourceview5::LanguageManager::default().language("sql") {
        builder_preview_buffer.set_language(Some(&language));
    }
    builder_preview_buffer.set_highlight_syntax(true);
    if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita") {
        builder_preview_buffer.set_style_scheme(Some(&scheme));
    }
    let builder_preview = sourceview5::View::with_buffer(&builder_preview_buffer);
    builder_preview.set_monospace(true);
    builder_preview.set_editable(false);
    let builder_preview_scroll = gtk::ScrolledWindow::builder()
        .height_request(140)
        .child(&builder_preview)
        .build();
    let builder_preview_group = adw::PreferencesGroup::builder().title("Preview").build();
    builder_preview_group.add(&builder_preview_scroll);

    let builder_page_prefs = adw::PreferencesPage::new();
    builder_page_prefs.add(&builder_columns_group);
    builder_page_prefs.add(&builder_filters_group);
    builder_page_prefs.add(&builder_order_group);
    builder_page_prefs.add(&builder_actions_group);
    builder_page_prefs.add(&builder_preview_group);
    let builder_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&builder_page_prefs)
        .build();
    let builder_form = gtk::Box::new(gtk::Orientation::Vertical, 4);
    builder_form.append(&builder_toolbar);
    builder_form.append(&builder_status);
    builder_form.append(&builder_scroll);
    let builder_empty = adw::StatusPage::builder()
        .icon_name("document-edit-symbolic")
        .title("No live connection")
        .description("Connect to a SQL database, then build a query here.")
        .build();
    let builder_stack = gtk::Stack::new();
    builder_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    builder_stack.add_named(&builder_empty, Some("empty"));
    builder_stack.add_named(&builder_form, Some("form"));
    builder_stack.set_visible_child_name("empty");

    // Schema page.
    let schema_status = gtk::Label::builder()
        .label("Choose a live connection.")
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    schema_status.add_css_class("unidb-muted");
    let schema_conn_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    schema_conn_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        schema_conn_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::SchemaConnectionSelected(dropdown.selected()));
        });
    }
    let schema_refresh_button = gtk::Button::with_label("Refresh");
    {
        let sender = sender.clone();
        schema_refresh_button.connect_clicked(move |_| sender.input(AppMsg::SchemaRefresh));
    }
    let schema_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    schema_toolbar.set_margin_all(10);
    schema_toolbar.append(&schema_conn_dropdown);
    schema_toolbar.append(&schema_refresh_button);
    let schema_tables: Rc<RefCell<Vec<ErdTable>>> = Rc::new(RefCell::new(Vec::new()));
    let schema_canvas = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .height_request(480)
        .build();
    {
        let tables = Rc::clone(&schema_tables);
        schema_canvas.set_draw_func(move |_area, context, width, height| {
            draw_erd(context, width, height, &tables.borrow());
        });
    }
    let schema_scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&schema_canvas)
        .build();
    let schema_form = gtk::Box::new(gtk::Orientation::Vertical, 4);
    schema_form.append(&schema_toolbar);
    schema_form.append(&schema_status);
    schema_form.append(&schema_scroll);
    let schema_empty = adw::StatusPage::builder()
        .icon_name("view-grid-symbolic")
        .title("No live connection")
        .description(
            "Connect to a SQL database in Connections, then open Schema to draw its relationships.",
        )
        .build();
    let schema_stack = gtk::Stack::new();
    schema_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    schema_stack.add_named(&schema_empty, Some("empty"));
    schema_stack.add_named(&schema_form, Some("form"));
    schema_stack.set_visible_child_name("empty");

    // Migrations page.
    let migrations_status = gtk::Label::builder()
        .label("Scan a migrations directory to see pending files.")
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    migrations_status.add_css_class("unidb-muted");
    let migrations_conn_dropdown =
        gtk::DropDown::new(Some(gtk::StringList::new(&[])), None::<gtk::Expression>);
    migrations_conn_dropdown.set_hexpand(true);
    {
        let sender = sender.clone();
        migrations_conn_dropdown.connect_selected_notify(move |dropdown| {
            sender.input(AppMsg::MigrationsConnChanged(dropdown.selected() as usize));
        });
    }
    let migrations_dir_entry = gtk::Entry::builder()
        .text("migrations")
        .placeholder_text("Directory with NNNN_name.sql files")
        .hexpand(true)
        .build();
    let migrations_scan_button = gtk::Button::with_label("Scan");
    {
        let sender = sender.clone();
        migrations_scan_button.connect_clicked(move |_| sender.input(AppMsg::MigrationsScan));
    }
    let migrations_apply_button = gtk::Button::with_label("Apply Pending");
    migrations_apply_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        migrations_apply_button.connect_clicked(move |_| sender.input(AppMsg::MigrationsApply));
    }
    let migrations_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    migrations_toolbar.set_margin_all(10);
    migrations_toolbar.append(&migrations_conn_dropdown);
    migrations_toolbar.append(&migrations_dir_entry);
    migrations_toolbar.append(&migrations_scan_button);
    migrations_toolbar.append(&migrations_apply_button);

    let migrations_list = gtk::ListBox::new();
    migrations_list.set_selection_mode(gtk::SelectionMode::Single);
    migrations_list.add_css_class("boxed-list");
    {
        let sender = sender.clone();
        migrations_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                sender.input(AppMsg::MigrationsSelectFile(row.index() as usize));
            }
        });
    }
    let migrations_list_scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(180)
        .child(&migrations_list)
        .build();

    let migrations_preview_buffer = sourceview5::Buffer::new(None::<&gtk::TextTagTable>);
    if let Some(language) = sourceview5::LanguageManager::default().language("sql") {
        migrations_preview_buffer.set_language(Some(&language));
    }
    migrations_preview_buffer.set_highlight_syntax(true);
    if let Some(scheme) = sourceview5::StyleSchemeManager::default().scheme("Adwaita") {
        migrations_preview_buffer.set_style_scheme(Some(&scheme));
    }
    let migrations_preview_view = sourceview5::View::builder()
        .buffer(&migrations_preview_buffer)
        .monospace(true)
        .show_line_numbers(true)
        .editable(false)
        .hexpand(true)
        .vexpand(true)
        .build();
    let migrations_preview_scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(180)
        .child(&migrations_preview_view)
        .build();

    let migrations_paned = gtk::Paned::new(gtk::Orientation::Vertical);
    migrations_paned.set_start_child(Some(&migrations_list_scroll));
    migrations_paned.set_end_child(Some(&migrations_preview_scroll));
    migrations_paned.set_position(220);
    migrations_paned.set_hexpand(true);
    migrations_paned.set_vexpand(true);

    let migrations_form = gtk::Box::new(gtk::Orientation::Vertical, 4);
    migrations_form.append(&migrations_toolbar);
    migrations_form.append(&migrations_status);
    migrations_form.append(&migrations_paned);

    let migrations_empty = adw::StatusPage::builder()
        .icon_name("system-software-update-symbolic")
        .title("No live connection")
        .description("Connect to a database in Connections, then manage migrations here.")
        .build();
    let migrations_stack = gtk::Stack::new();
    migrations_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    migrations_stack.add_named(&migrations_empty, Some("empty"));
    migrations_stack.add_named(&migrations_form, Some("form"));
    migrations_stack.set_visible_child_name("empty");

    let page_stack = gtk::Stack::new();
    page_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    page_stack.add_named(&connections_stack, Some("connections"));
    page_stack.add_named(&navigator_stack, Some("navigator"));
    page_stack.add_named(&editor_page, Some("editor"));
    page_stack.add_named(&designer_stack, Some("designer"));
    page_stack.add_named(&transfer_stack, Some("transfer"));
    page_stack.add_named(&history_box, Some("history"));
    page_stack.add_named(&settings_scroll, Some("settings"));
    page_stack.add_named(&schema_stack, Some("schema"));
    page_stack.add_named(&builder_stack, Some("builder"));
    page_stack.add_named(&migrations_stack, Some("migrations"));
    page_stack.add_named(&security_scroll, Some("security"));

    let content_page = adw::NavigationPage::new(&page_stack, "Connections");
    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.add_css_class("navigation-sidebar");
    for (icon, label) in [
        ("network-server-symbolic", "Connections"),
        ("view-list-symbolic", "Navigator"),
        ("utilities-terminal-symbolic", "Editor"),
        ("x-office-spreadsheet-symbolic", "Designer"),
        ("document-send-symbolic", "Transfer"),
        ("document-open-recent-symbolic", "History"),
        ("emblem-system-symbolic", "Settings"),
        ("view-grid-symbolic", "Schema"),
        ("document-edit-symbolic", "Builder"),
        ("system-software-update-symbolic", "Migrations"),
        ("security-high-symbolic", "Security"),
    ] {
        let row = gtk::ListBoxRow::new();
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);
        row_box.append(&gtk::Image::from_icon_name(icon));
        let label = gtk::Label::new(Some(label));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        row_box.append(&label);
        row.set_child(Some(&row_box));
        sidebar.append(&row);
    }
    {
        let sender = sender.clone();
        sidebar.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                sender.input(AppMsg::SelectSection(row.index() as usize));
            }
        });
    }
    if let Some(first) = sidebar.row_at_index(0) {
        sidebar.select_row(Some(&first));
    }
    let sidebar_page = adw::NavigationPage::new(&sidebar, "Sections");

    let split_view = adw::NavigationSplitView::new();
    split_view.set_sidebar(Some(&sidebar_page));
    split_view.set_content(Some(&content_page));
    split_view.set_min_sidebar_width(200.0);
    split_view.set_max_sidebar_width(260.0);

    let diagnostic_page = adw::StatusPage::builder()
        .icon_name("dialog-warning-symbolic")
        .title("UniDB could not start")
        .description("The app database could not be opened.")
        .build();
    let retry_button = gtk::Button::with_label("Retry");
    retry_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        retry_button.connect_clicked(move |_| sender.input(AppMsg::RetryBootstrap));
    }
    let diagnostic_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    diagnostic_box.append(&retry_button);
    diagnostic_page.set_child(Some(&diagnostic_box));

    let root_stack = gtk::Stack::new();
    root_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    root_stack.add_named(&split_view, Some("shell"));
    root_stack.add_named(&diagnostic_page, Some("diagnostic"));

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&root_stack));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toasts));

    root.set_content(Some(&toolbar));
    root.set_default_size(1120, 760);

    ShellWidgets {
        root_stack,
        toasts,
        content_page,
        page_stack,
        sidebar,
        window_title,
        checks_group,
        audit_group,
        backup_button,
        refresh_button,
        diagnostic_page,
        connections_group,
        connections_stack,
        details_group,
        name_entry,
        engine_combo,
        host_entry,
        port_spin,
        database_entry,
        username_entry,
        password_entry,
        tls_combo,
        readonly_switch,
        url_switch,
        url_entry,
        save_button,
        test_button,
        navigator_stack,
        navigator_combo,
        navigator_databases_list,
        navigator_tables_list,
        editor_dropdown,
        editor_view,
        editor_status,
        editor_error,
        editor_grid,
        run_button,
        cancel_button,
        clear_button,
        explain_button,
        chart_button,
        editor_results_stack,
        chart_area,
        plan_revealer,
        plan_buffer,
        chart_data,
        designer_stack,
        designer_conn_dropdown,
        designer_table_dropdown,
        designer_name_entry,
        designer_columns_box,
        designer_preview_buffer,
        designer_status,
        transfer_stack,
        transfer_conn_dropdown,
        transfer_table_dropdown,
        transfer_format_dropdown,
        transfer_headers_check,
        transfer_path_entry,
        transfer_status,
        history_search_entry,
        history_status,
        history_list,
        saved_list,
        saved_name_entry,
        settings_combo,
        update_button,
        update_status,
        schema_stack,
        schema_conn_dropdown,
        schema_status,
        schema_canvas,
        schema_tables,
        builder_stack,
        builder_conn_dropdown,
        builder_table_dropdown,
        builder_columns_box,
        builder_filters_box,
        builder_filter_column,
        builder_filter_op,
        builder_filter_value,
        builder_order_dropdown,
        builder_order_asc,
        builder_limit_spin,
        builder_preview_buffer,
        builder_status,
        fake_count_spin,
        migrations_stack,
        migrations_conn_dropdown,
        migrations_dir_entry,
        migrations_scan_button,
        migrations_apply_button,
        migrations_status,
        migrations_list,
        migrations_preview_buffer,
        snapshots_conn_dropdown,
        snapshots_table_entry,
        snapshots_note_entry,
        snapshots_create_button,
        snapshots_refresh_button,
        snapshots_status,
        snapshots_list,
        run_multi_button,
        bundle_path_entry,
        bundle_export_button,
        bundle_inspect_button,
        bundle_status,
    }
}

pub fn run(
    handle: tokio::runtime::Handle,
    service: Result<Arc<AppService>, String>,
) -> Result<(), String> {
    let app = RelmApp::new("io.github.sakib669.UniDB");
    app.run::<AppModel>((handle, service));
    Ok(())
}
