---
name: developing-gtk-apps
description: Build GNOME apps with GTK 4, libadwaita, GNOME Builder, and Flatpak. Use when starting new projects, setting up dev environment, choosing languages, or packaging apps. Covers platform stack, language bindings, project structure. Don't use for UI design decisions or HIG compliance.
license: CC-BY-SA-4.0
compatibility: GTK 4.x, libadwaita 1.4+, GNOME Builder, Flatpak
metadata:
  author: Paresh-Maheshwari
  version: "1.0.0"
  category: development
  last-updated: "2026-10-02"
  source: https://github.com/Paresh-Maheshwari/gnome-skills
---

# Developing GTK Apps

Build GNOME apps using GTK 4, libadwaita, and the GNOME platform with proper tooling and distribution.

## When to Use

- Starting a new GNOME application project
- Setting up development environment for GTK
- Choosing programming languages and tools
- Packaging apps with Flatpak for distribution
- Using GNOME Builder IDE

## Reference Files

Load as needed:
- [gnome-platform-libraries.md](gnome-platform-libraries.md) — When looking up specific GNOME libraries and their APIs

## Platform Stack

| Layer | Component | Purpose |
|-------|-----------|---------|
| **UI Toolkit** | GTK 4 | Widgets, rendering, input |
| **Design Library** | libadwaita | GNOME patterns, styling |
| **Core Libraries** | GLib, GIO, GObject | Data structures, I/O, type system |
| **IPC** | D-Bus (GDBus) | Inter-process communication |
| **Distribution** | Flatpak | Packaging and sandboxing |
| **IDE** | GNOME Builder | Development environment |

## Language Bindings

| Language | Project | Best For |
|----------|---------|----------|
| **C** | Native | Libraries, maximum control |
| **Python** | PyGObject | Rapid development, scripting |
| **Rust** | gtk-rs | Memory safety, performance |
| **JavaScript** | GJS | GNOME Shell extensions, prototyping |
| **Vala** | Vala | GObject-native syntax |
| **C++** | gtkmm | C++ integration |
| **C#** | gir.core | .NET ecosystem |

**Recommendation:** Use C for libraries (maximum binding support). Use any language for apps based on team expertise.

## GNOME Builder Setup

Install Builder from Flathub:
```bash
flatpak install flathub org.gnome.Builder
```

### Create New Project
1. Launch Builder → "Start New Project"
2. Enter project name and Application ID (e.g., `org.example.MyApp`)
3. Select language template
4. Click "Create Project"

Builder provides:
- Syntax highlighting and autocompletion
- Integrated Flatpak build/run
- Git integration
- Profiler and debugger
- Live previews for markup files

## Application ID

Every GNOME app needs a unique reverse-DNS application ID:
```
org.gnome.Calculator
com.example.MyApp
io.github.username.ProjectName
```

The ID is used for:
- D-Bus service name
- Desktop file name
- GSettings schema path
- Flatpak app ID

## Flatpak Distribution

### Manifest Structure

```json
{
    "app-id": "org.example.MyApp",
    "runtime": "org.gnome.Platform",
    "runtime-version": "46",
    "sdk": "org.gnome.Sdk",
    "command": "my-app",
    "finish-args": [
        "--share=ipc",
        "--socket=fallback-x11",
        "--socket=wayland",
        "--device=dri"
    ],
    "modules": [
        {
            "name": "my-app",
            "buildsystem": "meson",
            "sources": [
                {
                    "type": "git",
                    "url": "https://github.com/user/my-app.git"
                }
            ]
        }
    ]
}
```

## Flatpak Distribution

### Key Concepts

| Concept | Description |
|---------|-------------|
| **Runtime** | Collection of dependencies as a platform (e.g., `org.gnome.Platform`) |
| **SDK** | Development version of runtime with headers/tools |
| **Manifest** | JSON/YAML file defining build and permissions |
| **Sandbox** | Isolated environment with explicit permissions |

### Features

- Cross-distro app distribution
- Forward-compatibility across distro versions
- Control over dependencies
- Predictable development environments
- Easy testing with Builder
- Distribution via Flathub

### Manifest Example

