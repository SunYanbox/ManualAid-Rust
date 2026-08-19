use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

mod lock_name;
mod named_lock_error;
mod with_file_lock;

fn temp_path(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-file-io-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}
