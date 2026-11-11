//! Search session: native input/events, shared app index, async icons and a real composition surface.
use crate::{app_catalog::AppCatalog, directx::DirectCompositionPipeline, icon_cache::native::IconLoader,
    indexer::AppIndexer, settings::SpotlightHotkey, spotlight_native::NativeSpotlightWindow,
    spotlight_render::Direct2DSpotlightRenderer, spotlight_view::SpotlightView, spotlight_window::{SpotlightEvent, SpotlightWindowConfig}};
use mac_physics_core::spotlight::SpotlightItem;
use windows::Win32::Foundation::HWND;

// Fields drop in declaration order: the GPU resources must precede their native HWND.
pub struct SpotlightController {
    renderer: Direct2DSpotlightRenderer, pipeline: DirectCompositionPipeline,
    icons: IconLoader, view: SpotlightView, dirty: bool, window: NativeSpotlightWindow,
}
impl SpotlightController {
    pub fn new(shared: &DirectCompositionPipeline, catalog: &AppCatalog) -> Result<Self, String> {
        let window = NativeSpotlightWindow::new(SpotlightWindowConfig::default())?;
        // Put the visual below the EDIT child, sharing the Dock's GPU device.
        let pipeline = shared.for_window(window.hwnd(), false).map_err(|e| e.to_string())?;
        let renderer = Direct2DSpotlightRenderer::with_pipeline(&pipeline, window.width(), window.height()).map_err(|e| e.to_string())?;
        Ok(Self { renderer, pipeline, icons: IconLoader::new(), view: SpotlightView::new(AppIndexer::from_catalog(catalog)), dirty: true, window })
    }
    pub fn hotkey(&self) -> Option<SpotlightHotkey> { self.window.hotkey() }
    pub fn clear_hotkey(&mut self) { self.window.clear_hotkey(); }
    pub fn set_hotkey(&mut self, key: SpotlightHotkey) -> Result<(), String> { self.window.register_hotkey(key) }
    pub fn show(&mut self) { self.window.show(); }
    pub fn hide(&mut self) { self.window.hide(); self.view.clear(); self.dirty = true; }
    pub fn dismiss(&mut self) { self.window.dismiss_after_execution(); self.view.clear(); self.dirty = true; }
    pub fn hwnd(&self) -> HWND { self.window.hwnd() }
    pub fn frame(&mut self, catalog: &AppCatalog) -> Result<Option<SpotlightItem>, String> {
        let mut execute = None;
        for event in self.window.poll_events() {
            match event {
                SpotlightEvent::Opened => {
                    self.view.replace_index(AppIndexer::from_catalog(catalog));
                    self.view.set_limit(self.window.geometry().capacity);
                    self.dirty = true;
                }
                SpotlightEvent::QueryChanged(query) => { self.view.set_query(&query); self.dirty = true; }
                SpotlightEvent::Navigate(delta) => { self.view.navigate(delta); self.dirty = true; }
                SpotlightEvent::Execute => { execute = self.view.selected_item().cloned(); }
                SpotlightEvent::ClickResult(id) => {
                    if self.view.select_id(&id) {
                        execute = self.view.selected_item().cloned();
                        self.dirty = true;
                    }
                }
                SpotlightEvent::Closed | SpotlightEvent::Escaped => {
                    execute = None; self.view.clear(); self.dirty = true;
                }
                SpotlightEvent::Paint => self.dirty = true,
                SpotlightEvent::DisplayChanged => { self.view.set_limit(self.window.geometry().capacity); self.dirty = true; }
                SpotlightEvent::Error(error) => return Err(error),
            }
        }
        if !self.window.is_visible() { return Ok(None); }
        let sources: Vec<_> = self.view.get_results().iter().filter_map(|item| catalog.get(&item.id))
            .map(|app| (app.id.clone(), app.target.clone())).collect();
        self.icons.retain(&sources);
        self.renderer.retain_icons(&sources.iter().map(|(id,_)| id.clone()).collect::<Vec<_>>());
        for (id, target) in &sources { self.icons.request(id, target); }
        for reply in self.icons.drain() {
            if !sources.iter().any(|(id, target)| id == &reply.id && target == &reply.source) { continue; }
            match reply.pixels {
                Ok(pixels) => { self.renderer.set_icon(&reply.id, &pixels).map_err(|e| e.to_string())?; self.dirty = true; }
                Err(error) => crate::diagnostics::write(&format!("Spotlight icon unavailable: {}: {error}", reply.source)),
            }
        }
        if self.dirty {
            self.window.set_results(self.view.results_count(), !self.view.get_query().trim().is_empty())?;
            let geometry = self.window.geometry();
            self.renderer.resize(&self.pipeline, geometry.width as u32, geometry.height as u32).map_err(|e| e.to_string())?;
            self.renderer.draw(self.view.get_query(), self.view.get_results(), self.view.selected_index(), geometry).map_err(|e| e.to_string())?;
            self.pipeline.commit().map_err(|e| e.to_string())?;
            self.window.set_row_ids(self.view.get_results().iter().take(geometry.visible_rows).map(|item| item.id.clone()).collect());
            self.dirty = false;
        }
        Ok(execute)
    }
}