```json
{
    "app-id": "org.gnome.Hello",
    "runtime": "org.gnome.Platform",
    "runtime-version": "46",
    "sdk": "org.gnome.Sdk",
    "command": "hello",
    "finish-args": [
        "--share=network",
        "--share=ipc",
        "--socket=fallback-x11",
        "--socket=wayland"
    ],
    "cleanup": [
        "/include",
        "/lib/pkgconfig",
        "*.la",
        "*.a"
    ],
    "modules": [
        {
            "name": "hello",
            "buildsystem": "meson",
            "sources": [
                {
                    "type": "git",
                    "url": "file:///home/user/Projects/hello"
                }
            ]
        }
    ]
}
```

### Common finish-args

```json
"--share=network",         // Network access
"--share=ipc",             // X11 shared memory
"--socket=wayland",        // Wayland display
"--socket=fallback-x11",   // X11 fallback
"--socket=pulseaudio",     // Audio
"--device=dri",            // GPU acceleration
"--filesystem=home",       // Home directory (avoid if possible)
"--talk-name=org.freedesktop.secrets"  // D-Bus service access
```

### Build Commands

```bash
# Build and run in Builder: just press Play button

# Manual build
flatpak-builder --user --install --force-clean build-dir org.example.MyApp.json

# Run
flatpak run org.example.MyApp
```

### Publish to Flathub

1. Fork `github.com/flathub/flathub`
2. Create manifest at `org.example.MyApp.json`
3. Submit PR to Flathub
4. Wait for review and merge

## Project Structure

```
my-app/
├── meson.build                 # Build system
├── org.example.MyApp.json      # Flatpak manifest
├── src/
│   ├── main.c (or .py/.rs)     # Entry point
│   ├── application.c           # GtkApplication subclass
│   ├── window.c                # Main window
│   └── meson.build
├── data/
│   ├── org.example.MyApp.desktop.in     # Desktop entry
│   ├── org.example.MyApp.metainfo.xml   # AppStream metadata
│   ├── org.example.MyApp.gschema.xml    # Settings schema
│   └── icons/
│       └── hicolor/
│           └── scalable/
│               └── apps/
│                   └── org.example.MyApp.svg
└── po/                         # Translations
    ├── LINGUAS
    └── POTFILES
```

## Minimal Python Example

```python
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio

class MyApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id='org.example.MyApp',
                         flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
        
    def do_activate(self):
        win = Adw.ApplicationWindow(application=self)
        win.set_default_size(800, 600)
        
        header = Adw.HeaderBar()
        content = Adw.StatusPage(title="Hello, GNOME!",
                                  icon_name="face-smile-symbolic")
        
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(header)
        box.append(content)
        
        win.set_content(box)
        win.present()

if __name__ == '__main__':
    app = MyApp()
    app.run(None)
```

## Meson Build (Python)

```meson
project('my-app',
  version: '0.1.0',
  meson_version: '>= 0.59.0',
)

python = import('python')
py_installation = python.find_installation('python3')

# Install Python sources
install_data(
  'src/main.py',
  install_dir: get_option('bindir'),
  install_mode: 'rwxr-xr-x',
  rename: 'my-app'
)

# Install desktop file
desktop_file = i18n.merge_file(
  input: 'data/org.example.MyApp.desktop.in',
  output: 'org.example.MyApp.desktop',
  type: 'desktop',
  po_dir: 'po',
  install: true,
  install_dir: get_option('datadir') / 'applications'
)

# Install GSettings schema
install_data(
  'data/org.example.MyApp.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0/schemas'
)
```

## Quick Reference

### GtkApplication Lifecycle

1. `startup` signal — one-time initialization
2. `activate` signal — show main window
3. `open` signal — open files (if supporting file arguments)
4. `shutdown` signal — cleanup

### Essential Imports (Python)

```python
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, Gio, GLib, GObject
```

### Essential Includes (C)

```c
#include <gtk/gtk.h>
#include <adwaita.h>
```

## Red Flags

- Missing Application ID or using invalid format
- Requesting unnecessary Flatpak permissions
- Not using libadwaita for GNOME apps
- Hardcoding paths instead of using XDG directories
- Missing desktop file or metainfo
- No icon at proper sizes

