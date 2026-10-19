use crate::file_index::{FileIndex, FileRecord};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

pub enum WatcherEvent {
    Scanned(Vec<FileRecord>),
}

pub struct WatcherHandle {
    _tx_stop: Sender<()>,
}

impl WatcherHandle {
    pub fn stop(self) {
        let _ = self._tx_stop.send(());
    }
}

pub fn spawn(
    watch_folders: Vec<PathBuf>,
    manual_paths: Vec<PathBuf>,
    out: Sender<WatcherEvent>,
) -> WatcherHandle {
    let (tx_stop, rx_stop) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut index = FileIndex::new();
        loop {
            if rx_stop.try_recv().is_ok() {
                break;
            }
            let now = Utc::now();
            let (auto, _) = index.scan(&watch_folders, now);
            let mut manual = Vec::new();
            for p in &manual_paths {
                if let Some(r) = FileRecord::read(p) {
                    manual.push(r);
                }
            }
            let mut all = auto;
            for m in manual {
                if !all.iter().any(|x| x.path == m.path) {
                    all.push(m);
                }
            }
            all.sort_by(|a, b| b.modified.cmp(&a.modified));
            let _ = out.send(WatcherEvent::Scanned(all));
            thread::sleep(Duration::from_secs(1));
        }
    });
    WatcherHandle { _tx_stop: tx_stop }
}

pub fn drain_records(rx: &Receiver<WatcherEvent>) -> Option<Vec<FileRecord>> {
    let mut latest = None;
    while let Ok(ev) = rx.try_recv() {
        if let WatcherEvent::Scanned(r) = ev {
            latest = Some(r);
        }
    }
    latest
}
