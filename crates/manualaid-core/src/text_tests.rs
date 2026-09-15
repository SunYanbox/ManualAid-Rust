use super::*;

// An unknown key is returned verbatim by the translation backend, so these
// cases exercise substitution without depending on any translation being
// loaded for the active locale.
// 未知键会被翻译后端原样返回，因此这些用例无需依赖当前 locale 已加载任何
// 译文即可验证替换行为。

#[test]
fn replaces_provided_placeholders() {
    let out = t_fmt("%{a} and %{b}", &[("a", "1"), ("b", "2")]);
    assert_eq!(out, "1 and 2");
}

#[test]
fn leaves_unknown_placeholders_untouched() {
    let out = t_fmt("%{a} and %{b}", &[("a", "1")]);
    assert_eq!(out, "1 and %{b}");
}

#[test]
fn does_not_touch_longer_placeholder_names() {
    let out = t_fmt("%{unique_name}", &[("name", "n")]);
    assert_eq!(out, "%{unique_name}");
}

#[test]
fn substitutes_every_occurrence() {
    let out = t_fmt("%{a}-%{a}", &[("a", "x")]);
    assert_eq!(out, "x-x");
}
