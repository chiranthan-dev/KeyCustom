//! Configuration files on disk: atomic saves, last-known-good copy,
//! recovery from damaged files, backups, import and export.
//!
//! Layout inside the data directory:
//! - `config.json`            current configuration
//! - `config.last-good.json`  configuration in effect before the last change
//! - `config.corrupt-*.json`  damaged files that were set aside
//! - `backups/config-*.json`  automatic and manual backups (newest 20 kept)

use crate::config::Config;
use crate::validate::validate;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_BACKUPS: usize = 20;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Write via a temporary file in the same folder and rename it over the
/// target, so readers never see a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn to_json(config: &Config) -> String {
    serde_json::to_string_pretty(config).expect("config serialises")
}

/// Parse and validate a configuration file's contents.
pub fn parse(text: &str) -> Result<Config, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON ({e})"))?;
    let version = value.get("version").and_then(|v| v.as_u64()).ok_or("missing \"version\"")?;
    if version > u64::from(crate::config::SCHEMA_VERSION) {
        return Err(format!("created by a newer version of KeyForge (schema {version})"));
    }
    let config: Config = serde_json::from_value(value).map_err(|e| format!("does not match the schema ({e})"))?;
    let (report, _) = validate(&config);
    if !report.ok() {
        return Err(format!("contains invalid settings: {}", report.error_text()));
    }
    Ok(config)
}

pub fn read_file(path: &Path) -> Result<Config, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("the file is too large to be a KeyForge configuration".into());
    }
    parse(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub created: u64,
    pub size: u64,
}

pub struct Loaded {
    pub config: Config,
    /// Shown to the user when the configuration had to be recovered.
    pub notice: Option<String>,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    pub fn last_good_path(&self) -> PathBuf {
        self.dir.join("config.last-good.json")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    pub fn save(&self, config: &Config) -> io::Result<()> {
        write_atomic(&self.config_path(), to_json(config).as_bytes())
    }

    /// Keep the configuration currently on disk as the last known-good copy.
    /// Called before a new configuration replaces it.
    pub fn save_last_good(&self) -> io::Result<()> {
        match fs::read(self.config_path()) {
            Ok(bytes) => write_atomic(&self.last_good_path(), &bytes),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn read_last_good(&self) -> Option<Config> {
        read_file(&self.last_good_path()).ok()
    }

    /// Load the configuration, recovering from a missing or damaged file.
    pub fn load(&self) -> Loaded {
        let path = self.config_path();
        let problem = match fs::metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let config = Config::default();
                let _ = self.save(&config);
                return Loaded { config, notice: None };
            }
            Err(e) => format!("could not be read ({e})"),
            Ok(_) => match read_file(&path) {
                Ok(config) => return Loaded { config, notice: None },
                Err(e) => e,
            },
        };

        let kept = format!("config.corrupt-{}.json", now_ms());
        let moved = fs::rename(&path, self.dir.join(&kept)).is_ok();
        let kept_note = if moved { format!(" The damaged file was kept as {kept}.") } else { String::new() };
        let (config, what) = match self.read_last_good() {
            Some(config) => (config, "The last working configuration was restored"),
            None => (Config::default(), "Default settings were restored"),
        };
        let _ = self.save(&config);
        Loaded { config, notice: Some(format!("Your configuration {problem}. {what}.{kept_note}")) }
    }

    /// Copy the current configuration into the backups folder.
    pub fn backup(&self, reason: &str) -> io::Result<Option<String>> {
        let bytes = match fs::read(self.config_path()) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let reason: String = reason.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect();
        let name = format!("config-{}-{reason}.json", now_ms());
        write_atomic(&self.backups_dir().join(&name), &bytes)?;
        let mut all = self.list_backups();
        while all.len() > MAX_BACKUPS {
            let oldest = all.pop().unwrap();
            let _ = fs::remove_file(self.backups_dir().join(oldest.name));
        }
        Ok(Some(name))
    }

    /// Backups, newest first.
    pub fn list_backups(&self) -> Vec<BackupInfo> {
        let mut out: Vec<BackupInfo> = fs::read_dir(self.backups_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let created = name.strip_prefix("config-")?.split(['-', '.']).next()?.parse().ok()?;
                Some(BackupInfo { size: entry.metadata().ok()?.len(), name, created })
            })
            .collect();
        out.sort_by(|a, b| b.created.cmp(&a.created).then(b.name.cmp(&a.name)));
        out
    }

