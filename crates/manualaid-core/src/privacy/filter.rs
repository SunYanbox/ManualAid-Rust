//! # Description
//! Reversible privacy masking: replace sensitive values with stable
//! placeholders (`[PRV_<TYPE>_<N>]`) before sending text to an LLM, and
//! restore the original values afterwards.
//!
//! The pipeline combines three detection sources with a fixed priority
//! (higher priority wins overlapping spans):
//! 1. `regex` patterns from the `[privacy_mask_extension.regex]` table;
//! 2. exact literal values from the `[privacy_mask_extension.literal]` table;
//! 3. built-in PII detection (cloakrs: emails, phones, credit cards, ...).
//! # 描述
//! 可逆隐私掩码：在把文本发送给 LLM 前，将敏感值替换为稳定占位符
//! （`[PRV_<类型>_<编号>]`），之后再把占位符还原为原始值。
//!
//! 管线合并三种检测来源，重叠区间按固定优先级处理（高优先级胜出）：
//! 1. `[privacy_mask_extension.regex]` 表中的正则模式；
//! 2. `[privacy_mask_extension.literal]` 表中的精确匹配值；
//! 3. cloakrs 内置 PII 检测（邮箱、手机号、银行卡等）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use cloakrs_core::Scanner;
use regex::Regex;

use crate::error::{CoreError, CoreResult};
use crate::privacy::config::PrivacyMaskExtension;
use crate::privacy::registry::PrivacyRegistry;

/// Prefix used for the named alternation groups of a combined tier regex.
/// Collisions with user patterns are handled by falling back to per-pattern
/// scanning.
/// 合并层正则的命名分组前缀。与用户模式重名时回退到逐模式扫描。
const ALT_GROUP_PREFIX: &str = "__manualaid_alt_";

/// Process-wide privacy registry shared by all maskers, so the same
/// plaintext always maps to the same mask ID across turns.
/// 进程级隐私注册表，由所有 masker 共享，保证同一明文跨轮次映射到
/// 同一掩码 ID。
pub fn global_privacy_registry() -> &'static PrivacyRegistry {
    static REGISTRY: OnceLock<PrivacyRegistry> = OnceLock::new();
    REGISTRY.get_or_init(PrivacyRegistry::new)
}

/// A finding from any detection source, expressed as a byte span in the
/// original text plus the entity type used for the mask ID prefix.
/// 任一检测来源产出的命中项：原始文本中的字节区间，以及用于掩码 ID
/// 前缀的实体类型。
struct Finding {
    start: usize,
    end: usize,
    entity_type: String,
}

/// One compiled pattern of a tier.
/// 某一层中的一个已编译模式。
struct TierPattern {
    entity_type: String,
    pattern: String,
    regex: Regex,
}

/// A compiled tier: either one combined regex with one named alternation
/// group per pattern (single-pass scan), or — if the combined regex cannot
/// be built (e.g. duplicate named groups in user patterns) — per-pattern
/// scanning.
/// 一个已编译的匹配层：要么是每个模式一个命名交替分组的单一正则
/// （单次扫描），要么——当无法构建合并正则（例如用户模式包含重名
/// 命名分组）——退化为逐模式扫描。
struct Tier {
    combined: Option<Regex>,
    alt_names: Vec<String>,
    patterns: Vec<TierPattern>,
}

/// A set of non-overlapping intervals sorted by start, used to enforce
/// priority between detection sources. Overlap checks are O(log k).
/// 按起点排序且互不重叠的区间集合，用于执行检测来源间的优先级。
/// 重叠检查为 O(log k)。
#[derive(Default)]
struct IntervalSet {
    intervals: Vec<(usize, usize)>,
}

impl IntervalSet {
    fn overlaps(&self, start: usize, end: usize) -> bool {
        if end <= start {
            return false;
        }
        let idx = self.intervals.partition_point(|&(s, _)| s < end);
        idx > 0 && self.intervals[idx - 1].1 > start
    }

    fn insert(&mut self, start: usize, end: usize) {
        let idx = self.intervals.partition_point(|&(s, _)| s < start);
        self.intervals.insert(idx, (start, end));
    }
}

/// Reversible privacy masker.
///
/// A masker owns the compiled extension patterns plus the built-in cloakrs
/// scanner, while the underlying [`PrivacyRegistry`] is the process-wide
/// instance (stable IDs across calls).
/// 可逆隐私掩码器。
///
/// masker 持有已编译的扩展模式与内置 cloakrs 扫描器；底层
/// [`PrivacyRegistry`] 使用进程级实例（跨调用保持稳定编号）。
pub struct PrivacyMasker {
    regex_tier: Option<Tier>,
    literal_tier: Option<Tier>,
    scanner: Scanner,
}

