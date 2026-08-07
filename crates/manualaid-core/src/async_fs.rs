//! Async file I/O helpers built on `tokio::fs`, mapping errors into the
//! library's unified error type.
//! 基于 `tokio::fs` 的异步文件 I/O 辅助，将错误映射为库的统一错误类型。

use std::path::Path;

use crate::error::{CoreError, CoreResult};

/// Asynchronously read the whole file as UTF-8 text.
/// 异步读取整个文件为 UTF-8 文本。
pub async fn read_file(path: impl AsRef<Path>) -> CoreResult<String> {
    let path = path.as_ref();
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| CoreError::Io(format!("cannot read file `{}`: {e}", path.display())))
}

/// Asynchronously write bytes to a file, creating parent directories when
/// they are missing.
/// 异步将字节写入文件，父目录缺失时自动创建。
pub async fn write_file(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> CoreResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            CoreError::Io(format!(
                "cannot create parent directory `{}`: {e}",
                parent.display()
            ))
        })?;
    }
    tokio::fs::write(path, content)
        .await
        .map_err(|e| CoreError::Io(format!("cannot write file `{}`: {e}", path.display())))?;
    Ok(())
}
