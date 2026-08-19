use super::*;

fn fm(content: &str) -> Frontmatter {
    parse_frontmatter(content).expect("parse should succeed")
}

#[test]
fn parse_frontmatter_single_line_fields() {
    let frontmatter =
        fm("---\nname: greeter\ndescription: A greeting skill\n---\n## Usage\nHello\n");
    assert_eq!(frontmatter.name.as_deref(), Some("greeter"));
    assert_eq!(frontmatter.description, "A greeting skill");
    assert_eq!(frontmatter.body, "## Usage\nHello\n");
}

#[test]
fn parse_frontmatter_folded_block() {
    let frontmatter = fm("---\nname: a\ndescription: >\n  first line\n  second line\n---\nbody");
    assert_eq!(frontmatter.description, "first line second line");
}

#[test]
fn parse_frontmatter_folded_block_blank_line() {
    let frontmatter = fm("---\ndescription: >\n  first\n\n  second\n---\n");
    assert_eq!(frontmatter.description, "first\nsecond");
}

#[test]
fn parse_frontmatter_literal_block() {
    let frontmatter = fm("---\ndescription: |\n  line one\n  line two\n---\n");
    assert_eq!(frontmatter.description, "line one\nline two");
}

#[test]
fn parse_frontmatter_literal_block_blank_line() {
    let frontmatter = fm("---\ndescription: |\n  line one\n\n  line two\n---\n");
    assert_eq!(frontmatter.description, "line one\n\nline two");
}

#[test]
fn parse_frontmatter_block_followed_by_another_key() {
    let frontmatter = fm("---\ndescription: >\n  folded text\nname: after\n---\n");
    assert_eq!(frontmatter.description, "folded text");
    assert_eq!(frontmatter.name.as_deref(), Some("after"));
}

#[test]
fn parse_frontmatter_body_without_trailing_newline() {
    let frontmatter = fm("---\nname: a\ndescription: d\n---");
    assert_eq!(frontmatter.body, "");
}

#[test]
fn parse_frontmatter_indented_continuation() {
    let frontmatter = fm("---\ndescription:\n  first\n  second\n---\n");
    assert_eq!(frontmatter.description, "first second");
}

#[test]
fn parse_frontmatter_chomp_variants() {
    let folded = fm("---\ndescription: >-\n  a\n  b\n---\n");
    assert_eq!(folded.description, "a b");
    let literal = fm("---\ndescription: |+\n  a\n  b\n---\n");
    assert_eq!(literal.description, "a\nb");
}

#[test]
fn parse_frontmatter_no_frontmatter_returns_none() {
    assert!(parse_frontmatter("plain text").is_none());
}

#[test]
fn parse_frontmatter_unterminated_returns_none() {
    assert!(parse_frontmatter("---\nname: a\n").is_none());
}

#[test]
fn parse_frontmatter_missing_name_is_none_field() {
    let frontmatter = fm("---\ndescription: only desc\n---\nbody");
    assert_eq!(frontmatter.name, None);
    assert_eq!(frontmatter.description, "only desc");
}

#[test]
fn parse_frontmatter_empty_description_defaults_to_empty() {
    let frontmatter = fm("---\nname: a\n---\nbody");
    assert!(frontmatter.description.is_empty());
}

#[test]
fn parse_frontmatter_ignores_comments_lists_and_unknown_keys() {
    let frontmatter = fm("---\n# comment\n- list item\nname: a\nunknown: x\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
    assert_eq!(frontmatter.description, "d");
}

#[test]
fn parse_frontmatter_ignores_lines_without_colon() {
    let frontmatter = fm("---\nbare text line\nname: a\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
    assert_eq!(frontmatter.description, "d");
}

#[test]
fn parse_frontmatter_trimmed_leading_whitespace() {
    let frontmatter = fm("\n\n---\nname: a\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
}

#[test]
fn parse_frontmatter_quoted_values_not_unquoted() {
    let frontmatter = fm("---\ndescription: \"quoted\"\n---\n");
    assert_eq!(frontmatter.description, "\"quoted\"");
}
