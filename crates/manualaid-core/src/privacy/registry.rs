//! Session-scoped, in-memory registry of private-data ↔ mask mappings.
//! 会话范围内的内存注册表，管理隐私数据 ↔ 脱敏标识符的映射。
//!
//! # Description
//! Ensures the same plaintext value always maps to the same stable mask ID
//! (e.g. `[PRV_EMAIL_1]`) within a process lifetime. In-memory structures
//! store **only SHA-512 hashes** of plaintext values, never the plaintext
//! itself. Snapshots can be exported and loaded back later (e.g. for a
//! database or file layer) without ever containing plaintext.
//! # 描述
//! 保证同一明文在同一进程内始终映射到同一稳定掩码标识符（如
//! `[PRV_EMAIL_1]`）。内存中**只存储明文的 SHA-512 哈希**，绝不存储
//! 明文本身。快照可导出并重新加载（例如接入数据库或文件层），且
//! 永远不会包含明文。

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

use crate::error::{CoreError, CoreResult};

/// SHA-512 hash (64 bytes) used as the in-memory deduplication key.
/// 用作内存中去重键的 SHA-512 哈希值（64 字节）。
pub type Hash = [u8; 64];

/// Stable mask identifier such as `"[PRV_EMAIL_1]"`.
/// 稳定的脱敏标识符，例如 `"[PRV_EMAIL_1]"`。
pub type MaskId = String;

/// Runtime state — stores only hashes, never raw plaintext.
/// 运行时状态——仅存储哈希值，绝不存储原始明文。
#[derive(Default)]
struct RegistryInner {
    /// hash(plaintext) → stable mask_id.
    /// hash(明文) → 稳定的 mask_id。
    hash_to_mask: HashMap<Hash, MaskId>,
    /// Per-entity-type counter for generating the next stable ID (starts at
    /// 0; the first call for a type yields `[PRV_<TYPE>_1]`).
    /// 每个实体类型的计数器，用于生成下一个稳定 ID
    /// （从 0 开始；某类型首次调用生成 `[PRV_<TYPE>_1]`）。
    counters: HashMap<String, usize>,
}

/// One exported mapping: a stable mask ID plus the lowercase hex encoding of
/// the SHA-512 hash of its plaintext. Never contains plaintext.
/// 一条导出的映射：稳定掩码 ID 加上其明文 SHA-512 哈希的小写十六进制
/// 编码。绝不含明文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyRegistryEntry {
    /// Stable mask identifier, e.g. `[PRV_EMAIL_1]`.
    /// 稳定掩码标识符，例如 `[PRV_EMAIL_1]`。
    pub mask_id: MaskId,
    /// Lowercase hex encoding of `SHA-512(plaintext)` (128 hex characters).
    /// `SHA-512(明文)` 的小写十六进制编码（128 个十六进制字符）。
    pub hash_hex: String,
}

/// Serializable snapshot of a [`PrivacyRegistry`]. Contains only hashes and
/// counters — no plaintext — so it can be stored in a database or file and
/// loaded back later.
/// [`PrivacyRegistry`] 的可序列化快照。只包含哈希与计数器——不含明文——
/// 可存入数据库或文件，之后重新加载。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PrivacyRegistrySnapshot {
    /// All `hash → mask_id` mappings.
    /// 全部 `hash → mask_id` 映射。
    pub entries: Vec<PrivacyRegistryEntry>,
    /// Per-entity-type counters.
    /// 每个实体类型的计数器。
    pub counters: HashMap<String, usize>,
}

/// Session-scoped, in-memory registry of private-data ↔ mask mappings.
/// 会话范围内的内存注册表，管理隐私数据 ↔ 脱敏标识符的映射关系。
///
/// # Description
/// All lookups are O(1) and use only hashed plaintext values internally.
/// Use [`global_privacy_registry`](crate::privacy::global_privacy_registry)
/// for the process-wide instance, or create a fresh one for tests.
/// # 描述
/// 所有查找操作均为 O(1)，内部仅使用明文值的哈希。进程级实例见
/// [`global_privacy_registry`](crate::privacy::global_privacy_registry)，
/// 测试时可自行创建新实例。
pub struct PrivacyRegistry {
    inner: RwLock<RegistryInner>,
}

