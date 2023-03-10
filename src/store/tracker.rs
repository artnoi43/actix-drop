use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::clipboard::Clipboard;
use super::error::StoreError;
use super::persist;

/// Tracker is used to store in-memory actix-drop clipboard
pub struct Tracker {
    /// If a clipboard is `Clipboard::Mem`, its hash gets inserted as map key with value `Some(_)`
    /// If a clipboard is `Clipboard::Persist`, its hash gets inserted as map key with value `None`
    /// The one-shot sender is for aborting the timeout timer
    haystack: Mutex<HashMap<String, (Option<Clipboard>, oneshot::Sender<()>)>>,
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            haystack: Mutex::new(HashMap::new()),
        }
    }

    /// store_new_clipboard stores new clipboard in tracker.
    /// With each clipboard, a timer task will be dispatched
    /// to the background to expire it (see `async fn expire_timer`).
    /// If a new clipboard comes in with identical 4-byte hash,
    /// the previous clipboard timer thread is forced to return,
    /// and a the new clipboard with its own timer takes its place.
    pub fn store_new_clipboard(
        tracker: Arc<Self>,
        hash: &str,
        clipboard: Clipboard,
        dur: Duration,
    ) -> Result<(), StoreError> {
        // Drop the old timer thread
        if let Some((_, stopper)) = tracker.remove(&hash) {
            // Recevier might have been dropped
            if let Err(_) = stopper.send(()) {
                eprintln!("store_new_clipboard: failed to remove old timer for {hash}");
            }
        }

        let to_save = match clipboard.clone() {
            // Clipboard::Mem(data) => data will have to live in haystack
            clip @ Clipboard::Mem(_) => Some(clip),

            // Clipboard::Persist(data) => data does not have to live in haystack
            Clipboard::Persist(data) => {
                persist::write_clipboard_file(hash, data.as_ref())?;
                None
            }
        };

        let (tx, rx) = oneshot::channel();
        tokio::task::spawn(expire_timer(
            tracker.clone(),
            hash.to_owned(),
            dur.clone(),
            rx,
        ));

        tracker
            .haystack
            .lock()
            .expect("failed to lock haystack")
            .insert(hash.to_owned(), (to_save, tx));

        Ok(())
    }

    /// get_clipboard gets a clipboard whose entry key matches `hash`.
    /// Calling get_clipboard does not move the value out of haystack
    pub fn get_clipboard(&self, hash: &str) -> Option<Clipboard> {
        let mut haystack = self.haystack.lock().expect("failed to lock haystack");

        match haystack.get(hash) {
            // Clipboard::Mem
            Some(&(Some(ref clipboard), _)) => Some(clipboard.to_owned()),

            // Clipboard::Persist
            Some(&(None, _)) => {
                // If we could not read the file, remove it from haystack
                match persist::read_clipboard_file(hash) {
                    Err(err) => {
                        eprintln!("error reading file {hash}: {}", err.to_string());

                        // Clear dangling persisted clipboard from haystack
                        haystack.remove(hash);
                        return None;
                    }

                    Ok(data) => Some(Clipboard::Persist(data.into())),
                }
            }

            None => None,
        }
    }

    pub fn remove(&self, hash: &str) -> Option<(Option<Clipboard>, oneshot::Sender<()>)> {
        self.haystack
            .lock()
            .expect("failed to lock haystack")
            .remove(&hash.to_owned())
    }
}

/// expire_timer waits on 2 futures:
/// 1. the timer
/// 2. the abort signal
/// If the timer finishes first, expire_timer removes the entry from `tracker.haystack`.
/// If the abort signal comes first, expire_timer simply returns `Ok(())`.
async fn expire_timer(
    tracker: Arc<Tracker>,
    hash: String,
    dur: Duration,
    abort: oneshot::Receiver<()>,
) -> Result<(), StoreError> {
    tokio::select! {
        // Set a timer to remove clipboard once it expires
        _ = tokio::time::sleep(dur) => {
        if let Some((_, (clipboard, _))) = tracker.haystack
                .lock()
                .expect("failed to lock haystack")
                .remove_entry(&hash)
        {
            // Some(_, None) => clipboard persisted to disk
            if clipboard.is_none() {
                persist::rm_clipboard_file(hash)?;
            }
        }

    }
        // If we get cancellation signal, return from this function
        _ = abort => {
            println!("expire_timer: timer for {hash} extended for {dur:?}");
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(dead_code)] // Bad tests - actix/tokio runtime conflict, will come back later
mod tracker_tests {
    use super::*;

    #[test]
    fn test_store_get() {
        // We should be able to get multiple times
        let foo = "foo";
        let clip = Clipboard::Mem("eiei".into());
        let (tx, _) = oneshot::channel();

        let tracker = Tracker::new();
        tracker
            .haystack
            .lock()
            .expect("failed to lock haystack")
            .insert(foo.to_owned(), (Some(clip), tx));

        assert!(tracker.get_clipboard(foo).is_some());
        assert!(tracker.get_clipboard(foo).is_some());
        assert!(tracker.get_clipboard(foo).is_some());
    }

    #[test]
    fn test_store_tracker() {
        let foo = "foo";
        let bar = "bar";
        let hashes = vec![foo, bar];

        let tracker = Arc::new(Tracker::new());
        let dur = Duration::from_secs(1);
        for hash in hashes {
            Tracker::store_new_clipboard(
                tracker.clone(),
                &hash,
                Clipboard::Persist("eiei".into()),
                dur,
            )
            .expect("failed to insert into tracker");
        }

        let (_, r1) = oneshot::channel();
        let (_, r2) = oneshot::channel();
        let f1 = expire_timer(tracker.clone(), foo.to_string(), dur, r1);
        let f2 = expire_timer(tracker.clone(), bar.to_string(), dur, r2);

        let rt = actix_web::rt::Runtime::new().unwrap();

        rt.block_on(actix_web::rt::spawn(f1))
            .unwrap()
            .expect("fail to spawn f1");
        rt.block_on(actix_web::rt::spawn(f2))
            .unwrap()
            .expect("fail to spawn f2");

        if !tracker.haystack.lock().unwrap().is_empty() {
            panic!("tracker not empty after cleared");
        }
    }

    #[test]
    fn test_reset_timer() {
        let hash = "foo";
        let tracker = Arc::new(Tracker::new());

        let clipboard = Clipboard::Mem(vec![1u8, 2, 3].into());
        let two_secs = Duration::from_secs(2);
        let four_secs = Duration::from_secs(4);

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard.clone(), four_secs)
            .expect("failed to store to tracker");

        let rt = actix_web::rt::Runtime::new().unwrap();

        rt.block_on(rt.spawn(actix_web::rt::time::sleep(two_secs)))
            .expect("failed to sleep-block");

        Tracker::store_new_clipboard(tracker.clone(), hash, clipboard, four_secs)
            .expect("failed to re-write to tracker");

        rt.block_on(rt.spawn(actix_web::rt::time::sleep(two_secs)))
            .expect("failed to sleep-block");

        assert!(tracker.get_clipboard(hash).is_some());
    }
}
