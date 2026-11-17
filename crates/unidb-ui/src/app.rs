//! The application shell from the scaffold: a libadwaita window that reaches a
//! tokio worker through the app service and shows the result, proving the
//! interface to worker bridge end to end.

use std::sync::Arc;

use relm4::gtk::prelude::*;
use relm4::prelude::*;
use relm4::{adw, gtk};

use unidb_app::AppService;

struct AppModel {
    service: Arc<AppService>,
    status: String,
    busy: bool,
}

#[derive(Debug)]
enum AppMsg {
    Probe,
    ProbeDone(String),
    ProbeFailed(String),
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = Arc<AppService>;
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("UniDB"),
            set_default_width: 900,
            set_default_height: 600,
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_margin_all: 24,
                gtk::Label {
                    set_label: "UniDB",
                    add_css_class: "title-1",
                },
                gtk::Label {
                    set_label: "Native database studio",
                    add_css_class: "dim-label",
                },
                gtk::Label {
                    #[watch]
                    set_label: &model.status,
                },
                gtk::Button {
                    set_label: "Run SQLite probe",
                    #[watch]
                    set_sensitive: !model.busy,
                    connect_clicked => AppMsg::Probe,
                },
            }
        }
    }

    fn init(
        service: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel {
            service,
            status: format!("Ready. Version {}.", env!("CARGO_PKG_VERSION")),
            busy: false,
        };
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
                self.status = "Running the SQLite probe...".to_string();
                let service = Arc::clone(&self.service);
                let bridge = service.bridge().clone();
                let sender = sender.clone();
                bridge.spawn(async move {
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
            }
            AppMsg::ProbeDone(message) | AppMsg::ProbeFailed(message) => {
                self.busy = false;
                self.status = message;
            }
        }
    }
}

pub fn run(service: Arc<AppService>) -> Result<(), String> {
    let app = RelmApp::new("io.github.sakib669.UniDB");
    app.run::<AppModel>(service);
    Ok(())
}
