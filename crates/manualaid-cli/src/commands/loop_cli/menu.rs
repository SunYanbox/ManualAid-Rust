//! Menu model for the interactive loop: string keys, aliases, nested
//! submenus and automatic numeric indexing.
//! 交互式 loop 的菜单模型：字符串键、别名、嵌套子菜单与自动数字索引。

use std::collections::HashMap;

use super::command::LoopCommand;

/// The action attached to a menu item.
/// 菜单项绑定的动作。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum MenuAction {
    /// Run a loop command.
    /// 执行一个 loop 命令。
    Command(LoopCommand),
    /// Enter a nested menu. Kept for future multi-level menus even though
    /// current menus enter submenus through explicit commands.
    /// 进入嵌套子菜单。当前菜单通过显式命令进入子菜单，但保留该变体
    /// 以支持未来的多级菜单。
    #[allow(dead_code)]
    Submenu(Menu),
}

/// A single menu item. Its primary key and aliases are all matched as
/// exact, case-sensitive strings within the owning menu.
/// 单个菜单项。其主键与全部别名都在所属菜单内按精确、区分大小写的
/// 字符串进行匹配。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct MenuItem {
    key: String,
    aliases: Vec<String>,
    label: String,
    action: MenuAction,
}

impl MenuItem {
    /// Build an item that receives the next automatic numeric key.
    /// 构建使用下一个自动数字键的菜单项。
    pub(super) fn auto(label: String, action: MenuAction) -> Self {
        Self {
            key: String::new(),
            aliases: Vec::new(),
            label,
            action,
        }
    }

    /// Build an item with an explicit primary key and no aliases. Kept for
    /// future menus; current menus use `keyed_alias`.
    /// 构建显式主键且无别名的菜单项。当前菜单使用 `keyed_alias`，该
    /// 构造函数保留给未来的菜单。
    #[allow(dead_code)]
    pub(super) fn keyed(key: &str, label: String, action: MenuAction) -> Self {
        Self {
            key: key.to_string(),
            aliases: Vec::new(),
            label,
            action,
        }
    }

    /// Build an item with an explicit primary key and aliases.
    /// 构建显式主键并带别名的菜单项。
    pub(super) fn keyed_alias(
        key: &str,
        aliases: &[&str],
        label: String,
        action: MenuAction,
    ) -> Self {
        Self {
            key: key.to_string(),
            aliases: aliases.iter().map(|alias| (*alias).to_string()).collect(),
            label,
            action,
        }
    }
}

/// Error returned while registering a menu item.
/// 注册菜单项时返回的错误。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum MenuError {
    /// A primary key or alias is already registered in this menu.
    /// 主键或别名已在此菜单中注册。
    DuplicateKey(String),
}

/// A menu whose items are resolved by exact, case-sensitive string keys.
/// 通过精确且区分大小写的字符串键解析菜单项。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Menu {
    title: String,
    items: Vec<MenuItem>,
    key_index: HashMap<String, usize>,
    next_auto: usize,
}

