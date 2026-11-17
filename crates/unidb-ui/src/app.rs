//! The application shell: a libadwaita window with a sidebar, a Connections
//! workflow, a Studio empty state, a Security page from live store state, and a
//! startup diagnostic page. The dynamic parts are built in Rust.

use std::collections::HashSet;
use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};
use uuid::Uuid;

use unidb_app::{AppService, AuditEntry, CheckStatus, SecurityReport};
use unidb_core::{ConnectionProfile, Engine, SslMode};

const ABOUT_NAME: &str = "UniDB";

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
                if index == 3 && !self.security_loaded {
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
            2 => ("studio", "Studio"),
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
    page_stack.add_named(&studio_page, Some("studio"));
    page_stack.add_named(&security_scroll, Some("security"));

    let content_page = adw::NavigationPage::new(&page_stack, "Connections");
    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.add_css_class("navigation-sidebar");
    for (icon, label) in [
        ("network-server-symbolic", "Connections"),
        ("view-list-symbolic", "Navigator"),
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