    pub fn read_backup(&self, name: &str) -> Result<Config, String> {
        let safe = name.starts_with("config-") && name.ends_with(".json") && !name.contains(['/', '\\']) && !name.contains("..");
        if !safe {
            return Err("Unknown backup.".into());
        }
        read_file(&self.backups_dir().join(name))
    }

    pub fn export(&self, config: &Config, path: &Path) -> io::Result<()> {
        write_atomic(path, to_json(config).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::tests::{config_with, remap};

    fn temp_store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("kf-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store::new(dir)
    }

    #[test]
    fn first_run_writes_defaults() {
        let store = temp_store("first");
        let loaded = store.load();
        assert!(loaded.notice.is_none());
        assert_eq!(loaded.config, Config::default());
        assert!(store.config_path().exists());
    }

    #[test]
    fn round_trip_and_last_good() {
        let store = temp_store("rt");
        let a = config_with(vec![remap("a", "F13", "F14")]);
        store.save(&a).unwrap();
        store.save_last_good().unwrap();
        let b = config_with(vec![remap("b", "F15", "F16")]);
        store.save(&b).unwrap();
        assert_eq!(store.load().config, b);
        assert_eq!(store.read_last_good().unwrap(), a);
    }

    #[test]
    fn damaged_file_restores_last_good() {
        let store = temp_store("corrupt");
        let good = config_with(vec![remap("a", "F13", "F14")]);
        store.save(&good).unwrap();
        store.save_last_good().unwrap();
        fs::write(store.config_path(), "{ not json").unwrap();
        let loaded = store.load();
        assert_eq!(loaded.config, good);
        let notice = loaded.notice.unwrap();
        assert!(notice.contains("last working configuration") && notice.contains("config.corrupt-"), "{notice}");
        assert_eq!(store.load().config, good, "recovered config was saved");
        assert!(fs::read_dir(store.dir()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("config.corrupt-")));
    }

    #[test]
    fn invalid_or_newer_files_fall_back_to_defaults() {
        let store = temp_store("invalid");
        let mut bad = config_with(vec![remap("a", "Backspace", "F14")]);
        store.save(&bad).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.notice.unwrap().contains("invalid settings"));

        bad.version = 99;
        fs::write(store.config_path(), to_json(&bad)).unwrap();
        assert!(store.load().notice.unwrap().contains("newer version"));
    }

    #[test]
    fn backups_rotate_and_restore() {
        let store = temp_store("backups");
        assert_eq!(store.backup("manual").unwrap(), None, "nothing to back up yet");
        let c = config_with(vec![remap("a", "F13", "F14")]);
        store.save(&c).unwrap();
        for _ in 0..MAX_BACKUPS + 3 {
            store.backup("manual").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let list = store.list_backups();
        assert_eq!(list.len(), MAX_BACKUPS);
        assert!(list[0].created >= list[1].created);
        assert_eq!(store.read_backup(&list[0].name).unwrap(), c);
        assert!(store.read_backup("../config.json").is_err());
    }

    #[test]
    fn export_then_import() {
        let store = temp_store("export");
        let c = config_with(vec![remap("a", "F13", "F14")]);
        let path = store.dir().join("exported.json");
        store.export(&c, &path).unwrap();
        assert_eq!(read_file(&path).unwrap(), c);
        fs::write(&path, "[]").unwrap();
        assert!(read_file(&path).is_err());
    }
}