impl PrivacyRegistry {
    /// Create a new, empty registry.
    /// 创建一个新的空注册表。
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(RegistryInner::default()),
        }
    }

    /// Get or create a stable mask ID for the given plaintext.
    ///
    /// If the plaintext is already registered, returns its existing mask ID;
    /// otherwise generates and registers a new one (e.g. `[PRV_EMAIL_3]`).
    /// `entity_type` is used as the mask ID prefix (e.g. `"EMAIL"`).
    ///
    /// 获取或创建给定明文的稳定脱敏标识符。
    ///
    /// 如果该明文已经注册，返回其现有的脱敏标识符；否则生成并注册一个
    /// 新的（例如 `[PRV_EMAIL_3]`）。`entity_type` 用作脱敏标识符前缀
    /// （例如 `"EMAIL"`）。
    pub fn get_or_create(&self, entity_type: &str, plaintext: &str) -> CoreResult<MaskId> {
        let hash = hash_plaintext(plaintext);
        let mut inner = self
            .inner
            .write()
            .map_err(|e| CoreError::Other(format!("privacy registry lock poisoned: {e}")))?;

        // Fast path: already registered.
        // 快速路径：已经注册过。
        if let Some(id) = inner.hash_to_mask.get(&hash) {
            return Ok(id.clone());
        }

        let counter = inner.counters.entry(entity_type.to_string()).or_insert(0);
        *counter += 1;
        let mask_id = format!("[PRV_{entity_type}_{counter}]");

        inner.hash_to_mask.insert(hash, mask_id.clone());
        Ok(mask_id)
    }

    /// Normalise a (text, mapping) pair so all placeholders use stable IDs
    /// from this registry.
    ///
    /// `mapping` is a slice of `(placeholder, original_value)` tuples.
    /// Returns `(stable_text, stable_mapping)` where `stable_mapping` is a
    /// `HashMap<stable_placeholder, original_value>`.
    ///
    /// 规范化（文本，映射）对，使所有占位符使用此注册表中的稳定标识符。
    ///
    /// `mapping` 是 `(占位符, 原始值)` 元组的切片。返回
    /// `(stable_text, stable_mapping)`，其中 `stable_mapping` 是
    /// `HashMap<稳定占位符, 原始值>`。
    pub fn normalize(
        &self,
        text: &str,
        mapping: &[(String, String)],
    ) -> CoreResult<(String, HashMap<String, String>)> {
        let mut stable_mapping: HashMap<String, String> = HashMap::new();
        let mut replacements: Vec<(String, String)> = Vec::new(); // (old_ph, new_ph)

        for (placeholder, original) in mapping {
            let entity_type = extract_entity_type(placeholder);
            let stable_id = self.get_or_create(&entity_type, original)?;

            if stable_id != *placeholder {
                replacements.push((placeholder.clone(), stable_id.clone()));
            }
            stable_mapping.insert(stable_id, original.clone());
        }

        // Apply replacements in a single left-to-right pass, longest
        // placeholder first at each position. Sequential `String::replace`
        // is not idempotent here: a newly inserted placeholder can be
        // re-matched by a later old placeholder (e.g. `[PRV_EMAIL_10]` ->
        // `[PRV_EMAIL_1]` followed by `[PRV_EMAIL_1]` -> `[PRV_EMAIL_2]`),
        // corrupting the text. Advancing past each replacement keeps
        // inserted placeholders untouched.
        // 按从左到右单次扫描应用替换，每个位置优先匹配最长的占位符。
        // 逐条 `String::replace` 在这里不具备幂等性：新插入的占位符可能
        // 被后续旧占位符再次匹配（例如 `[PRV_EMAIL_10]` -> `[PRV_EMAIL_1]`
        // 之后再执行 `[PRV_EMAIL_1]` -> `[PRV_EMAIL_2]`），从而损坏文本。
        // 每次替换后越过已插入内容，保证新占位符不会被再次扫描。
        replacements.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
        let mut stable_text = String::with_capacity(text.len());
        let mut rest = text;
        while !rest.is_empty() {
            if let Some((old_ph, new_ph)) = replacements
                .iter()
                .find(|(old_ph, _)| !old_ph.is_empty() && rest.starts_with(old_ph.as_str()))
            {
                stable_text.push_str(new_ph);
                rest = &rest[old_ph.len()..];
            } else {
                let ch = rest.chars().next().expect("rest is not empty");
                stable_text.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }

        Ok((stable_text, stable_mapping))
    }

    /// Check whether a plaintext is already registered.
    /// 检查某个明文是否已经注册。
    pub fn contains(&self, plaintext: &str) -> CoreResult<bool> {
        let hash = hash_plaintext(plaintext);
        let inner = self
            .inner
            .read()
            .map_err(|e| CoreError::Other(format!("privacy registry lock poisoned: {e}")))?;
        Ok(inner.hash_to_mask.contains_key(&hash))
    }

    /// Return the number of registered unique plaintext values.
    /// 返回已注册的唯一明文值的数量。
    pub fn len(&self) -> CoreResult<usize> {
        let inner = self
            .inner
            .read()
            .map_err(|e| CoreError::Other(format!("privacy registry lock poisoned: {e}")))?;
        Ok(inner.hash_to_mask.len())
    }

    /// Returns `true` when the registry has no entries.
    /// 当注册表中没有条目时返回 `true`。
    pub fn is_empty(&self) -> CoreResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Export the current state as a serializable snapshot. Entries are
    /// sorted by `mask_id` for deterministic output; counters are exported
    /// as-is so a snapshot round-trips exactly.
    ///
    /// 将当前状态导出为可序列化快照。条目按 `mask_id` 排序以保证输出
    /// 确定；计数器原样导出，保证快照精确往返。
    pub fn export_snapshot(&self) -> CoreResult<PrivacyRegistrySnapshot> {
        let inner = self
            .inner
            .read()
            .map_err(|e| CoreError::Other(format!("privacy registry lock poisoned: {e}")))?;
        let mut entries: Vec<PrivacyRegistryEntry> = inner
            .hash_to_mask
            .iter()
            .map(|(hash, mask_id)| PrivacyRegistryEntry {
                mask_id: mask_id.clone(),
                hash_hex: hash_to_hex(hash),
            })
            .collect();
        entries.sort_by(|a, b| a.mask_id.cmp(&b.mask_id));
        Ok(PrivacyRegistrySnapshot {
            entries,
            counters: inner.counters.clone(),
        })
    }

    /// Rebuild a registry from a snapshot (replace semantics — the loaded
    /// state fully replaces the current one).
    ///
    /// Validation: every `hash_hex` must be a 128-character hex string;
    /// `mask_id`s must be unique and parseable as `[PRV_<TYPE>_<N>]` (or
    /// legacy `[<TYPE>_<N>]`) with `N >= 1`; each counter must be at least
    /// the maximum number seen among that type's entries (larger counters
    /// are allowed to preserve history).
    ///
    /// 从快照重建注册表（整体替换语义——加载的状态完全替换当前状态）。
    ///
    /// 校验：每个 `hash_hex` 必须是 128 位十六进制字符串；`mask_id` 必须
    /// 唯一且可解析为 `[PRV_<类型>_<编号>]`（或旧式 `[<类型>_<编号>]`）
    /// 且 `编号 >= 1`；每个计数器必须不小于该类条目中出现的最大编号
    /// （允许更大以保留历史计数）。
    pub fn from_snapshot(snapshot: PrivacyRegistrySnapshot) -> CoreResult<Self> {
        let mut hash_to_mask: HashMap<Hash, MaskId> = HashMap::new();
        let mut seen_mask_ids: HashSet<&str> = HashSet::new();
        let mut max_per_type: HashMap<String, usize> = HashMap::new();

        for entry in &snapshot.entries {
            if entry.mask_id.is_empty() {
                return Err(CoreError::Config(
                    "invalid privacy registry snapshot: empty mask_id".to_string(),
                ));
            }
            if !seen_mask_ids.insert(entry.mask_id.as_str()) {
                return Err(CoreError::Config(format!(
                    "invalid privacy registry snapshot: duplicate mask_id `{}`",
                    entry.mask_id
                )));
            }
            let hash = hex_to_hash(&entry.hash_hex).ok_or_else(|| {
                CoreError::Parse(format!(
                    "invalid privacy registry snapshot: invalid hash_hex `{}` for mask_id `{}`: expected 128 hex characters",
                    entry.hash_hex, entry.mask_id
                ))
            })?;
            if let Some(existing) = hash_to_mask.get(&hash) {
                return Err(CoreError::Config(format!(
                    "invalid privacy registry snapshot: hash_hex `{}` maps to both `{existing}` and `{}`",
                    entry.hash_hex, entry.mask_id
                )));
            }
            hash_to_mask.insert(hash, entry.mask_id.clone());

            let (entity_type, number) = parse_mask_id(&entry.mask_id).ok_or_else(|| {
                CoreError::Config(format!(
                    "invalid privacy registry snapshot: cannot parse mask_id `{}` (expected `[PRV_<TYPE>_<N>]` or `[<TYPE>_<N>]` with N >= 1)",
                    entry.mask_id
                ))
            })?;
            let max_n = max_per_type.entry(entity_type).or_insert(0);
            *max_n = (*max_n).max(number);
        }

        for (entity_type, max_n) in &max_per_type {
            let counter = snapshot.counters.get(entity_type).copied().unwrap_or(0);
            if counter < *max_n {
                return Err(CoreError::Config(format!(
                    "invalid privacy registry snapshot: counter for `{entity_type}` is {counter}, but entries require at least {max_n}"
                )));
            }
        }

        Ok(Self {
            inner: RwLock::new(RegistryInner {
                hash_to_mask,
                counters: snapshot.counters,
            }),
        })
    }
}

