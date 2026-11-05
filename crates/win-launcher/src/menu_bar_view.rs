//! menu_bar_view
//! macOS Top Menu Bar state machine, active application tracking, and dropdown menu model.

use mac_physics_core::menu_bar::{
    compute_menu_bar_layout, MenuBarGroup, MenuBarItem, MenuBarLayout, Rect,
    SystemStatusSnapshot,
};

/// High-level actions triggered by the Menu Bar and dropdown items.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuBarAction {
    AboutThisMac,
    SystemSettings,
    Sleep,
    Restart,
    ShutDown,
    LockScreen,
    LogOut,
    AppMenuAction { action_id: String },
    OpenSpotlight,
    ToggleControlCenter,
    StatusAction { item_id: String },
}

/// An individual row inside a dropdown menu.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuDropdownItem {
    pub id: String,
    pub title: String,
    pub shortcut: Option<String>,
    pub action: Option<MenuBarAction>,
    pub is_separator: bool,
    pub is_enabled: bool,
}

impl MenuDropdownItem {
    pub fn action(
        id: impl Into<String>,
        title: impl Into<String>,
        shortcut: Option<&str>,
        action: MenuBarAction,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: shortcut.map(String::from),
            action: Some(action),
            is_separator: false,
            is_enabled: true,
        }
    }

    pub fn disabled(
        id: impl Into<String>,
        title: impl Into<String>,
        shortcut: Option<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: shortcut.map(String::from),
            action: None,
            is_separator: false,
            is_enabled: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: "separator".to_string(),
            title: String::new(),
            shortcut: None,
            action: None,
            is_separator: true,
            is_enabled: false,
        }
    }
}

/// A floating squircle dropdown menu attached under an active menu bar item.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuDropdown {
    pub menu_id: String,
    pub origin_x: f32,
    pub origin_y: f32,
    pub width: f32,
    pub item_height: f32,
    pub items: Vec<MenuDropdownItem>,
    pub hovered_index: Option<usize>,
}

impl MenuDropdown {
    pub fn total_height(&self) -> f32 {
        self.items.len() as f32 * self.item_height + 10.0 // 5px top/bottom padding
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(self.origin_x, self.origin_y, self.width, self.total_height())
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.bounds().contains(x, y)
    }

    pub fn item_at_pos(&self, _x: f32, y: f32) -> Option<usize> {
        let rel_y = y - (self.origin_y + 5.0);
        if rel_y < 0.0 {
            return None;
        }
        let index = (rel_y / self.item_height) as usize;
        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }
}

/// State machine coordinating top menu bar items, active app title, and dropdown menus.
#[derive(Debug, Clone)]
pub struct MenuBarView {
    active_app_title: String,
    items: Vec<MenuBarItem>,
    status: SystemStatusSnapshot,
    active_dropdown: Option<MenuDropdown>,
    screen_width: f32,
}

impl Default for MenuBarView {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuBarView {
    /// Creates a new MenuBarView with standard macOS default menus and Finder as initial app.
    pub fn new() -> Self {
        let mut view = Self {
            active_app_title: "Finder".to_string(),
            items: Vec::new(),
            status: SystemStatusSnapshot::new(Some(100), true, true, 80, "12:00 PM"),
            active_dropdown: None,
            screen_width: 1920.0,
        };
        view.rebuild_items();
        view
    }

    /// Sets the assumed screen width for layout calculations.
    pub fn set_screen_width(&mut self, width: f32) {
        self.screen_width = width.max(320.0);
    }

    /// Returns the currently active focused application title.
    pub fn active_app_title(&self) -> &str {
        &self.active_app_title
    }

    /// Updates the active application title and refreshes the left menus.
    pub fn update_active_app(&mut self, app_name: &str) {
        let trimmed = app_name.trim();
        let normalized = if trimmed.is_empty() { "Finder" } else { trimmed };
        if self.active_app_title != normalized {
            self.active_app_title = normalized.to_string();
            self.close_dropdown();
            self.rebuild_items();
        }
    }

    /// Updates the hardware and OS system status indicators.
    pub fn update_status(&mut self, status: SystemStatusSnapshot) {
        self.status = status;
    }

    /// Returns the cached system status snapshot.
    pub fn status(&self) -> &SystemStatusSnapshot {
        &self.status
    }

    /// Returns the list of top-level menu bar items.
    pub fn items(&self) -> &[MenuBarItem] {
        &self.items
    }

    /// Computes the horizontal bounding boxes for all menu bar items.
    pub fn layout(&self) -> MenuBarLayout {
        compute_menu_bar_layout(&self.items, &self.status, self.screen_width)
    }

    /// Returns whether any dropdown menu is currently opened.
    pub fn is_dropdown_open(&self) -> bool {
        self.active_dropdown.is_some()
    }

    /// Returns the currently open dropdown menu, if any.
    pub fn active_dropdown(&self) -> Option<&MenuDropdown> {
        self.active_dropdown.as_ref()
    }

