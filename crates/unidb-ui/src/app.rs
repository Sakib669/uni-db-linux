//! The application shell: a libadwaita window with a sidebar, a Connections
//! workflow, a Studio empty state, a Security page from live store state, and a
//! startup diagnostic page. The dynamic parts are built in Rust.

use std::collections::HashSet;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};
use sourceview5::prelude::*;
use uuid::Uuid;

use unidb_app::{AppService, AuditEntry, CheckStatus, ImportOptions, ImportReport, SecurityReport};
use unidb_core::{
    ColumnDesign, ConnectionProfile, Engine, SqlDialect, SslMode, TableColumnInfo, TableDesign,
    TransferFormat, alter_table_sql, create_table_sql,
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
    status_label: gtk::Label,
    probe_button: gtk::Button,
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
}

enum AppMsg {
    Probe,
    ProbeDone(String),
    ProbeFailed(String),
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
}

impl std::fmt::Debug for AppMsg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Probe => "Probe",
            Self::ProbeDone(_) => "ProbeDone",
            Self::ProbeFailed(_) => "ProbeFailed",
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
    status_label: gtk::Label,
    probe_button: gtk::Button,
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
            status_label: widgets.status_label,
            probe_button: widgets.probe_button,
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
        };

        if let Some(error) = &model.error {
            model.show_diagnostic(error);
        } else {
            model.root_stack.set_visible_child_name("shell");
            model.select_section(0);
            model.load_connections(&sender);
            model.load_security(&sender);
        }
        model.update_controls();

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Probe => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.update_controls();
                self.status_label.set_label("Running the SQLite probe...");
                if let Some(service) = &self.service {
                    let service = Arc::clone(service);
                    let sender = sender.clone();
                    self.handle.spawn(async move {
                        match service.probe_sqlite().await {
                            Ok(result) => {
                                let version = result
                                    .rows
                                    .first()
                                    .and_then(|row| row.first())
                                    .map(unidb_app::value_text)
                                    .unwrap_or_default();
                                sender.input(AppMsg::ProbeDone(format!(
                                    "SQLite {version} answered through the worker bridge in {} ms.",
                                    result.elapsed_ms
                                )));
                            }
                            Err(error) => sender.input(AppMsg::ProbeFailed(format!(
                                "Probe failed: {}",
                                error.user_message()
                            ))),
                        }
                    });
                }
            }
            AppMsg::ProbeDone(message) | AppMsg::ProbeFailed(message) => {
                self.busy = false;
                self.status_label.set_label(&message);
                self.update_controls();
            }
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
                if index == 6 && !self.security_loaded {
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
                self.update_controls();
                self.render_result(&result);
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
                self.editor_status.set_label("Ready.");
                self.editor_error.set_visible(false);
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
            5 => ("studio", "Studio"),
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
        self.probe_button.set_sensitive(ready && !self.busy);
        self.backup_button.set_sensitive(ready && !self.busy);
        self.refresh_button.set_sensitive(ready && !self.busy);
        self.save_button.set_sensitive(ready && !self.busy);
        self.test_button.set_sensitive(ready && !self.busy);
        self.run_button.set_sensitive(ready && !self.editor_busy);
        self.cancel_button.set_sensitive(ready && self.editor_busy);
        self.clear_button.set_sensitive(true);
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
    let connections_page = adw::PreferencesPage::new();
    connections_page.add(&connections_group);
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
    let results_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    results_box.set_margin_top(8);
    results_box.set_margin_bottom(8);
    results_box.set_margin_start(12);
    results_box.set_margin_end(12);
    results_box.append(&editor_status);
    results_box.append(&editor_error);
    results_box.append(&editor_grid_scroll);
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
    let export_csv_button = gtk::Button::with_label("Export CSV");
    {
        let sender = sender.clone();
        export_csv_button
            .connect_clicked(move |_| sender.input(AppMsg::ExportQuery(TransferFormat::Csv)));
    }
    let export_json_button = gtk::Button::with_label("Export JSON");
    {
        let sender = sender.clone();
        export_json_button
            .connect_clicked(move |_| sender.input(AppMsg::ExportQuery(TransferFormat::Json)));
    }
    let editor_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    editor_toolbar.set_margin_all(10);
    editor_toolbar.append(&editor_dropdown);
    editor_toolbar.append(&run_button);
    editor_toolbar.append(&cancel_button);
    editor_toolbar.append(&clear_button);
    editor_toolbar.append(&export_csv_button);
    editor_toolbar.append(&export_json_button);
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

    let status_label = gtk::Label::builder()
        .label("Ready.")
        .xalign(0.0)
        .wrap(true)
        .build();
    let probe_button = gtk::Button::with_label("Run SQLite probe");
    probe_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        probe_button.connect_clicked(move |_| sender.input(AppMsg::Probe));
    }
    let studio_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    studio_box.append(&status_label);
    studio_box.append(&probe_button);
    let studio_page = adw::StatusPage::builder()
        .icon_name("utilities-terminal-symbolic")
        .title("Studio ready")
        .description(
            "The query editor arrives in the next features. Use Connections to add a database first.",
        )
        .child(&studio_box)
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

    let page_stack = gtk::Stack::new();
    page_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    page_stack.add_named(&connections_stack, Some("connections"));
    page_stack.add_named(&navigator_stack, Some("navigator"));
    page_stack.add_named(&editor_page, Some("editor"));
    page_stack.add_named(&designer_stack, Some("designer"));
    page_stack.add_named(&transfer_stack, Some("transfer"));
    page_stack.add_named(&studio_page, Some("studio"));
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
        ("utilities-terminal-symbolic", "Studio"),
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
        status_label,
        probe_button,
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
