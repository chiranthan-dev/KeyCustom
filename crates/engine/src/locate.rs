//! Finding an AutoHotkey v2 executable.

use std::path::{Path, PathBuf};

/// Environment variable that overrides every other location.
pub const ENV_OVERRIDE: &str = "KEYFORGE_AHK";
pub const EXE_NAME: &str = "AutoHotkey64.exe";

/// Candidate locations, most preferred first: the override variable, the copy
/// bundled with KeyForge, then the standard per-machine and per-user installs.
pub fn candidates(bundled_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os(ENV_OVERRIDE) {
        out.push(PathBuf::from(p));
    }
    if let Some(dir) = bundled_dir {
        out.push(dir.join(EXE_NAME));
    }
    for (var, sub) in [("ProgramFiles", "AutoHotkey"), ("LOCALAPPDATA", "Programs\\AutoHotkey")] {
        if let Some(base) = std::env::var_os(var) {
            out.push(PathBuf::from(base).join(sub).join("v2").join(EXE_NAME));
        }
    }
    out
}

pub fn find(bundled_dir: Option<&Path>) -> Option<PathBuf> {
    candidates(bundled_dir).into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_copy_is_found() {
        let dir = std::env::temp_dir().join(format!("kf-locate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(EXE_NAME), b"").unwrap();
        if std::env::var_os(ENV_OVERRIDE).is_none() {
            assert_eq!(find(Some(&dir)), Some(dir.join(EXE_NAME)));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
