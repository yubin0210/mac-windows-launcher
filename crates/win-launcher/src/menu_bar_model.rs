//! Production menu contents: every interactive item has an implemented command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction { Menu, Search, Network, Sound, DateTime, Power }
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BarStatus {
    pub clock: String, pub battery: Option<(u8, bool)>, pub volume: Option<u8>, pub muted: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BarModel { pub app_title: String, pub status: BarStatus }
pub struct BarItem { pub x: f32, pub width: f32, pub label: String, pub action: Option<BarAction> }
impl BarModel {
    pub fn layout(&self, width: f32, scale: f32) -> Vec<BarItem> {
        let left = (104.0*scale).min(width/4.0);
        let mut right = width;
        let mut tail = Vec::new();
        let add = |right: &mut f32, tail: &mut Vec<BarItem>, label: String, logical_width: f32, action: BarAction| {
            let size = (logical_width*scale).min((width-left)/2.0);
            *right -= size;
            tail.push(BarItem { x: *right, width: size, label, action: Some(action) });
        };
        add(&mut right, &mut tail, self.status.clock.clone(), 160.0, BarAction::DateTime);
        add(&mut right, &mut tail, "Search".into(), 64.0, BarAction::Search);
        // Keep the clock, search and app name readable on narrow or highly scaled screens.
        let mut remaining = right-left;
        if remaining >= 220.0*scale { add(&mut right, &mut tail, if self.status.muted == Some(true) { "Muted".into() } else { self.status.volume.map(|v| format!("Sound {v}%")).unwrap_or("Sound".into()) }, 96.0, BarAction::Sound); remaining -= 96.0*scale; }
        if remaining >= 200.0*scale { add(&mut right, &mut tail, "Network".into(), 80.0, BarAction::Network); remaining -= 80.0*scale; }
        if remaining >= 180.0*scale {
            if let Some((percent, charging)) = self.status.battery { add(&mut right, &mut tail, format!("{}{percent}%", if charging { "+" } else { "" }), 68.0, BarAction::Power); }
        }
        tail.reverse();
        let mut items = vec![BarItem { x: 0.0, width: left, label: "MacLauncher".into(), action: Some(BarAction::Menu) }];
        if right > left { items.push(BarItem { x: left, width: right-left, label: self.app_title.clone(), action: None }); }
        items.extend(tail);
        items
    }
    pub fn hit(&self, width: f32, scale: f32, x: f32, y: f32) -> Option<BarAction> {
        if y < 0.0 || y >= 28.0*scale { return None; }
        self.layout(width, scale).into_iter().find(|item| x >= item.x && x < item.x+item.width).and_then(|item| item.action)
    }
}