## Reference Files

| Topic | File |
|-------|------|
| Platform libraries | [gnome-platform-libraries.md](gnome-platform-libraries.md) |
| Flatpak details | [flatpak-distribution.md](flatpak-distribution.md) |

## External Resources

- [GTK 4 Docs](https://docs.gtk.org/gtk4/)
- [Libadwaita Docs](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/)
- [Flatpak Docs](https://docs.flatpak.org/)
- [GNOME Builder Docs](https://builder.readthedocs.io/)
- [PyGObject Docs](https://pygobject.gnome.org/)
- [gtk-rs Book](https://gtk-rs.org/gtk4-rs/stable/latest/book/)

---

## Blueprint UI Language

Blueprint is a modern declarative markup language for GTK 4 UIs, cleaner than XML.

### Comparison

**XML (GtkBuilder):**
```xml
<object class="GtkBox">
  <property name="orientation">vertical</property>
  <child>
    <object class="GtkLabel">
      <property name="label">Hello World</property>
    </object>
  </child>
</object>
```

**Blueprint:**
```blueprint
Gtk.Box {
  orientation: vertical;
  
  Gtk.Label {
    label: "Hello World";
  }
}
```

### Installing Blueprint Compiler

```bash
# Fedora
sudo dnf install blueprint-compiler

# Flatpak SDK extension
flatpak install org.gnome.Sdk.Extension.blueprint-compiler
```

### Blueprint Syntax

```blueprint
using Gtk 4.0;
using Adw 1;

template $MyWindow: Adw.ApplicationWindow {
  default-width: 800;
  default-height: 600;
  title: "My App";
  
  content: Adw.ToolbarView {
    [top]
    Adw.HeaderBar {}
    
    content: Adw.StatusPage {
      icon-name: "face-smile-symbolic";
      title: "Hello, GNOME!";
      description: "Welcome to my app";
      
      child: Gtk.Button {
        label: "Click Me";
        clicked => $on_button_clicked();
        
        styles ["suggested-action", "pill"]
      };
    };
  };
}
```

### Meson Integration

```meson
blueprints = custom_target('blueprints',
  input: files('window.blp'),
  output: '.',
  command: [
    find_program('blueprint-compiler'),
    'batch-compile',
    '@OUTPUT@',
    '@CURRENT_SOURCE_DIR@',
    '@INPUT@'
  ]
)

gnome.compile_resources('my-app-resources',
  'my-app.gresource.xml',
  dependencies: blueprints,
  source_dir: meson.current_build_dir()
)
```

### Loading Blueprint Templates (Python)

```python
@Gtk.Template(resource_path='/org/example/MyApp/window.ui')
class MyWindow(Adw.ApplicationWindow):
    __gtype_name__ = 'MyWindow'
    
    # Bind child widgets
    button = Gtk.Template.Child()
    
    @Gtk.Template.Callback()
    def on_button_clicked(self, button):
        print("Button clicked!")
```

---

## GSettings (App Configuration)

GSettings provides persistent app settings with automatic change notification.

### Schema File

`data/org.example.MyApp.gschema.xml`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<schemalist>
  <schema id="org.example.MyApp" path="/org/example/MyApp/">
    
    <key name="window-width" type="i">
      <default>800</default>
      <summary>Window width</summary>
    </key>
    
    <key name="window-height" type="i">
      <default>600</default>
      <summary>Window height</summary>
    </key>
    
    <key name="dark-mode" type="b">
      <default>false</default>
      <summary>Dark mode preference</summary>
    </key>
    
    <key name="recent-files" type="as">
      <default>[]</default>
      <summary>Recently opened files</summary>
    </key>
    
  </schema>
</schemalist>
```

### Schema Installation (Meson)

```meson
install_data(
  'data/org.example.MyApp.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0/schemas'
)

# Compile schemas after install
meson.add_install_script('glib-compile-schemas',
  get_option('prefix') / get_option('datadir') / 'glib-2.0/schemas')
```

### Using GSettings (Python)

```python
from gi.repository import Gio

settings = Gio.Settings.new('org.example.MyApp')

# Read values
width = settings.get_int('window-width')
dark = settings.get_boolean('dark-mode')
files = settings.get_strv('recent-files')

# Write values
settings.set_int('window-width', 1024)
settings.set_boolean('dark-mode', True)
settings.set_strv('recent-files', ['/path/to/file.txt'])

# React to changes
settings.connect('changed::dark-mode', on_dark_mode_changed)

# Bind to widget property
settings.bind('dark-mode', switch_widget, 'active',
              Gio.SettingsBindFlags.DEFAULT)
```

### Using GSettings (C)

```c
GSettings *settings = g_settings_new("org.example.MyApp");

// Read
int width = g_settings_get_int(settings, "window-width");
gboolean dark = g_settings_get_boolean(settings, "dark-mode");

// Write
g_settings_set_int(settings, "window-width", 1024);

// Bind to property
g_settings_bind(settings, "dark-mode",
                switch_widget, "active",
                G_SETTINGS_BIND_DEFAULT);
```

### Testing Schemas (Without Installing)

```bash
# Compile schema locally
glib-compile-schemas data/

# Run with local schema
GSETTINGS_SCHEMA_DIR=data/ ./my-app
```

---

## XDG Desktop Portals

Portals let sandboxed Flatpak apps access system features securely.

### Why Portals?

Flatpak apps run in a sandbox and can't directly:
- Open files from anywhere
- Access the camera/microphone
- Show notifications
- Take screenshots
- Print documents

Portals provide secure, user-consented access to these features.

### GTK Automatic Portal Usage

GTK 4 automatically uses portals when sandboxed:
- `GtkFileDialog` → File chooser portal
- `GtkPrintDialog` → Print portal
- `GtkUriLauncher` → Open URI portal

**Just use GTK's high-level APIs and portals work automatically.**

### Using libportal (Direct Portal Access)

For features GTK doesn't wrap, use libportal:

```python
from gi.repository import Xdp

portal = Xdp.Portal.new()

# Take screenshot
portal.take_screenshot(
    parent=None,
    flags=Xdp.ScreenshotFlags.NONE,
    cancellable=None,
    callback=on_screenshot_done
)

# Open URI
portal.open_uri(
    parent=window,
    uri="https://gnome.org",
    flags=Xdp.OpenUriFlags.NONE,
    cancellable=None,
    callback=None
)

# Request background permission
portal.request_background(
    parent=window,
    reason="Run sync in background",
    commandline=["my-app", "--background"],
    flags=Xdp.BackgroundFlags.AUTOSTART,
    cancellable=None,
    callback=on_background_result
)
```

### Common Portals

| Portal | Purpose | GTK Widget |
|--------|---------|------------|
| FileChooser | Open/save files | `GtkFileDialog` |
| Print | Printing | `GtkPrintDialog` |
| OpenURI | Open URLs/files | `GtkUriLauncher` |
| Notification | System notifications | `GNotification` |
| Screenshot | Take screenshots | libportal |
| Camera | Camera access | libportal |
| Location | GPS location | libportal |
| Background | Run in background | libportal |
| Secret | Store passwords | libsecret |

### Flatpak Permissions for Portals

Portals don't need Flatpak permissions — that's the point. But some portals require:

```json
"finish-args": [
    "--talk-name=org.freedesktop.portal.Desktop"
]
```

This is included by default in GNOME runtime.

### Notification Example

```python
from gi.repository import Gio

notification = Gio.Notification.new("Download Complete")
notification.set_body("Your file has finished downloading")
notification.set_icon(Gio.ThemedIcon.new("folder-download-symbolic"))
notification.add_button("Open", "app.open-download")

app.send_notification("download-complete", notification)
```

---

## Development Tips

### Hot Reload with Builder
- Edit `.blp` or `.ui` files
- Builder auto-reloads preview
- No need to restart app for UI changes

### Testing Different Locales
```bash
LANGUAGE=de_DE.UTF-8 flatpak run org.example.MyApp
```

### Debugging Flatpak Apps
```bash
flatpak run --command=sh org.example.MyApp
# Inside sandbox
G_MESSAGES_DEBUG=all /app/bin/my-app
```

### Check Portal Availability
```bash
# List available portals
busctl --user list | grep portal
```
