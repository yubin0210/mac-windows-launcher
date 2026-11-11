//! Production grouping shares Dock identities; focus comes only from observed windows.
use crate::{app_catalog::{AppCatalog,AppRecord},AppInfo,display::WorkArea};
#[derive(Debug,Clone,PartialEq)]
pub struct ShelfGroup { pub id:String,pub title:String,pub windows:Vec<AppInfo> }
#[derive(Debug,Clone,PartialEq,Default)]
pub struct ShelfModel { pub groups:Vec<ShelfGroup>,pub active:Option<String> }
impl ShelfModel {
    pub fn new(catalog:&AppCatalog,windows:&[AppInfo])->Self {
        let mut model=Self::default();
        for window in windows.iter().filter(|window|window.hwnd!=0 && window.pid!=std::process::id()) {
            let record=catalog.apps().iter().find(|app|app.matches(window)).cloned().unwrap_or_else(|| {
                let title=window.process_name.rsplit(['\\','/']).next().unwrap_or(&window.process_name).trim_end_matches(".exe");
                let mut record=AppRecord::new(&window.process_name,title,Some(window.process_name.clone()));
                if let Some(id)=&window.app_id { record=record.with_app_id(id.clone()); } record
            });
            if window.is_focused { model.active=Some(record.id.clone()); }
            if let Some(group)=model.groups.iter_mut().find(|group|group.id==record.id) { group.windows.push(window.clone()); }
            else { model.groups.push(ShelfGroup{id:record.id,title:record.title,windows:vec![window.clone()]}); }
        }
        model.groups.sort_by(|a,b|a.title.to_lowercase().cmp(&b.title.to_lowercase()).then(a.id.cmp(&b.id)));
        for group in &mut model.groups { group.windows.sort_by_key(|window|(!window.is_focused,window.hwnd)); }
        model
    }
    pub fn page(&self,page:usize,capacity:usize)->Vec<ShelfGroup> {
        let groups:Vec<_>=self.groups.iter().filter(|group|Some(&group.id)!=self.active.as_ref()).collect();
        let capacity=capacity.max(1); let pages=groups.len().div_ceil(capacity).max(1);
        groups.into_iter().skip(page%pages*capacity).take(capacity).cloned().collect()
    }
    pub fn pages(&self,capacity:usize)->usize { self.groups.iter().filter(|group|Some(&group.id)!=self.active.as_ref()).count().div_ceil(capacity.max(1)).max(1) }
}
#[derive(Debug,Clone,Copy,PartialEq)]
pub struct ShelfGeometry { pub x:i32,pub y:i32,pub width:i32,pub height:i32,pub capacity:usize,pub scale:f32,pub card_height:f32 }
impl ShelfGeometry {
    pub fn fit(area:WorkArea,dpi:u32,count:usize)->Result<Self,String> {
        let w=area.right-area.left; let h=area.bottom-area.top;
        if w<240 || h<200 || !(48..=768).contains(&dpi) { return Err("No usable Stage shelf work area".into()); }
        let scale=(dpi as f32/96.0).min(w as f32/320.0).min((h-16) as f32/220.0);
        let card_height=132.0*scale;
        let capacity=(((h-16) as f32-56.0*scale)/card_height).floor().max(1.0) as usize;
        let rows=count.min(capacity).max(1);
        let height=(56.0*scale+rows as f32*card_height).ceil() as i32;
        Ok(Self{x:area.left,y:area.top+(h-height)/2,width:(200.0*scale).ceil() as i32,height,capacity,scale,card_height})
    }
    pub fn visible_bounds(&self,t:f32)->(i32,i32,i32) {
        let t=if t.is_finite(){t.clamp(0.0,1.0)}else{0.0};
        let strip=(6.0*self.scale).round().max(3.0) as i32;
        let width=(strip as f32+(self.width-strip) as f32*t).round() as i32;
        let height=((48.0*self.scale)+(self.height as f32-48.0*self.scale)*t).round() as i32;
        (width,height,self.y+(self.height-height)/2)
    }
}
