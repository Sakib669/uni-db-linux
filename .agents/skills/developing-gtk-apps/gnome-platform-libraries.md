---
name: gnome-platform-libraries
description: GNOME platform libraries and system services including GLib, GIO, libsoup, GStreamer, libsecret, portals, NetworkManager. Use when finding the right library for a task or understanding platform dependencies. Don't use for UI patterns.
license: CC-BY-SA-4.0
metadata:
  author: Paresh-Maheshwari
  source: https://github.com/Paresh-Maheshwari/gnome-skills
---

# GNOME Platform Libraries and Services Reference

Complete reference of GNOME platform libraries and system services.

## User Interface Libraries

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GTK** | UI toolkit with widgets, rendering, input | [docs.gtk.org/gtk4](https://docs.gtk.org/gtk4/) |
| **libadwaita** | GNOME design patterns, styling, widgets | [libadwaita docs](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/) |
| **WebKitGTK** | Web functionality, HTML rendering, embedded web views | [WebKitGTK reference](https://webkitgtk.org/reference/webkit2gtk/stable/index.html) |

## Core Libraries

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GLib** | Data structures, memory, strings, main loop | [docs.gtk.org/glib](https://docs.gtk.org/glib/) |
| **GIO** | I/O, GFile, GSettings, GDBus, networking | [docs.gtk.org/gio](https://docs.gtk.org/gio/) |
| **GObject** | Object system, properties, signals | [docs.gtk.org/gobject](https://docs.gtk.org/gobject/) |

## Fonts & Rendering

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **Fontconfig** | Font discovery and matching | [freedesktop.org](https://www.freedesktop.org/wiki/Software/fontconfig/) |
| **FreeType** | Font rendering | [freetype.org](https://www.freetype.org/) |
| **HarfBuzz** | Text shaping | [harfbuzz.github.io](https://harfbuzz.github.io/) |
| **Pango** | Text layout, internationalization, Unicode | [docs.gtk.org/Pango](https://docs.gtk.org/Pango/) |

## Image Loading

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **gdk-pixbuf** | Image loading, pixel buffer operations | [docs.gtk.org/gdk-pixbuf](https://docs.gtk.org/gdk-pixbuf/) |
| **librsvg** | SVG rendering | [librsvg docs](https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/index.html) |
| **glycin** | Sandboxed image decoding to GdkTextures | [glycin docs](https://docs.rs/glycin/latest/glycin/) |

## Graphics

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **Cairo** | 2D vector graphics, PDF/SVG output | [cairo manual](https://cairographics.org/manual/) |

## File System

| API | Purpose | Documentation |
|-----|---------|---------------|
| **GFile** | Virtual filesystem abstraction, file operations, metadata | [GFile docs](https://docs.gtk.org/gio/iface.File.html) |

## IPC & Communication

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GDBus** | D-Bus protocol implementation, high-level API | [GDBus docs](https://docs.gtk.org/gio/class.DBusConnection.html) |

## Multimedia

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GStreamer** | Audio/video playback, multimedia processing | [GStreamer docs](https://gstreamer.freedesktop.org/documentation/) |
| GTK 4 Video | Simple video playback (built into GTK 4) | [GtkVideo](https://docs.gtk.org/gtk4/class.Video.html) |

## Settings

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GSettings** | Application settings storage | [GSettings docs](https://docs.gtk.org/gio/class.Settings.html) |

## Data Storage

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GMarkup** | Simple XML parsing | [GMarkup docs](https://docs.gtk.org/glib/struct.MarkupParseContext.html) |
| **libxml2** | Full XML parser/generator | [libxml2 manual](http://www.xmlsoft.org/html/index.html) |
| **GKeyFile** | INI-style key-value files | [GKeyFile docs](https://docs.gtk.org/glib/struct.KeyFile.html) |
| **JSON-GLib** | JSON parser and generator | [JSON-GLib docs](https://gnome.pages.gitlab.gnome.org/json-glib/) |
| **GDA** | Multi-database access (SQLite, MySQL, Postgres) | [GDA project](https://gitlab.gnome.org/GNOME/libgda) |
| **GOM** | GObject to SQLite mapper | [GOM project](https://gitlab.gnome.org/GNOME/gom) |
| **TinySPARQL** | RDF triple store with SPARQL | [TinySPARQL docs](https://gnome.pages.gitlab.gnome.org/tinysparql/) |

## Networking

| Library | Purpose | Documentation |
|---------|---------|---------------|
| **GSocket** | Low-level networking, connections, services | [GSocket docs](https://docs.gtk.org/gio/class.Socket.html) |
| **GIO TLS** | TLS connections, DNS resolution | [TLS overview](https://docs.gtk.org/gio/tls-overview.html) |
| **Avahi** | Zeroconf/mDNS service discovery | [avahi.org](http://avahi.org/) |
| **libsoup** | HTTP client/server, async operations | [libsoup docs](https://libsoup.gnome.org/libsoup-3.0/index.html) |

## System Services

### Mail & Calendar

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **Evolution Data Server** | Shared address book, calendar | [ECal API](https://gnome.pages.gitlab.gnome.org/evolution-data-server/libecal/) |

### Contacts

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **Folks** | Contact aggregation across accounts | [Folks API](https://telepathy.freedesktop.org/doc/folks/c/) |

### File Indexing

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **LocalSearch** | Filesystem indexing and search | [LocalSearch docs](https://gnome.pages.gitlab.gnome.org/localsearch/) |

### Authorization

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **Polkit** | Privilege escalation authorization | [Polkit docs](https://www.freedesktop.org/software/polkit/docs/latest/) |

### Secrets

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **libsecret** | Password and secrets storage | [libsecret docs](https://gnome.pages.gitlab.gnome.org/libsecret/) |

### Network Management

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **NetworkManager** | Network configuration and connection management | [libnm docs](https://developer-old.gnome.org/libnm/stable/) |
| **ModemManager** | Mobile broadband device control | [ModemManager](https://www.freedesktop.org/wiki/Software/ModemManager/) |

### Power Management

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **UPower** | Power device enumeration, battery status | [UPower docs](https://upower.freedesktop.org/docs/) |

### Sandboxing

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **Portals** | Sandboxed app access to system features | [Portal docs](https://flatpak.github.io/xdg-desktop-portal/docs/index.html) |
| **libportal** | C library for portals | [libportal docs](https://flatpak.github.io/libportal/) |
| **ashpd** | Rust library for portals | [ashpd docs](https://docs.rs/ashpd/latest/ashpd/) |

### Media

| Service | Purpose | Documentation |
|---------|---------|---------------|
| **PipeWire** | Low-latency audio/video processing | [pipewire.org](https://pipewire.org/) |
| **MPRIS** | Media player remote control | [MPRIS spec](https://specifications.freedesktop.org/mpris-spec/latest/) |

## Programming Languages

| Language | Project | Documentation | Notes |
|----------|---------|---------------|-------|
| **C** | Native | GTK/GLib docs | Default platform language, recommended for libraries |
| **C++** | gtkmm | [gtkmm.gnome.org](https://gtkmm.gnome.org/) | Used by Inkscape, GParted |
| **Python** | PyGObject | [pygobject.gnome.org](https://pygobject.gnome.org/) | Python 3, PyPy3. Used by GNOME Music, Pitivi |
| **Rust** | gtk-rs | [gtk-rs.org](https://gtk-rs.org/) | Used by Authenticator, Shortwave |
| **JavaScript** | GJS | [gjs.guide](https://gjs.guide/) | ES6, SpiderMonkey. Used by Polari, Maps |
| **Vala** | Vala | [vala.dev](https://vala.dev/) | Compiles to C. Used by Calculator, Boxes |
| **C#** | gir.core | [gircore.github.io](https://gircore.github.io/) | .NET bindings |
| **Java** | Java-GI | [java-gi.org](https://java-gi.org/) | OpenJDK 25+ |

## Desktop Integration

### Desktop Files

```ini
[Desktop Entry]
Name=Your Application
Comment=An amazing application
Exec=your-application
Icon=com.example.YourApplication
Keywords=various;keywords;
StartupNotify=true
Terminal=false
```

Install to: `/usr/share/applications/com.example.YourApplication.desktop`

### Application Icons

Install in hicolor theme:
- `/usr/share/icons/hicolor/scalable/apps/com.example.YourApplication.svg`
- `/usr/share/icons/hicolor/256x256/apps/com.example.YourApplication.png`
- `/usr/share/icons/hicolor/symbolic/apps/com.example.YourApplication-symbolic.svg`

### D-Bus Activation

Create service file at `/usr/share/dbus-1/services/com.example.YourApplication.service`:

```ini
[D-BUS Service]
Name=com.example.YourApplication
Exec=/usr/bin/your-app --gapplication-service
```

Add to desktop file:
```ini
DBusActivatable=true
```

### MIME Types

Add to desktop file:
```ini
MimeType=image/png;application/pdf;
```

Use `%F` for multiple files, `%f` for single file in Exec line.

Register custom MIME type in `/usr/share/mime/packages/com.example.YourApplication.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
   <mime-type type="application/x-example">
     <comment>Example file type</comment>
     <glob pattern="*.example"/>
   </mime-type>
</mime-info>
```

## Quick Selection Guide

| Need | Library/Service |
|------|-----------------|
| Application window | GTK + libadwaita |
| Settings storage | GSettings |
| File operations | GFile |
| HTTP requests | libsoup |
| JSON parsing | JSON-GLib |
| Video playback | GStreamer or GtkVideo |
| Secrets/passwords | libsecret |
| Desktop notifications | GNotification or libnotify |
| D-Bus communication | GDBus |
| Sandboxed file access | Portals |

## External Resources

- [GNOME Components](https://developer.gnome.org/components/)
- [Apps for GNOME](https://apps.gnome.org)
- [Freedesktop Specifications](https://specifications.freedesktop.org/)
