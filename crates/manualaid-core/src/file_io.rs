//! Locked file I/O for `.ManualAid/*` files.
//! `.ManualAid/*` 文件的加锁文件 I/O。
//!
//! # Description
//! All access to `.ManualAid/*` files must go through [`with_file_lock`],
//! which holds a process-internal mutex and a cross-process named lock.
//! # Test notes
//! The cross-process lock cannot be exercised reliably in CI (Windows named
//! mutexes and Unix `flock` behave differently), and the OS-level failure
//! branches of `NamedLock::create` / `lock` depend on system state; these
//! branches are not covered by tests.
//! # 描述
//! 所有对 `.ManualAid/*` 文件的访问都必须经过 [`with_file_lock`]，它持有
//! 进程内互斥锁与跨进程命名锁。
//! # 测试说明
//! 跨进程锁无法在 CI 中可靠验证（Windows 命名互斥量与 Unix `flock` 行为
//! 不同），`NamedLock::create` / `lock` 的 OS 级失败分支依赖系统状态；
//! 这些分支不要求测试覆盖。

use std::path::Path;
use std::sync::{Mutex, Once};

use named_lock::NamedLock;

use crate::error::{CoreError, CoreResult};

/// Serializes all `.ManualAid` file operations within this process.
/// 串行化本进程内所有 `.ManualAid` 文件操作。
static FILE_IO_LOCK: Mutex<()> = Mutex::new(());

/// Recover from a poisoned lock while logging a one-time warning, so a prior
/// panic is not silently swallowed. Recovery is safe for `Mutex<()>` and for
/// the skill store (the config file remains the durable source of truth).
pub(crate) fn warn_poisoned_lock(lock: &str) {
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        log::warn!(
            "manualaid-core: recovered from poisoned lock `{lock}`; a thread previously panicked while holding it"
        );
    });
}

/// Run `f` with the file-level locks for `path` held.
/// 持有 `path` 对应的文件级锁执行 `f`。
///
/// # Description
/// Acquires the process-internal mutex first, then the cross-process named
/// lock derived from `path`; both are released on return. All filesystem
/// access to `.ManualAid/*` files must go through this function.
/// # 描述
/// 先获取进程内互斥锁，再获取由 `path` 派生的跨进程命名锁；返回时两者
/// 自动释放。所有对 `.ManualAid/*` 文件系统的访问都必须经过本函数。
pub(crate) fn with_file_lock<T>(path: &Path, f: impl FnOnce() -> CoreResult<T>) -> CoreResult<T> {
    let _process_guard = match FILE_IO_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn_poisoned_lock("FILE_IO_LOCK");
            poisoned.into_inner()
        }
    };
    let named =
        NamedLock::create(&lock_name(path)).map_err(|e| named_lock_error(path, "create", e))?;
    let _named_guard = named
        .lock()
        .map_err(|e| named_lock_error(path, "acquire", e))?;
    f()
}

/// Derive the cross-process lock name for a file path.
///
/// The raw path cannot be used as a lock name (`named-lock` forbids `/`,
/// `\` and `\0`, and Windows limits object-name length), so the lowercased,
/// `/`-normalized path is hashed. Lowercasing makes case-only differences
/// share one lock.
/// 派生文件路径对应的跨进程锁名。
///
/// 原始路径不能直接用作锁名（`named-lock` 禁止 `/`、`\`、`\0`，且 Windows
/// 对对象名长度有限制），因此对小写化并统一 `/` 分隔符后的路径取散列。
/// 小写化使仅大小写不同的路径共享同一把锁。
fn lock_name(path: &Path) -> String {
    let input = path.to_string_lossy().replace('\\', "/").to_lowercase();
    format!("manualaid-{:016x}", fnv1a64(input.as_bytes()))
}

/// FNV-1a 64-bit hash.
/// FNV-1a 64 位散列。
const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash
}

/// Map a `named-lock` failure to a `CoreError::Io` with context.
/// 将 `named-lock` 失败映射为带上下文的 `CoreError::Io`。
fn named_lock_error(path: &Path, operation: &str, err: named_lock::Error) -> CoreError {
    CoreError::Io(format!(
        "cannot {operation} named lock for `{}`: {err}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "manualaid-file-io-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn lock_name_is_stable_and_contains_no_separators() {
        let path = Path::new("/home/alice/.ManualAid/config.toml");
        let first = lock_name(path);
        assert_eq!(first, lock_name(path));
        assert!(!first.is_empty());
        assert!(!first.contains(['/', '\\']));
        assert_ne!(lock_name(Path::new("/home/alice/.ManualAid/a")), first);
    }

    #[test]
    fn lock_name_is_case_and_separator_insensitive() {
        let windows = Path::new(r"C:\Users\Alice\.ManualAid\config.toml");
        let normalized = Path::new("c:/users/alice/.manualaid/config.toml");
        assert_eq!(lock_name(windows), lock_name(normalized));
    }

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
    fn named_lock_error_maps_all_variants() {
        let path = Path::new("/tmp/x");
        for err in [
            named_lock::Error::EmptyName,
            named_lock::Error::InvalidCharacter,
            named_lock::Error::CreateFailed(std::io::Error::other("boom")),
            named_lock::Error::LockFailed,
            named_lock::Error::UnlockFailed,
            named_lock::Error::WouldBlock,
        ] {
            let core = named_lock_error(path, "acquire", err);
            assert!(matches!(core, CoreError::Io(_)));
            assert!(core.to_string().contains("named lock"));
        }
    }
}
