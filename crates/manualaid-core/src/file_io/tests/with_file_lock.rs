use std::io::Write;

use super::*;

#[test]
fn with_file_lock_writes_and_reads() {
    let path = temp_path("roundtrip");
    let _ = std::fs::remove_file(&path);
    with_file_lock(&path, || {
        std::fs::write(&path, "content").map_err(CoreError::from)
    })
    .expect("write should succeed");
    let read = with_file_lock(&path, || {
        std::fs::read_to_string(&path).map_err(CoreError::from)
    })
    .expect("read should succeed");
    assert_eq!(read, "content");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn with_file_lock_serializes_threads() {
    let path = temp_path("threads");
    let _ = std::fs::remove_file(&path);
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                with_file_lock(&path, || {
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .map_err(CoreError::from)?;
                    writeln!(file, "line").map_err(CoreError::from)
                })
                .expect("locked write should succeed")
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("thread should finish");
    }
    let content = std::fs::read_to_string(&path).expect("file should exist");
    assert_eq!(content, "line\n".repeat(8));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn with_file_lock_recovers_from_poisoned_mutex() {
    // A panicking holder leaves the lock poisoned; with_file_lock must
    // recover instead of propagating the poison. Later tests keep
    // working because every access path recovers the same way.
    // 持有者 panic 会让锁进入中毒状态；with_file_lock 必须恢复而非传播
    // 中毒。后续测试仍正常工作，因为所有访问路径都以相同方式恢复。
    let _ = std::panic::catch_unwind(|| {
        let _guard = FILE_IO_LOCK.lock().unwrap();
        std::panic::panic_any("poison on purpose");
    });
    assert!(FILE_IO_LOCK.is_poisoned());
    let path = temp_path("poisoned");
    let _ = std::fs::remove_file(&path);
    with_file_lock(&path, || Ok(())).expect("recover from poisoned lock");
    let _ = std::fs::remove_file(&path);
}
