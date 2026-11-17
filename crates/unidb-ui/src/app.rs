//! The application shell: a libadwaita window with a sidebar, a Studio empty
//! state, a Security page built from live store state, and a startup diagnostic
//! page. The shell is built programmatically because most of it is dynamic.

use std::sync::Arc;

use relm4::adw::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};

use unidb_app::{AppService, AuditEntry, CheckStatus, SecurityReport};

const ABOUT_NAME: &str = "UniDB";

struct AppModel {
    handle: tokio::runtime::Handle,
    service: Option<Arc<AppService>>,
    error: Option<String>,
    section: usize,
    busy: bool,
    security_loaded: bool,
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
        };

        if let Some(error) = &model.error {
            model.show_diagnostic(error);
        } else {
            model.root_stack.set_visible_child_name("shell");
            model.select_section(0);
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
                            Err(error) => {
                                sender.input(AppMsg::ProbeFailed(format!(
                                    "Probe failed: {}",
                                    error.user_message()
                                )));
                            }
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
                if index == 1 && !self.security_loaded {
                    self.load_security(&sender);
                }
            }
            AppMsg::Refresh => {
                self.load_security(&sender);
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
                self.busy = false;
                self.update_controls();
            }
            AppMsg::SecurityFailed(message) => {
                self.busy = false;
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
            AppMsg::Quit => {
                relm4::main_application().quit();
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
            0 => ("studio", "Studio"),
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
    }

    fn load_security(&mut self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.update_controls();
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

fn build_shell(root: &adw::ApplicationWindow, sender: &ComponentSender<AppModel>) -> ShellWidgets {
    let window_title = adw::WindowTitle::new(ABOUT_NAME, "Studio");
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
            "Run a local SQLite probe to prove the worker bridge. The connection manager and query editor arrive in the next features.",
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
    page_stack.add_named(&studio_page, Some("studio"));
    page_stack.add_named(&security_scroll, Some("security"));

    let content_page = adw::NavigationPage::new(&page_stack, "Studio");
    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.add_css_class("navigation-sidebar");
    for (icon, label) in [
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