impl Default for PrivacyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the SHA-512 hash of a plaintext string.
/// 计算明文字符串的 SHA-512 哈希值。
fn hash_plaintext(text: &str) -> Hash {
    let mut hasher = Sha512::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    let mut hash = [0u8; 64];
    hash.copy_from_slice(&result);
    hash
}

/// Encode a hash as 128 lowercase hex characters.
/// 将哈希编码为 128 个小写十六进制字符。
fn hash_to_hex(hash: &Hash) -> String {
    let mut s = String::with_capacity(128);
    for byte in hash {
        write!(s, "{byte:02x}").expect("writing to a String cannot fail");
    }
    s
}

/// Decode a 128-character hex string into a hash. Accepts upper and lower
/// case; returns `None` for malformed input.
/// 将 128 位十六进制字符串解码为哈希。大小写均可；格式非法返回 `None`。
fn hex_to_hash(hex: &str) -> Option<Hash> {
    if hex.len() != 128 {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut hash = [0u8; 64];
    for (i, chunk) in bytes.chunks_exact(2).enumerate() {
        let hi = hex_val(chunk[0])?;
        let lo = hex_val(chunk[1])?;
        hash[i] = (hi << 4) | lo;
    }
    Some(hash)
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Parse a mask ID into `(entity_type, number)`.
///
/// Accepts `[PRV_EMAIL_1]` / `[PRV_CREDIT_CARD_3]` and legacy `[EMAIL_2]`
/// forms; the number must be >= 1.
/// 将掩码 ID 解析为 `(实体类型, 编号)`。
///
/// 接受 `[PRV_EMAIL_1]` / `[PRV_CREDIT_CARD_3]` 及旧式 `[EMAIL_2]` 形式；
/// 编号必须 >= 1。
fn parse_mask_id(mask_id: &str) -> Option<(String, usize)> {
    let inner = mask_id
        .strip_prefix("[PRV_")
        .and_then(|s| s.strip_suffix(']'))
        .or_else(|| mask_id.strip_prefix('[').and_then(|s| s.strip_suffix(']')))?;
    let idx = inner.rfind('_')?;
    let number: usize = inner[idx + 1..].parse().ok()?;
    if number == 0 {
        return None;
    }
    Some((inner[..idx].to_string(), number))
}

/// Extract the entity-type prefix from a placeholder.
///
/// `"[PRV_EMAIL_1]"` or `"[EMAIL_2]"` → `"EMAIL"`;
/// `"[PRV_CREDIT_CARD_3]"` → `"CREDIT_CARD"`.
/// 从脱敏标识符中提取实体类型前缀。
///
/// `"[PRV_EMAIL_1]"` 或 `"[EMAIL_2]"` → `"EMAIL"`；
/// `"[PRV_CREDIT_CARD_3]"` → `"CREDIT_CARD"`。
fn extract_entity_type(placeholder: &str) -> String {
    // Try the new [PRV_…] format first.
    // 优先尝试新的 [PRV_…] 格式。
    if let Some(inner) = placeholder
        .strip_prefix("[PRV_")
        .and_then(|s| s.strip_suffix(']'))
        && let Some(idx) = inner.rfind('_')
    {
        return inner[..idx].to_string();
    }
    // Fallback to the bare [ENTITY_…] format (external / legacy placeholders).
    // 回退到裸 [ENTITY_…] 格式（外部 / 旧式占位符）。
    let trimmed = placeholder
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(placeholder);
    if let Some(idx) = trimmed.rfind('_') {
        trimmed[..idx].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