    /// Opens the dropdown menu for the given menu item ID.
    pub fn open_dropdown(&mut self, item_id: &str) {
        let layout = self.layout();
        let target = layout.left_items.iter().find(|item| item.id == item_id);
        let origin_x = target.map(|t| t.bounds.x).unwrap_or(12.0);

        let dropdown = match item_id {
            "apple" => self.build_apple_dropdown(origin_x),
            "app_root" => self.build_app_root_dropdown(origin_x),
            "file" => self.build_file_dropdown(origin_x),
            "edit" => self.build_edit_dropdown(origin_x),
            "view" => self.build_view_dropdown(origin_x),
            "window" => self.build_window_dropdown(origin_x),
            "help" => self.build_help_dropdown(origin_x),
            _ => None,
        };

        self.active_dropdown = dropdown;
    }

    /// Closes any currently active dropdown menu.
    pub fn close_dropdown(&mut self) {
        self.active_dropdown = None;
    }

    /// Handles mouse movement for hover highlighting and menu switching.
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        // If dropdown is open:
        if let Some(dropdown) = &mut self.active_dropdown {
            // Check if mouse moved into another top bar item to switch dropdowns
            if y <= 28.0 {
                let layout = compute_menu_bar_layout(&self.items, &self.status, self.screen_width);
                if let Some(item) = layout.hit_test(x, y) {
                    if item.group == MenuBarGroup::Left && item.has_dropdown && item.id != dropdown.menu_id {
                        let id = item.id.clone();
                        self.open_dropdown(&id);
                        return;
                    }
                }
            }

            // Check hover over dropdown rows
            if dropdown.contains(x, y) {
                if let Some(idx) = dropdown.item_at_pos(x, y) {
                    if let Some(item) = dropdown.items.get(idx) {
                        if !item.is_separator && item.is_enabled {
                            dropdown.hovered_index = Some(idx);
                            return;
                        }
                    }
                }
            }
            dropdown.hovered_index = None;
        }
    }

    /// Handles mouse clicks for opening dropdowns, selecting items, or dismissing.
    pub fn on_mouse_click(&mut self, x: f32, y: f32) -> Option<MenuBarAction> {
        // 1. Check if click is inside an active dropdown
        if let Some(dropdown) = &self.active_dropdown {
            if dropdown.contains(x, y) {
                if let Some(idx) = dropdown.item_at_pos(x, y) {
                    if let Some(item) = dropdown.items.get(idx) {
                        if !item.is_separator && item.is_enabled {
                            let action = item.action.clone();
                            self.close_dropdown();
                            return action;
                        }
                    }
                }
                return None;
            }
        }

        // 2. Check if click is on the top Menu Bar (y <= 28.0)
        if y <= 28.0 {
            let layout = self.layout();
            if let Some(item) = layout.hit_test(x, y) {
                match item.group {
                    MenuBarGroup::Left => {
                        if item.has_dropdown {
                            if let Some(dropdown) = &self.active_dropdown {
                                if dropdown.menu_id == item.id {
                                    self.close_dropdown();
                                    return None;
                                }
                            }
                            self.open_dropdown(&item.id);
                            return None;
                        }
                    }
                    MenuBarGroup::Right => {
                        self.close_dropdown();
                        return match item.id.as_str() {
                            "spotlight" => Some(MenuBarAction::OpenSpotlight),
                            "control_center" => Some(MenuBarAction::ToggleControlCenter),
                            other => Some(MenuBarAction::StatusAction {
                                item_id: other.to_string(),
                            }),
                        };
                    }
                }
            }
        }

        // 3. Click outside closes dropdown
        self.close_dropdown();
        None
    }

    fn rebuild_items(&mut self) {
        self.items = vec![
            MenuBarItem::apple_logo(),
            MenuBarItem::new("app_root", &self.active_app_title, MenuBarGroup::Left, true, true),
            MenuBarItem::new("file", "File", MenuBarGroup::Left, false, true),
            MenuBarItem::new("edit", "Edit", MenuBarGroup::Left, false, true),
            MenuBarItem::new("view", "View", MenuBarGroup::Left, false, true),
            MenuBarItem::new("window", "Window", MenuBarGroup::Left, false, true),
            MenuBarItem::new("help", "Help", MenuBarGroup::Left, false, true),
        ];
    }

    fn build_apple_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let items = vec![
            MenuDropdownItem::action("about_mac", "About This Mac", None, MenuBarAction::AboutThisMac),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("sys_settings", "System Settings...", Some("Cmd+,"), MenuBarAction::SystemSettings),
            MenuDropdownItem::action("app_store", "App Store...", None, MenuBarAction::AppMenuAction { action_id: "app_store".into() }),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("sleep", "Sleep", None, MenuBarAction::Sleep),
            MenuDropdownItem::action("restart", "Restart...", None, MenuBarAction::Restart),
            MenuDropdownItem::action("shutdown", "Shut Down...", None, MenuBarAction::ShutDown),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("lock", "Lock Screen", Some("Ctrl+Cmd+Q"), MenuBarAction::LockScreen),
            MenuDropdownItem::action("logout", "Log Out...", Some("Shift+Cmd+Q"), MenuBarAction::LogOut),
        ];

