use notify::{Watcher, RecursiveMode, Config, Event};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use anyhow::Result;

pub enum FileEvent {
    Modified(PathBuf),
    Created(PathBuf),
    Deleted(PathBuf),
}

pub struct RepoWatcher {
    watcher: notify::RecommendedWatcher,
    pub receiver: Receiver<FileEvent>,
}

impl RepoWatcher {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let (tx, rx) = channel();
        let (event_tx, event_rx) = channel();

        let mut watcher = notify::RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

        std::thread::spawn(move || {
            for res in rx {
                match res {
                    Ok(event) => {
                        if let Some(file_event) = Self::map_event(event) {
                            let _ = event_tx.send(file_event);
                        }
                    }
                    Err(e) => eprintln!("watch error: {:?}", e),
                }
            }
        });

        Ok(Self {
            watcher,
            receiver: event_rx,
        })
    }

    fn map_event(event: Event) -> Option<FileEvent> {
        if event.kind.is_modify() {
            Some(FileEvent::Modified(event.paths[0].clone()))
        } else if event.kind.is_create() {
            Some(FileEvent::Created(event.paths[0].clone()))
        } else if event.kind.is_remove() {
            Some(FileEvent::Deleted(event.paths[0].clone()))
        } else {
            None
        }
    }
}
