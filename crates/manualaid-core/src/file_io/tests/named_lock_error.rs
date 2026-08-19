use std::path::Path;

use super::*;

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
