//! The first real interface: a studio page, a Security page computed from live
//! store state, and a diagnostic page when startup cannot open the app store.

use std::sync::Arc;

use relm4::gtk::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};

use unidb_app::{AppService, AuditEntry, SecurityReport};

enum Page {
    Studio,
    Security,
}

struct AppModel {
    handle: tokio::runtime::Handle,
    service: Option<Arc<AppService>>,
    error: Option<String>,
    page: Page,
    status: String,
    busy: bool,
    security_text: String,
    audit_text: String,
    security_loaded: bool,
}

enum AppMsg {
    Probe,
    ProbeDone(String),
    ProbeFailed(String),
    ShowStudio,
    ShowSecurity,
    Refresh,
    Backup,
    SecurityLoaded(Box<(SecurityReport, Vec<AuditEntry>)>),
    SecurityFailed(String),
    RetryBootstrap,
    BootstrapOk(Arc<AppService>),
    BootstrapFailed(String),
}

impl std::fmt::Debug for AppMsg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Probe => "Probe",
            Self::ProbeDone(_) => "ProbeDone",
            Self::ProbeFailed(_) => "ProbeFailed",
            Self::ShowStudio => "ShowStudio",
            Self::ShowSecurity => "ShowSecurity",
            Self::Refresh => "Refresh",
            Self::Backup => "Backup",
            Self::SecurityLoaded(_) => "SecurityLoaded",
            Self::SecurityFailed(_) => "SecurityFailed",
            Self::RetryBootstrap => "RetryBootstrap",
            Self::BootstrapOk(_) => "BootstrapOk",
            Self::BootstrapFailed(_) => "BootstrapFailed",
        };
        formatter.write_str(name)
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = (tokio::runtime::Handle, Result<Arc<AppService>, String>);
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("UniDB"),
            set_default_width: 980,
            set_default_height: 680,
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 18,
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    gtk::Button {
                        set_label: "Studio",
                        connect_clicked => AppMsg::ShowStudio,
                    },
                    gtk::Button {
                        set_label: "Security",
                        connect_clicked => AppMsg::ShowSecurity,
                        #[watch]
                        set_sensitive: model.service.is_some(),
                    },
                    gtk::Button {
                        set_label: "Refresh",
                        connect_clicked => AppMsg::Refresh,
                        #[watch]
                        set_sensitive: model.service.is_some() && !model.busy,
                    },
                    gtk::Button {
                        set_label: "Back up now",
                        connect_clicked => AppMsg::Backup,
                        #[watch]
                        set_sensitive: model.service.is_some() && !model.busy,
                    },
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    #[watch]
                    set_visible: model.error.is_some(),
                    gtk::Label {
                        set_label: "UniDB could not start",
                        add_css_class: "title-1",
                        set_xalign: 0.0,
                    },
                    gtk::Label {
                        #[watch]
                        set_label: model.error.as_deref().unwrap_or_default(),
                        set_wrap: true,
                        set_xalign: 0.0,
                        set_selectable: true,
                    },
                    gtk::Label {
                        set_label: "Unlock the system keyring, then retry. UniDB never falls back to plaintext keys.",
                        set_wrap: true,
                        set_xalign: 0.0,
                        add_css_class: "dim-label",
                    },
                    gtk::Button {
                        set_label: "Retry",
                        connect_clicked => AppMsg::RetryBootstrap,
                        #[watch]
                        set_sensitive: !model.busy,
                    },
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    #[watch]
                    set_visible: model.error.is_none() && model.service.is_some(),
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        #[watch]
                        set_visible: matches!(model.page, Page::Studio),
                        gtk::Label {
                            set_label: "Studio",
                            add_css_class: "title-1",
                            set_xalign: 0.0,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.status,
                            set_xalign: 0.0,
                            set_wrap: true,
                        },
                        gtk::Button {
                            set_label: "Run SQLite probe",
                            connect_clicked => AppMsg::Probe,
                            #[watch]
                            set_sensitive: !model.busy,
                        },
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,
                        #[watch]
                        set_visible: matches!(model.page, Page::Security),
                        gtk::Label {
                            set_label: "Security",
                            add_css_class: "title-1",
                            set_xalign: 0.0,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.security_text,
                            set_xalign: 0.0,
                            set_wrap: true,
                            set_selectable: true,
                        },
                        gtk::Label {
                            set_label: "Recent audit entries",
                            add_css_class: "heading",
                            set_xalign: 0.0,
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.audit_text,
                            set_xalign: 0.0,
                            set_wrap: true,
                            set_selectable: true,
                        },
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (handle, startup) = init;
        let (service, error) = match startup {
            Ok(service) => (Some(service), None),
            Err(message) => (None, Some(message)),
        };
        let mut model = AppModel {
            handle,
            service,
            error,
            page: Page::Studio,
            status: format!(
                "Ready. Version {}. Choose Security to see the store state.",
                env!("CARGO_PKG_VERSION")
            ),
            busy: false,
            security_text: "Loading the security report...".to_string(),
            audit_text: String::new(),
            security_loaded: false,
        };
        let widgets = view_output!();
        if model.service.is_some() {
            model.load_security(&sender);
        }
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::ShowStudio => {
                self.page = Page::Studio;
            }
            AppMsg::ShowSecurity => {
                self.page = Page::Security;
                if !self.security_loaded {
                    self.load_security(&sender);
                }
            }
            AppMsg::Refresh => {
                self.load_security(&sender);
            }
            AppMsg::Backup => {
                if let Some(service) = &self.service {
                    self.busy = true;
                    let service = Arc::clone(service);
                    let sender = sender.clone();
                    self.handle.spawn(async move {
                        match service.backup_now().await {
                            Ok(path) => sender.input(AppMsg::ProbeDone(format!(
                                "Backup written to {}.",
                                path.display()
                            ))),
                            Err(error) => sender.input(AppMsg::ProbeFailed(format!(
                                "Backup failed: {} ({})",
                                error.user_message(),
                                error.code()
                            ))),
                        }
                    });
                }
            }
            AppMsg::Probe => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.status = "Running the SQLite probe...".to_string();
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
                                sender.input(AppMsg::ProbeFailed(format!("Probe failed: {error}")));
                            }
                        }
                    });
                } else {
                    self.busy = false;
                }
            }
            AppMsg::ProbeDone(message) | AppMsg::ProbeFailed(message) => {
                self.busy = false;
                self.status = message;
            }
            AppMsg::SecurityLoaded(payload) => {
                let (report, audit) = *payload;
                self.security_text = format_security(&report);
                self.audit_text = format_audit(&audit);
                self.security_loaded = true;
                self.busy = false;
            }
            AppMsg::SecurityFailed(message) => {
                self.security_text = format!("Could not build the security report: {message}");
                self.security_loaded = false;
                self.busy = false;
            }
            AppMsg::RetryBootstrap => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.error = Some("Retrying startup...".to_string());
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
                self.status = format!(
                    "Ready. Version {}. Choose Security to see the store state.",
                    env!("CARGO_PKG_VERSION")
                );
                self.load_security(&sender);
            }
            AppMsg::BootstrapFailed(message) => {
                self.busy = false;
                self.error = Some(message);
            }
        }
    }
}

