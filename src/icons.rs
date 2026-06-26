use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};
use std::path::PathBuf;

pub fn icon_name_for(app_id: &str) -> String {
    let search_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from(format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        )),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from(format!(
            "{}/.local/share/flatpak/exports/share/applications",
            std::env::var("HOME").unwrap_or_default()
        )),
    ];

    // Direct filename match: "firefox" -> "firefox.desktop"
    for dir in &search_dirs {
        let path = dir.join(format!("{app_id}.desktop"));
        if let Ok(bytes) = std::fs::read(&path) {
            let Ok(s) = std::str::from_utf8(&bytes) else { continue };
            let Ok(entry) = DesktopEntry::decode(&path, s) else { continue };
            if let Some(icon) = entry.icon() {
                return icon.to_string();
            }
        }
    }

    // Scan all .desktop files for StartupWMClass match
    // Handles apps like Chrome, Ghostty with non-standard app_ids
    for path in DesktopIter::new(search_dirs) {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(s) = std::str::from_utf8(&bytes) else { continue };
        let Ok(entry) = DesktopEntry::decode(&path, s) else { continue };
        let wm_class = entry.startup_wm_class().unwrap_or_default();
        if wm_class.eq_ignore_ascii_case(app_id) {
            if let Some(icon) = entry.icon() {
                return icon.to_string();
            }
        }
    }

    // Fallback: last component of reverse-DNS name
    app_id.split('.').last().unwrap_or(app_id).to_string()
}
