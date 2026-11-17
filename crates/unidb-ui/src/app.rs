//! The minimal application shell from the scaffold: a libadwaita window that
//! proves the native toolkit and the app identity are wired before the design
//! system feature restyles and fills it.

use relm4::prelude::*;
use relm4::{adw, gtk};

struct AppModel {
    version: String,
    engine_status: String,
}

#[derive(Debug)]
enum AppMsg {}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
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
                    set_label: &model.version,
                },
                gtk::Label {
                    #[watch]
                    set_label: &model.engine_status,
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let engines: Vec<&str> = unidb_core::Engine::ALL
            .iter()
            .map(|engine| engine.display())
            .collect();
        let model = AppModel {
            version: format!("Version {}", env!("CARGO_PKG_VERSION")),
            engine_status: format!(
                "Engines: {} available, SQLite driver ready",
                engines.join(", ")
            ),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {}
    }
}

pub fn run() -> Result<(), String> {
    let app = RelmApp::new("io.github.sakib669.UniDB");
    app.run::<AppModel>(());
    Ok(())
}
