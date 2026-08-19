use super::*;

#[test]
fn store_recovers_from_poisoned_locks() {
    // Poisoning the global store must not wedge later accesses: every guard
    // acquisition path recovers via `into_inner` because reloads rebuild
    // state from disk.
    // 让全局存储中毒后，后续访问不应被卡住：所有获取 guard 的路径都会
    // 通过 `into_inner` 恢复，因为重载总是从磁盘重建状态。
    let _ = std::panic::catch_unwind(|| {
        let _guard = STORE.write().unwrap();
        std::panic::panic_any("poison on purpose");
    });
    assert!(STORE.is_poisoned());
    assert!(read_store().project_root.is_none());
    let root = write_store().project_root.clone();
    assert!(root.is_none());
}