impl Menu {
    pub(super) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
            key_index: HashMap::new(),
            next_auto: 1,
        }
    }

    /// Register one item. A `None` key uses the next free automatic
    /// numeric key; explicit keys and aliases are checked for duplicates
    /// (case-sensitively) within this menu.
    /// 注册一个菜单项。自动项使用下一个可用的数字键；显式主键与别名
    /// 在本菜单内做区分大小写的重复校验。
    pub(super) fn add(mut self, item: MenuItem) -> Result<Self, MenuError> {
        let primary_key = if item.key.is_empty() {
            let mut key = self.next_auto.to_string();
            while self.key_index.contains_key(&key) {
                self.next_auto += 1;
                key = self.next_auto.to_string();
            }
            self.next_auto += 1;
            key
        } else {
            let key = item.key.clone();
            // Keep automatic numbering clear of explicitly registered
            // numeric keys so a later `auto` item never collides.
            // 自动编号避开显式注册的数字键，避免后续自动项冲突。
            if let Ok(value) = key.parse::<usize>()
                && value >= self.next_auto
            {
                self.next_auto = value + 1;
            }
            key
        };

        let item_index = self.items.len();
        self.insert_key(&primary_key, item_index)?;
        for alias in &item.aliases {
            self.insert_key(alias, item_index)?;
        }
        self.items.push(MenuItem {
            key: primary_key,
            aliases: item.aliases,
            label: item.label,
            action: item.action,
        });
        Ok(self)
    }

    fn insert_key(&mut self, key: &str, item_index: usize) -> Result<(), MenuError> {
        if self.key_index.insert(key.to_string(), item_index).is_some() {
            return Err(MenuError::DuplicateKey(key.to_string()));
        }
        Ok(())
    }

    /// Render the menu text with `"{key}. {label}"` lines.
    /// 渲染菜单文本，每项格式为 `"{key}. {label}"`。
    pub(super) fn render(&self) -> String {
        let mut lines = vec![crate::style::header(&self.title)];
        for item in &self.items {
            lines.push(format!("{}. {}", item.key, item.label));
        }
        lines.join("\n") + "\n"
    }

    /// Resolve trimmed user input to the matching action. Matching is
    /// exact and case-sensitive; aliases resolve to the same action.
    /// 将去除首尾空白后的用户输入解析为对应动作。匹配精确且区分大小写；
    /// 别名指向同一动作。
    pub(super) fn resolve(&self, input: &str) -> Option<&MenuAction> {
        let item_index = self.key_index.get(input)?;
        self.items.get(*item_index).map(|item| &item.action)
    }
}

/// Build the main loop menu.
/// 构建主循环菜单。
pub(super) fn build_main_menu() -> Menu {
    Menu::new(i18n::t_str("cli.loop.menu_title"))
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_generate"),
            MenuAction::Command(LoopCommand::GeneratePrompt),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_paste"),
            MenuAction::Command(LoopCommand::PasteAndSubmit),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_input"),
            MenuAction::Command(LoopCommand::InputAndSubmit),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_copy"),
            MenuAction::Command(LoopCommand::CopyRoundResult),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_config"),
            MenuAction::Command(LoopCommand::ConfigMenu),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_summary"),
            MenuAction::Command(LoopCommand::SessionSummary),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_history"),
            MenuAction::Command(LoopCommand::ToolHistory),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.loop.menu_copy_prompt"),
            MenuAction::Command(LoopCommand::CopyPromptMenu),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.loop.menu_exit"),
            MenuAction::Command(LoopCommand::Exit),
        ))
        .expect("unique menu key")
}