impl AppModel {
    fn load_security(&mut self, sender: &ComponentSender<Self>) {
        let Some(service) = &self.service else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        let service = Arc::clone(service);
        let sender = sender.clone();
        self.handle.spawn(async move {
            match service.security_report().await {
                Ok(report) => {
                    let audit = service.recent_audit(10).await.unwrap_or_default();
                    sender.input(AppMsg::SecurityLoaded(Box::new((report, audit))));
                }
                Err(error) => {
                    sender.input(AppMsg::SecurityFailed(format!(
                        "{} ({})",
                        error.user_message(),
                        error.code()
                    )));
                }
            }
        });
    }
}

fn format_security(report: &SecurityReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{} connections, {} encrypted secrets, {} audit entries",
        report.connection_count, report.secret_count, report.audit_count
    ));
    for check in &report.checks {
        lines.push(format!(
            "[{}] {}: {}",
            check.status.label(),
            check.label,
            check.detail
        ));
    }
    lines.join("\n")
}

fn format_audit(entries: &[AuditEntry]) -> String {
    if entries.is_empty() {
        return "No audit entries yet.".to_string();
    }
    entries
        .iter()
        .map(|entry| format!("{}  {}  {}", entry.ts, entry.action, entry.outcome))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn run(
    handle: tokio::runtime::Handle,
    service: Result<Arc<AppService>, String>,
) -> Result<(), String> {
    let app = RelmApp::new("io.github.sakib669.UniDB");
    app.run::<AppModel>((handle, service));
    Ok(())
}
