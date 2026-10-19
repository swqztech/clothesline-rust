use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const TEMPORARY_SUFFIXES: &[&str] = &[
    ".crdownload",
    ".download",
    ".part",
    ".partial",
    ".tmp",
    ".temp",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    pub path: PathBuf,
    pub is_directory: bool,
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
}

impl FileRecord {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn fingerprint(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.is_directory,
            self.size,
            self.modified.timestamp_millis(),
            self.created.timestamp_millis()
        )
    }

    pub fn read(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if meta.file_type().is_symlink() {
            return None;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let attrs = meta.file_attributes();
            const HIDDEN: u32 = 0x2;
            const SYSTEM: u32 = 0x4;
            if attrs & (HIDDEN | SYSTEM) != 0 {
                return None;
            }
        }
        let is_directory = meta.is_dir();
        let size = if is_directory { 0 } else { meta.len() };
        let modified = system_time_to_utc(meta.modified().ok()?);
        let created = system_time_to_utc(meta.created().ok()?);
        Some(Self {
            path: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
            is_directory,
            size,
            modified,
            created,
        })
    }
}

fn system_time_to_utc(t: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

pub fn is_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    if name.is_empty() || name.starts_with('.') || name.starts_with("~$") {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    !TEMPORARY_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

fn has_temp_companion(path: &Path) -> bool {
    TEMPORARY_SUFFIXES.iter().any(|s| {
        let p = path.with_extension("");
        let mut companion = path.as_os_str().to_owned();
        companion.push(s);
        let companion_path = PathBuf::from(companion);
        companion_path.exists() || {
            let mut with_suffix = path.to_path_buf();
            with_suffix.set_extension(format!(
                "{}{}",
                path.extension().map(|e| e.to_string_lossy()).unwrap_or_default(),
                s
            ));
            // 浏览器常见：file.pdf.crdownload
            let alt = PathBuf::from(format!("{}{}", path.display(), s));
            alt.exists()
        }
    }) || TEMPORARY_SUFFIXES.iter().any(|s| {
        PathBuf::from(format!("{}{}", path.display(), s)).exists()
    })
}

#[derive(Clone)]
struct Sample {
    fingerprint: String,
    since: DateTime<Utc>,
    count: i32,
}

pub struct FileIndex {
    samples: HashMap<PathBuf, Sample>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            samples: HashMap::new(),
        }
    }

    pub fn scan(
        &mut self,
        folders: &[PathBuf],
        now: DateTime<Utc>,
    ) -> (Vec<FileRecord>, Vec<String>) {
        let mut failures = Vec::new();
        let mut records: HashMap<PathBuf, FileRecord> = HashMap::new();
        let mut visited = HashSet::new();

        for folder in folders {
            let entries = match std::fs::read_dir(folder) {
                Ok(e) => e,
                Err(e) => {
                    failures.push(format!("{}: {}", folder.display(), e));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !is_candidate(&path) || has_temp_companion(&path) {
                    continue;
                }
                let Some(file) = FileRecord::read(&path) else {
                    continue;
                };
                visited.insert(file.path.clone());
                let base = match self.samples.get(&file.path) {
                    Some(s) if s.fingerprint == file.fingerprint() => s.clone(),
                    _ => Sample {
                        fingerprint: file.fingerprint(),
                        since: now,
                        count: 0,
                    },
                };
                let sample = Sample {
                    fingerprint: base.fingerprint,
                    since: base.since,
                    count: base.count + 1,
                };
                self.samples.insert(file.path.clone(), sample.clone());
                let stable_secs = (now - sample.since).num_seconds();
                let modified_secs = (now - file.modified).num_seconds();
                if sample.count >= 2 && stable_secs >= 2 && modified_secs >= 2 {
                    records.insert(file.path.clone(), file);
                }
            }
        }

        self.samples
            .retain(|p, _| visited.contains(p));

        let mut list = records.into_values().collect::<Vec<_>>();
        list.sort_by(|a, b| b.modified.cmp(&a.modified));
        (list, failures)
    }
}

#[derive(Clone, Default)]
pub struct RopeState {
    pub dismissed: HashMap<PathBuf, String>,
    pub pinned: HashSet<PathBuf>,
}

impl RopeState {
    pub fn select(&self, records: &[FileRecord], capacity: usize) -> Vec<FileRecord> {
        let mut files: Vec<FileRecord> = records
            .iter()
            .filter(|f| {
                self.dismissed
                    .get(&f.path)
                    .map(|fp| fp != &f.fingerprint())
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        files.sort_by(|a, b| b.modified.cmp(&a.modified));
        files.dedup_by(|a, b| a.path == b.path);

        let mut pins: Vec<FileRecord> = files
            .iter()
            .filter(|f| self.pinned.contains(&f.path))
            .cloned()
            .collect();
        pins.sort_by(|a, b| b.modified.cmp(&a.modified));

        let pin_set: HashSet<_> = pins.iter().map(|f| f.path.clone()).collect();
        let rest_cap = capacity.saturating_sub(pins.len());
        let rest: Vec<FileRecord> = files
            .into_iter()
            .filter(|f| !pin_set.contains(&f.path))
            .take(rest_cap)
            .collect();

        pins.extend(rest);
        pins
    }

    pub fn dismiss(&mut self, file: &FileRecord) {
        self.dismissed.insert(file.path.clone(), file.fingerprint());
        self.pinned.remove(&file.path);
    }

    pub fn clear_dismissed(&mut self) {
        self.dismissed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn stable_file_appears_after_two_scans() {
        let dir = std::env::temp_dir().join("clothesline_rust_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.txt");
        fs::write(&file, b"ok").unwrap();
        thread::sleep(Duration::from_millis(2100));

        let mut index = FileIndex::new();
        let t0 = Utc::now();
        let (a, _) = index.scan(&[dir.clone()], t0);
        thread::sleep(Duration::from_millis(2100));
        let t1 = Utc::now();
        let (b, _) = index.scan(&[dir.clone()], t1);
        assert!(a.is_empty());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].name(), "hello.txt");
        let _ = fs::remove_dir_all(&dir);
    }
}