impl PrivacyMasker {
    /// Build a masker with built-in cloakrs PII detection only.
    /// 构建仅含内置 cloakrs PII 检测的 masker。
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            regex_tier: None,
            literal_tier: None,
            scanner: build_scanner()?,
        })
    }

    /// Build a masker with built-in detection plus the given extension
    /// patterns. Invalid patterns (compile failure or empty-string match)
    /// are logged and ignored; they never fail this call.
    /// 构建含内置检测与给定扩展模式的 masker。非法模式（编译失败或
    /// 可匹配空串）会被记录日志并忽略，不会导致本调用失败。
    pub fn from_extensions(extensions: &PrivacyMaskExtension) -> CoreResult<Self> {
        let regex_tier = compile_tier("regex", &extensions.regex, false);
        let literal_tier = compile_tier("literal", &extensions.literal, true);
        Ok(Self {
            regex_tier,
            literal_tier,
            scanner: build_scanner()?,
        })
    }

    /// Build a masker from the merged global + project configuration.
    /// 从合并后的全局 + 项目配置构建 masker。
    pub fn from_config(project_root: &Path) -> CoreResult<Self> {
        let extensions = PrivacyMaskExtension::load(project_root)?;
        Self::from_extensions(&extensions)
    }

    /// Replace sensitive values with stable placeholders.
    ///
    /// Returns `(masked_text, mapping)` where `mapping` maps each mask ID to
    /// its original plaintext; keep it and pass it to
    /// [`restore_masked_data`] for reversal.
    /// 将敏感值替换为稳定占位符。
    ///
    /// 返回 `(掩码文本, 映射)`，其中 `映射` 将每个掩码 ID 映射到其原始
    /// 明文；请保留该映射并传递给 [`restore_masked_data`] 进行还原。
    pub fn sanitize(&self, text: &str) -> CoreResult<(String, HashMap<String, String>)> {
        let mut occupied = IntervalSet::default();
        let mut selected: Vec<Finding> = Vec::new();

        // Priority order: regex > literal > cloakrs. Each tier is applied in
        // order; lower-priority findings overlapping an already-selected
        // higher-priority span are skipped.
        // 优先级顺序：正则 > 精确匹配 > cloakrs。按序处理每一层；与已选中
        // 高优先级区间重叠的低优先级命中会被跳过。
        if let Some(tier) = &self.regex_tier {
            selected.extend(apply_tier(scan_tier(tier, text), &mut occupied));
        }
        if let Some(tier) = &self.literal_tier {
            selected.extend(apply_tier(scan_tier(tier, text), &mut occupied));
        }
        let cloakrs_findings = scan_cloakrs(&self.scanner, text)?;
        selected.extend(apply_tier(cloakrs_findings, &mut occupied));

        apply_selected(&mut selected, text, global_privacy_registry())
    }
}

/// Convenience entry point: built-in cloakrs detection only (old signature).
/// For the full pipeline (built-in + config extensions) use
/// [`PrivacyMasker::from_config`] or [`PrivacyMasker::from_extensions`].
/// 便捷入口：仅内置 cloakrs 检测（保持旧版签名）。完整管线
/// （内置 + 配置扩展）请使用 [`PrivacyMasker::from_config`] 或
/// [`PrivacyMasker::from_extensions`]。
pub fn sanitize_prompt(text: &str) -> CoreResult<(String, HashMap<String, String>)> {
    PrivacyMasker::new()?.sanitize(text)
}

/// Restore original values from text containing placeholders produced by
/// [`sanitize_prompt`] / [`PrivacyMasker::sanitize`].
///
/// Single left-to-right pass that locates `[`/`]` pairs and looks them up in
/// `mapping`; O(n), naturally distinguishing `[PRV_EMAIL_1]` from
/// `[PRV_EMAIL_10]`. Brackets that do not form a known placeholder are kept
/// as-is.
/// 从包含 [`sanitize_prompt`] / [`PrivacyMasker::sanitize`] 生成的占位符
/// 的文本中恢复原始值。
///
/// 单次从左到右扫描，定位 `[`/`]` 配对并在 `mapping` 中查找；O(n)，天然
/// 区分 `[PRV_EMAIL_1]` 与 `[PRV_EMAIL_10]`。不构成已知占位符的方括号
/// 原样保留。
pub fn restore_masked_data(text: &str, mapping: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(pos) = rest.find('[') {
        result.push_str(&rest[..pos]);
        rest = &rest[pos..];

        if let Some(end) = rest.find(']') {
            let candidate = &rest[..=end];
            if let Some(original) = mapping.get(candidate) {
                result.push_str(original);
                rest = &rest[end + 1..];
                continue;
            }
        }
        result.push('[');
        rest = &rest[1..];
    }
    result.push_str(rest);
    result
}

fn build_scanner() -> CoreResult<Scanner> {
    use cloakrs_core::ScannerBuilder;

    let registry = cloakrs_locales::default_registry();
    ScannerBuilder::from_registry(registry)
        .without_masking()
        .build()
        .map_err(|e| CoreError::Filter(format!("failed to build sanitizer scanner: {e}")))
}

