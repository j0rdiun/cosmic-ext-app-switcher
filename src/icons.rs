use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};
use std::path::PathBuf;

pub fn icon_name_for(app_id: &str) -> String {
    if app_id.is_empty() {
        return "application-x-executable".to_string();
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let search_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from(format!("{home}/.local/share/applications")),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from(format!("{home}/.local/share/flatpak/exports/share/applications")),
    ];

    // Steam games: always use the generic Steam icon rather than trying to resolve
    // game-specific icons, which aren't reliably resolvable via from_name().
    if app_id.starts_with("steam_app_") {
        return "steam".to_string();
    }

    // Direct filename match: "firefox" -> "firefox.desktop"
    for dir in &search_dirs {
        let path = dir.join(format!("{app_id}.desktop"));
        if let Ok(bytes) = std::fs::read(&path) {
            let Ok(s) = std::str::from_utf8(&bytes) else { continue };
            let Ok(entry) = DesktopEntry::decode(&path, s) else { continue };
            if let Some(icon) = entry.icon() {
                return icon_name_from_field(icon);
            }
        }
    }

    // Scan all .desktop files for StartupWMClass match
    for path in DesktopIter::new(search_dirs) {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(s) = std::str::from_utf8(&bytes) else { continue };
        let Ok(entry) = DesktopEntry::decode(&path, s) else { continue };
        let wm_class = entry.startup_wm_class().unwrap_or_default();
        if wm_class.eq_ignore_ascii_case(app_id) {
            if let Some(icon) = entry.icon() {
                return icon_name_from_field(icon);
            }
        }
    }

    // Fallback: last component of reverse-DNS name
    app_id.split('.').last().unwrap_or(app_id).to_string()
}

// If the Icon= field in a .desktop file is an absolute path, extract the stem
// (e.g. "/path/to/steam_icon_480.png" -> "steam_icon_480") so that from_name()
// can resolve it through the XDG icon theme where Steam has already installed it.
fn icon_name_from_field(icon: &str) -> String {
    if icon.starts_with('/') {
        std::path::Path::new(icon)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(icon)
            .to_string()
    } else {
        icon.to_string()
    }
}