        Some(MenuDropdown {
            menu_id: "apple".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 220.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_app_root_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let app = &self.active_app_title;
        let items = vec![
            MenuDropdownItem::action(
                "about_app",
                format!("About {app}"),
                None,
                MenuBarAction::AppMenuAction { action_id: "about_app".into() },
            ),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action(
                "settings",
                "Settings...",
                Some("Cmd+,"),
                MenuBarAction::AppMenuAction { action_id: "settings".into() },
            ),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action(
                "hide_app",
                format!("Hide {app}"),
                Some("Cmd+H"),
                MenuBarAction::AppMenuAction { action_id: "hide_app".into() },
            ),
            MenuDropdownItem::action(
                "hide_others",
                "Hide Others",
                Some("Opt+Cmd+H"),
                MenuBarAction::AppMenuAction { action_id: "hide_others".into() },
            ),
            MenuDropdownItem::action(
                "show_all",
                "Show All",
                None,
                MenuBarAction::AppMenuAction { action_id: "show_all".into() },
            ),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action(
                "quit_app",
                format!("Quit {app}"),
                Some("Cmd+Q"),
                MenuBarAction::AppMenuAction { action_id: "quit_app".into() },
            ),
        ];

        let max_len = items.iter().map(|it| it.title.len()).max().unwrap_or(20);
        let width = ((max_len as f32 * 7.5) + 60.0).max(230.0);

        Some(MenuDropdown {
            menu_id: "app_root".to_string(),
            origin_x,
            origin_y: 28.0,
            width,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_file_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let items = vec![
            MenuDropdownItem::action("new_window", "New Window", Some("Cmd+N"), MenuBarAction::AppMenuAction { action_id: "new_window".into() }),
            MenuDropdownItem::action("new_tab", "New Tab", Some("Cmd+T"), MenuBarAction::AppMenuAction { action_id: "new_tab".into() }),
            MenuDropdownItem::action("open", "Open...", Some("Cmd+O"), MenuBarAction::AppMenuAction { action_id: "open".into() }),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("close_window", "Close Window", Some("Cmd+W"), MenuBarAction::AppMenuAction { action_id: "close_window".into() }),
        ];
        Some(MenuDropdown {
            menu_id: "file".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 200.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_edit_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let items = vec![
            MenuDropdownItem::action("undo", "Undo", Some("Cmd+Z"), MenuBarAction::AppMenuAction { action_id: "undo".into() }),
            MenuDropdownItem::action("redo", "Redo", Some("Shift+Cmd+Z"), MenuBarAction::AppMenuAction { action_id: "redo".into() }),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("cut", "Cut", Some("Cmd+X"), MenuBarAction::AppMenuAction { action_id: "cut".into() }),
            MenuDropdownItem::action("copy", "Copy", Some("Cmd+C"), MenuBarAction::AppMenuAction { action_id: "copy".into() }),
            MenuDropdownItem::action("paste", "Paste", Some("Cmd+V"), MenuBarAction::AppMenuAction { action_id: "paste".into() }),
            MenuDropdownItem::action("select_all", "Select All", Some("Cmd+A"), MenuBarAction::AppMenuAction { action_id: "select_all".into() }),
        ];
        Some(MenuDropdown {
            menu_id: "edit".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 190.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_view_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let items = vec![
            MenuDropdownItem::action("reload", "Reload", Some("Cmd+R"), MenuBarAction::AppMenuAction { action_id: "reload".into() }),
            MenuDropdownItem::action("toggle_fullscreen", "Toggle Full Screen", Some("Ctrl+Cmd+F"), MenuBarAction::AppMenuAction { action_id: "fullscreen".into() }),
        ];
        Some(MenuDropdown {
            menu_id: "view".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 210.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_window_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let items = vec![
            MenuDropdownItem::action("minimize", "Minimize", Some("Cmd+M"), MenuBarAction::AppMenuAction { action_id: "minimize".into() }),
            MenuDropdownItem::action("zoom", "Zoom", None, MenuBarAction::AppMenuAction { action_id: "zoom".into() }),
            MenuDropdownItem::separator(),
            MenuDropdownItem::action("bring_all_to_front", "Bring All to Front", None, MenuBarAction::AppMenuAction { action_id: "bring_front".into() }),
        ];
        Some(MenuDropdown {
            menu_id: "window".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 200.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }

    fn build_help_dropdown(&self, origin_x: f32) -> Option<MenuDropdown> {
        let app = &self.active_app_title;
        let items = vec![
            MenuDropdownItem::action(
                "app_help",
                format!("{app} Help"),
                None,
                MenuBarAction::AppMenuAction { action_id: "app_help".into() },
            ),
        ];
        Some(MenuDropdown {
            menu_id: "help".to_string(),
            origin_x,
            origin_y: 28.0,
            width: 200.0,
            item_height: 24.0,
            items,
            hovered_index: None,
        })
    }
}