/// Compile one tier. `escape` is `true` for the literal tier (values are
/// treated as plain text). Patterns are sorted by original value length
/// descending, then key ascending, so at the same start position the longer
/// match wins (leftmost-first alternation).
/// 编译一个匹配层。`escape` 为 `true` 表示精确匹配层（值按普通文本
/// 处理）。模式按原始值长度降序、键名升序排序，使相同起点处更长的
/// 匹配胜出（左端优先交替）。
fn compile_tier(table: &str, patterns: &HashMap<String, String>, escape: bool) -> Option<Tier> {
    let mut items: Vec<(String, String, String)> = patterns
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.clone(),
                if escape {
                    regex::escape(value)
                } else {
                    value.clone()
                },
            )
        })
        .collect();
    items.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let mut compiled: Vec<TierPattern> = Vec::new();
    for (key, _original, pattern) in items {
        match Regex::new(&pattern) {
            Ok(re) if !re.is_match("") => compiled.push(TierPattern {
                entity_type: key,
                pattern,
                regex: re,
            }),
            Ok(_) => log::error!(
                "privacy mask extension: invalid pattern for key `privacy_mask_extension.{table}.{key}`: pattern matches empty string; ignored"
            ),
            Err(e) => log::error!(
                "privacy mask extension: invalid pattern for key `privacy_mask_extension.{table}.{key}`: {e}; ignored"
            ),
        }
    }
    if compiled.is_empty() {
        return None;
    }

    let alt_names: Vec<String> = (0..compiled.len())
        .map(|i| format!("{ALT_GROUP_PREFIX}{i}"))
        .collect();
    let mut expr = String::new();
    for (i, pattern) in compiled.iter().enumerate() {
        if i > 0 {
            expr.push('|');
        }
        expr.push_str("(?P<");
        expr.push_str(&alt_names[i]);
        expr.push('>');
        expr.push_str(&pattern.pattern);
        expr.push(')');
    }
    let combined = match Regex::new(&expr) {
        Ok(re) => Some(re),
        Err(e) => {
            log::warn!(
                "privacy mask extension: cannot build combined regex for table `{table}` ({e}); falling back to per-pattern scanning"
            );
            None
        }
    };
    Some(Tier {
        combined,
        alt_names,
        patterns: compiled,
    })
}

fn scan_tier(tier: &Tier, text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(combined) = &tier.combined {
        for caps in combined.captures_iter(text) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let mut idx = None;
            for (i, _) in tier.patterns.iter().enumerate() {
                if caps.name(&tier.alt_names[i]).is_some() {
                    idx = Some(i);
                    break;
                }
            }
            let Some(idx) = idx else {
                continue;
            };
            findings.push(Finding {
                start: full.start(),
                end: full.end(),
                entity_type: tier.patterns[idx].entity_type.clone(),
            });
        }
    } else {
        for pattern in &tier.patterns {
            for m in pattern.regex.find_iter(text) {
                findings.push(Finding {
                    start: m.start(),
                    end: m.end(),
                    entity_type: pattern.entity_type.clone(),
                });
            }
        }
    }
    findings
}

fn scan_cloakrs(scanner: &Scanner, text: &str) -> CoreResult<Vec<Finding>> {
    let result = scanner
        .scan(text)
        .map_err(|e| CoreError::Filter(format!("scanning failed: {e}")))?;
    let mut findings = Vec::new();
    for finding in result.findings {
        let tag = finding.entity_type.redaction_tag();
        let entity_type = tag
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        findings.push(Finding {
            start: finding.span.start,
            end: finding.span.end,
            entity_type,
        });
    }
    Ok(findings)
}

/// Select the non-overlapping findings of one tier: longest span first
/// (end descending), skipping anything that overlaps an already-selected
/// interval (from higher-priority tiers or earlier in this tier).
/// 选择某一层的互不重叠命中：end 降序优先较长区间，跳过与已选中区间
/// （来自更高优先级层或本层先前选中）重叠的命中。
fn apply_tier(mut findings: Vec<Finding>, occupied: &mut IntervalSet) -> Vec<Finding> {
    findings.sort_by(|a, b| b.end.cmp(&a.end).then_with(|| a.start.cmp(&b.start)));
    let mut kept = Vec::new();
    for finding in findings {
        if occupied.overlaps(finding.start, finding.end) {
            continue;
        }
        occupied.insert(finding.start, finding.end);
        kept.push(finding);
    }
    kept
}

/// Replace the selected spans back-to-front on the original text (byte
/// offsets stay valid), registering each unique plaintext once.
/// 按 end 降序在原始文本上后向替换已选中的区间（字节偏移保持有效），
/// 每个唯一明文只注册一次。
fn apply_selected(
    selected: &mut [Finding],
    text: &str,
    registry: &PrivacyRegistry,
) -> CoreResult<(String, HashMap<String, String>)> {
    selected.sort_by_key(|b| std::cmp::Reverse(b.end));
    let mut masked = text.to_string();
    let mut mapping = HashMap::new();
    for finding in selected.iter() {
        let plaintext = &text[finding.start..finding.end];
        let mask_id = registry
            .get_or_create(&finding.entity_type, plaintext)
            .map_err(|e| CoreError::Filter(format!("privacy registry error: {e}")))?;
        masked.replace_range(finding.start..finding.end, &mask_id);
        mapping.insert(mask_id, plaintext.to_string());
    }
    Ok((masked, mapping))
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