/// Render the main menu text.
/// 渲染主菜单文本。
pub(super) fn render_main_menu() -> String {
    build_main_menu().render()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_marker() -> LoopCommand {
        LoopCommand::SessionSummary
    }

    #[test]
    fn auto_keys_start_at_one_and_resolve() {
        let menu = Menu::new("m")
            .add(MenuItem::auto(
                "first".to_string(),
                MenuAction::Command(command_marker()),
            ))
            .unwrap()
            .add(MenuItem::auto(
                "second".to_string(),
                MenuAction::Command(LoopCommand::ToolHistory),
            ))
            .unwrap();
        let rendered = menu.render();
        assert!(rendered.contains("1. first"));
        assert!(rendered.contains("2. second"));
        assert!(menu.resolve("1").is_some());
        assert!(menu.resolve("2").is_some());
        assert!(menu.resolve("3").is_none());
    }

    #[test]
    fn explicit_key_and_aliases_resolve_to_same_action() {
        let menu = Menu::new("m")
            .add(MenuItem::keyed_alias(
                "0",
                &["q", "quit", "exit"],
                "back".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap();
        assert!(matches!(
            menu.resolve("0"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(matches!(
            menu.resolve("q"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(matches!(
            menu.resolve("quit"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(matches!(
            menu.resolve("exit"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(menu.resolve("Q").is_none());
    }

    #[test]
    fn duplicate_primary_key_is_rejected() {
        let result = Menu::new("m")
            .add(MenuItem::keyed(
                "a",
                "one".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap()
            .add(MenuItem::keyed(
                "a",
                "two".to_string(),
                MenuAction::Command(LoopCommand::SessionSummary),
            ));
        assert_eq!(result, Err(MenuError::DuplicateKey("a".to_string())));
    }

    #[test]
    fn duplicate_alias_is_rejected() {
        let result = Menu::new("m")
            .add(MenuItem::keyed(
                "a",
                "one".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap()
            .add(MenuItem::keyed_alias(
                "b",
                &["a"],
                "two".to_string(),
                MenuAction::Command(LoopCommand::SessionSummary),
            ));
        assert_eq!(result, Err(MenuError::DuplicateKey("a".to_string())));
    }

    #[test]
    fn auto_key_skips_explicit_numeric_key() {
        let menu = Menu::new("m")
            .add(MenuItem::keyed(
                "1",
                "fixed".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap()
            .add(MenuItem::auto(
                "automatic".to_string(),
                MenuAction::Command(LoopCommand::SessionSummary),
            ))
            .unwrap();
        assert!(menu.resolve("1").is_some());
        assert!(menu.resolve("2").is_some());
        assert!(!menu.render().contains("1. automatic"));
    }

    #[test]
    fn nested_menu_is_reachable_but_has_independent_keys() {
        let submenu = Menu::new("sub")
            .add(MenuItem::keyed(
                "0",
                "sub back".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap();
        let menu = Menu::new("m")
            .add(MenuItem::keyed(
                "s",
                "open sub".to_string(),
                MenuAction::Submenu(submenu),
            ))
            .unwrap()
            .add(MenuItem::keyed(
                "0",
                "main back".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap();
        assert!(matches!(menu.resolve("s"), Some(MenuAction::Submenu(_))));
        assert!(matches!(
            menu.resolve("0"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        let MenuAction::Submenu(submenu) = menu.resolve("s").unwrap() else {
            panic!("expected submenu");
        };
        assert!(matches!(
            submenu.resolve("0"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(submenu.resolve("0").is_some());
    }

    #[test]
    fn case_sensitive_keys_are_distinct() {
        let menu = Menu::new("m")
            .add(MenuItem::keyed(
                "q",
                "lower".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap()
            .add(MenuItem::keyed(
                "Q",
                "upper".to_string(),
                MenuAction::Command(LoopCommand::SessionSummary),
            ))
            .unwrap();
        assert!(matches!(
            menu.resolve("q"),
            Some(MenuAction::Command(LoopCommand::Back))
        ));
        assert!(matches!(
            menu.resolve("Q"),
            Some(MenuAction::Command(LoopCommand::SessionSummary))
        ));
    }

    #[test]
    fn explicit_key_less_than_next_auto_does_not_update_next_auto() {
        // 覆盖 menu.rs:117-119
        let mut menu = Menu::new("m");
        // 添加一个 auto 项，next_auto 变成 2
        menu = menu
            .add(MenuItem::auto(
                "first".to_string(),
                MenuAction::Command(command_marker()),
            ))
            .unwrap();
        // next_auto = 2, 添加显式键 "0" (< next_auto)，"0" 未被占用
        menu = menu
            .add(MenuItem::keyed(
                "0",
                "zero key".to_string(),
                MenuAction::Command(LoopCommand::Back),
            ))
            .unwrap();
        // 再添加一个 auto 项，应该继续用 key "2"（因为 next_auto 没有被更新）
        menu = menu
            .add(MenuItem::auto(
                "second".to_string(),
                MenuAction::Command(LoopCommand::SessionSummary),
            ))
            .unwrap();
        let rendered = menu.render();
        assert!(rendered.contains("1. first"));
        assert!(rendered.contains("0. zero key"));
        assert!(rendered.contains("2. second"));
    }

    #[test]
    fn resolve_unknown_input_returns_none() {
        // 覆盖 menu.rs:375 附近 resolve 返回 None 的情况
        let menu = build_main_menu();
        assert!(menu.resolve("bogus_input").is_none());
        assert!(menu.resolve("").is_none());
        assert!(menu.resolve("999").is_none());
    }
}
