//! # Description
//! Privacy masking and restoration: a hash-only registry, a reversible
//! sanitize/restore pipeline (built-in PII detection plus user-configured
//! extension patterns), and config loading from global/project files.
//! # 描述
//! 隐私掩码与还原：仅存哈希的注册表、可逆的脱敏/还原管线（内置 PII
//! 检测 + 用户配置的扩展模式），以及全局/项目配置文件加载。

mod config;
mod filter;
mod registry;

pub use config::PrivacyMaskExtension;
pub use filter::{PrivacyMasker, global_privacy_registry, restore_masked_data, sanitize_prompt};
pub use registry::{Hash, MaskId, PrivacyRegistry, PrivacyRegistryEntry, PrivacyRegistrySnapshot};
