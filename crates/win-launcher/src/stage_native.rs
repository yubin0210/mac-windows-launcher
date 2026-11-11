//! Optional production shelf. Its HWND clips both previews and input while retracting.
use crate::{AppInfo,AppTracker,ProcessLauncher,app_catalog::AppCatalog,stage_model::{ShelfGeometry,ShelfModel},directx::DirectCompositionPipeline};
use std::{cell::{Cell,RefCell},collections::HashMap,time::{Instant,Duration}};
use windows::core::w;
use windows::Win32::{Foundation::*,Graphics::{Gdi::*,Dwm::*,Direct2D::*,Direct2D::Common::*,DirectWrite::*,DirectComposition::IDCompositionSurface,Dxgi::Common::*},UI::WindowsAndMessaging::*};
#[derive(Clone)]
enum Event { Group(String,bool), Page(i32), Pin }
struct State { events:RefCell<Vec<Event>>,cards:RefCell<Vec<(String,RECT)>>,paint:Cell<bool>,scale:Cell<f32> }
unsafe extern "system" fn procedure(hwnd:HWND,msg:u32,wp:WPARAM,lp:LPARAM)->LRESULT {
    let pointer=GetWindowLongPtrW(hwnd,GWLP_USERDATA) as *const State;
    if pointer.is_null(){return DefWindowProcW(hwnd,msg,wp,lp);}
    let state=&*pointer;
    match msg {
        WM_PAINT=>{let _=ValidateRect(hwnd,None);state.paint.set(true);LRESULT(0)}
        WM_MOUSEWHEEL=>{let delta=(wp.0>>16) as i16;if delta!=0{state.events.borrow_mut().push(Event::Page(if delta<0{1}else{-1}));} LRESULT(0)}
        WM_LBUTTONUP|WM_RBUTTONUP=>{
            let x=(lp.0&0xFFFF) as i16 as i32;let y=((lp.0>>16)&0xFFFF) as i16 as i32;
            if y<(32.0*state.scale.get()) as i32 {state.events.borrow_mut().push(Event::Pin);}
            else {
                let id=state.cards.borrow().iter().find(|(_,r)|x>=r.left&&x<r.right&&y>=r.top&&y<r.bottom).map(|(id,_)|id.clone());
                if let Some(id)=id {state.events.borrow_mut().push(Event::Group(id,msg==WM_RBUTTONUP));}
            }
            LRESULT(0)
        }
        WM_CLOSE=>LRESULT(0),
        _=>DefWindowProcW(hwnd,msg,wp,lp)
    }
}
struct Window {hwnd:HWND,state:Box<State>}
impl Window {
    fn new()->Result<Self,String>{unsafe{
        let instance=windows::Win32::System::LibraryLoader::GetModuleHandleW(None).map_err(|e|e.to_string())?;
        let class=w!("MacLauncherProductionStage");
        let wc=WNDCLASSEXW{cbSize:std::mem::size_of::<WNDCLASSEXW>() as u32,hInstance:instance.into(),lpszClassName:class,lpfnWndProc:Some(procedure),hCursor:LoadCursorW(None,IDC_ARROW).unwrap_or_default(),..Default::default()};let _=RegisterClassExW(&wc);
        let hwnd=CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP|WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,class,w!("Stage Manager"),WS_POPUP,0,0,6,48,None,None,instance,None).map_err(|e|e.to_string())?;
        let window=Self{hwnd,state:Box::new(State{events:RefCell::new(Vec::new()),cards:RefCell::new(Vec::new()),paint:Cell::new(true),scale:Cell::new(1.0)})};
        SetWindowLongPtrW(hwnd,GWLP_USERDATA,window.state.as_ref() as *const _ as isize);Ok(window)
    }}
}
impl Drop for Window{fn drop(&mut self){unsafe{SetWindowLongPtrW(self.hwnd,GWLP_USERDATA,0);let _=DestroyWindow(self.hwnd);}}}
struct Preview {handle:isize,pid:u32}
impl Drop for Preview{fn drop(&mut self){if let Err(error)=unsafe{DwmUnregisterThumbnail(self.handle)}{crate::diagnostics::write(&format!("Thumbnail cleanup: {error}"));}}}
fn valid(window:&AppInfo)->bool{unsafe{let hwnd=HWND(window.hwnd as *mut _);let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));IsWindow(hwnd).as_bool()&&pid==window.pid&&pid!=0}}
fn iconic(window:&AppInfo)->bool{unsafe{IsIconic(HWND(window.hwnd as *mut _)).as_bool()}}

pub struct NativeStage {
    // DWM handles and GPU resources release before the destination HWND.
    restore_events:crate::stage_events::RestoreEvents,previews:HashMap<usize,Preview>,surface:IDCompositionSurface,pipeline:DirectCompositionPipeline,text:IDWriteFactory,
    failed:HashMap<(usize,u32),Instant>,changes:crate::stage_actions::StageChanges,spring:mac_physics_core::Spring,
    model:ShelfModel,geometry:Option<ShelfGeometry>,page:usize,pinned:bool,last_inside:Instant,last_t:f32,last_preview:Instant,error:Option<String>,last_bounds:(i32,i32,i32,i32),window:Window,
}
impl NativeStage {
    pub fn new(shared:&DirectCompositionPipeline)->Result<Self,String>{
        let window=Window::new()?;let pipeline=shared.for_window(window.hwnd,true).map_err(|e|e.to_string())?;
        let surface=unsafe{pipeline.dcomp_device().CreateSurface(200,200,DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_ALPHA_MODE_PREMULTIPLIED)}.map_err(|e|e.to_string())?;
        unsafe{pipeline.root_visual().SetContent(&surface)}.map_err(|e|e.to_string())?;
        Ok(Self{restore_events:crate::stage_events::RestoreEvents::new()?,previews:HashMap::new(),surface,pipeline,text:unsafe{DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)}.map_err(|e|e.to_string())?,failed:HashMap::new(),changes:crate::stage_actions::StageChanges::default(),spring:mac_physics_core::Spring::new(1.0,180.0,24.0),model:ShelfModel::default(),geometry:None,page:0,pinned:false,last_inside:Instant::now()-Duration::from_secs(1),last_t:-1.0,last_preview:Instant::now(),error:None,last_bounds:(0,0,0,0),window})
    }
    pub fn animating(&self)->bool{!self.spring.is_resting()}
    pub fn take_error(&mut self)->Option<String>{self.error.take()}
    pub fn hwnd(&self)->HWND{self.window.hwnd}
    pub fn frame(&mut self,dock:HWND,catalog:&AppCatalog,tracker:&AppTracker,dt:f32)->Result<(),String>{
        for (hwnd,pid) in self.restore_events.drain(){self.changes.observed_restore(hwnd,pid);}
        let next=ShelfModel::new(catalog,&tracker.all_apps());let model_changed=next!=self.model;self.model=next;
        let mut info=MONITORINFO{cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default()};
        unsafe{if !GetMonitorInfoW(MonitorFromWindow(dock,MONITOR_DEFAULTTOPRIMARY),&mut info).as_bool(){return Err("Cannot locate Stage display".into());}}
        let foreground=unsafe{GetForegroundWindow()};let mut foreground_rect=RECT::default();
        let mut class=[0u16;128];let class_length=unsafe{GetClassNameW(foreground,&mut class)}.max(0) as usize;
        let shell_desktop=["Progman","WorkerW","Shell_TrayWnd","Shell_SecondaryTrayWnd"].contains(&String::from_utf16_lossy(&class[..class_length]).as_str());
        let fullscreen=!shell_desktop && unsafe{GetWindowLongW(foreground,GWL_STYLE) as u32&WS_CAPTION.0==0 && GetWindowRect(foreground,&mut foreground_rect).is_ok()}
            && foreground_rect.left<=info.rcMonitor.left && foreground_rect.top<=info.rcMonitor.top && foreground_rect.right>=info.rcMonitor.right && foreground_rect.bottom>=info.rcMonitor.bottom;
        if fullscreen{unsafe{let _=ShowWindow(self.window.hwnd,SW_HIDE);}self.previews.clear();self.window.state.cards.borrow_mut().clear();self.window.state.events.borrow_mut().clear();self.spring.reset(0.0);self.last_t=-1.0;return Ok(());}
        let area=info.rcWork;
        let count=self.model.groups.iter().filter(|g|Some(&g.id)!=self.model.active.as_ref()).count();
        let geometry=ShelfGeometry::fit(crate::display::WorkArea{left:area.left,top:area.top,right:area.right,bottom:area.bottom},unsafe{windows::Win32::UI::HiDpi::GetDpiForWindow(dock)},count)?;
        let geometry_changed=self.geometry!=Some(geometry);
        if geometry_changed {
            let surface=unsafe{self.pipeline.dcomp_device().CreateSurface(geometry.width as u32,geometry.height as u32,DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_ALPHA_MODE_PREMULTIPLIED)}.map_err(|e|e.to_string())?;
            unsafe{self.pipeline.root_visual().SetContent(&surface)}.map_err(|e|e.to_string())?;
            self.surface=surface;self.geometry=Some(geometry);self.window.state.scale.set(geometry.scale);
        }
        let events:Vec<_>=self.window.state.events.borrow_mut().drain(..).collect();let mut page_changed=false;
        for event in events {
            match event {
                Event::Pin=>{self.pinned=!self.pinned;self.window.state.paint.set(true);},
                Event::Page(delta)=>{self.page=(self.page as i64+delta as i64).rem_euclid(self.model.pages(geometry.capacity) as i64) as usize;page_changed=true;}
                Event::Group(id,choose)=>if let Err(error)=self.activate(&id,choose,catalog){self.error=Some(error);},
            }
        }
        let mut pointer=POINT::default();let mut bounds=RECT::default();
        unsafe{let _=GetCursorPos(&mut pointer);let _=GetWindowRect(self.window.hwnd,&mut bounds);}
        let inside=pointer.x>=bounds.left&&pointer.x<bounds.right&&pointer.y>=bounds.top&&pointer.y<bounds.bottom;
        if inside{self.last_inside=Instant::now();}
        let target=if self.pinned||self.last_inside.elapsed()<Duration::from_millis(400){1.0}else{0.0};
        let t=self.spring.step(dt,target).clamp(0.0,1.0);let progress=t!=self.last_t;self.last_t=t;
        let (width,height,y)=geometry.visible_bounds(t);let position=(geometry.x,y,width,height);
        if self.last_bounds!=position {unsafe{SetWindowPos(self.window.hwnd,None,geometry.x,y,width,height,SWP_NOACTIVATE|SWP_NOZORDER).map_err(|e|e.to_string())?;}self.last_bounds=position;}
        let groups=self.model.page(self.page,geometry.capacity);
        let repaint=self.window.state.paint.replace(false);
        let redraw=geometry_changed||model_changed||page_changed||progress||repaint;
        if redraw{
            self.draw(geometry,&groups,t)?;
            self.pipeline.commit().map_err(|e|e.to_string())?;
            *self.window.state.cards.borrow_mut()=groups.iter().filter(|_|t>=0.98).enumerate().map(|(i,g)|{
                let top=(32.0*geometry.scale+i as f32*geometry.card_height) as i32;
                (g.id.clone(),RECT{left:0,top,right:geometry.width,bottom:top+geometry.card_height as i32})
            }).collect();
        }
        if redraw||self.last_preview.elapsed()>=Duration::from_millis(500){self.sync_previews(&groups,geometry,t)?;self.last_preview=Instant::now();}
        unsafe{if !IsWindowVisible(self.window.hwnd).as_bool(){let _=ShowWindow(self.window.hwnd,SW_SHOWNOACTIVATE);}}
        Ok(())
    }
    fn activate(&mut self,id:&str,choose:bool,catalog:&AppCatalog)->Result<(),String>{
        let mut tracker=AppTracker::new();tracker.refresh_running_apps();let fresh=ShelfModel::new(catalog,&tracker.all_apps());
        let group=fresh.groups.iter().find(|g|g.id==id).ok_or("The selected group has closed")?;
        let primary=if choose&&group.windows.len()>1 {
            match crate::dock_menu::windows(self.window.hwnd,&group.windows)? {
                Some(crate::dock_menu::DockAction::Activate(hwnd,pid))=>group.windows.iter().find(|window|window.hwnd==hwnd&&window.pid==pid),
                _=>return Ok(()),
            }
        }else{group.windows.first()}.ok_or("The selected window has closed")?;
        let previous=fresh.groups.iter().find(|group|Some(&group.id)==fresh.active.as_ref());
        let errors=self.changes.switch(previous,group,primary,&mut NativeOps)?;
        if !errors.is_empty(){self.error=Some(errors.join("\n"));}Ok(())
    }
    fn sync_previews(&mut self,groups:&[crate::stage_model::ShelfGroup],geometry:ShelfGeometry,t:f32)->Result<(),String>{
        let desired:HashMap<_,_>=groups.iter().filter_map(|group|group.windows.first()).map(|window|(window.hwnd,window.pid)).collect();
        self.previews.retain(|source,preview|desired.get(source)==Some(&preview.pid)&&t>0.05);
        self.failed.retain(|key,_|desired.get(&key.0)==Some(&key.1));
        if t<=0.05{self.failed.clear();return Ok(());}
        for (index,group) in groups.iter().enumerate(){
            let Some(window)=group.windows.first()else{continue;};let key=(window.hwnd,window.pid);
            if !valid(window)||self.failed.get(&key).is_some_and(|time|time.elapsed()<Duration::from_secs(3)){continue;}
            let result=(||->windows::core::Result<()>{unsafe{
                if !self.previews.contains_key(&window.hwnd){
                    let handle=DwmRegisterThumbnail(self.window.hwnd,HWND(window.hwnd as *mut _))?;
                    self.previews.insert(window.hwnd,Preview{handle,pid:window.pid});
                }
                let preview=&self.previews[&window.hwnd];let size=DwmQueryThumbnailSourceSize(preview.handle)?;
                let fit=mac_physics_core::compute_thumbnail_aspect_fit(size.cx.max(0) as u32,size.cy.max(0) as u32,184.0*geometry.scale,92.0*geometry.scale);
                let x=8.0*geometry.scale+fit.x;let y=60.0*geometry.scale+index as f32*geometry.card_height+fit.y;
                let props=DWM_THUMBNAIL_PROPERTIES{dwFlags:DWM_TNP_RECTDESTINATION|DWM_TNP_VISIBLE|DWM_TNP_OPACITY,rcDestination:RECT{left:x.round() as i32,top:y.round() as i32,right:(x+fit.width).round() as i32,bottom:(y+fit.height).round() as i32},fVisible:true.into(),opacity:(t*255.0) as u8,..Default::default()};
                DwmUpdateThumbnailProperties(preview.handle,&props)
            }})();
            if let Err(error)=result{self.previews.remove(&window.hwnd);self.failed.insert(key,Instant::now());self.window.state.paint.set(true);crate::diagnostics::write(&format!("Preview unavailable for {}: {error}",group.title));}
            else if self.failed.remove(&key).is_some(){self.window.state.paint.set(true);}
        }
        Ok(())
    }
    fn draw(&self,geometry:ShelfGeometry,groups:&[crate::stage_model::ShelfGroup],t:f32)->Result<(),String>{
        let mut offset=POINT::default();let context:ID2D1DeviceContext=unsafe{self.surface.BeginDraw(None,&mut offset)}.map_err(|e|e.to_string())?;
        let draw=(||->windows::core::Result<()>{unsafe{
            context.Clear(Some(&D2D1_COLOR_F{r:0.11,g:0.12,b:0.14,a:1.0}));
            let brush=context.CreateSolidColorBrush(&D2D1_COLOR_F{r:0.92,g:0.94,b:0.97,a:1.0},None)?;
            let font=self.text.CreateTextFormat(w!("Segoe UI"),None,DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,12.0*geometry.scale,w!(""))?;
            font.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;let ellipsis=self.text.CreateEllipsisTrimmingSign(&font)?;
            let text=|label:&str,y:f32,h:f32|->windows::core::Result<()>{
                let units:Vec<u16>=label.encode_utf16().collect();let layout=self.text.CreateTextLayout(&units,&font,(geometry.width as f32-16.0*geometry.scale).max(1.0),h)?;
                layout.SetTrimming(&DWRITE_TRIMMING{granularity:DWRITE_TRIMMING_GRANULARITY_CHARACTER,..Default::default()},&ellipsis)?;
                context.DrawTextLayout(D2D_POINT_2F{x:offset.x as f32+8.0*geometry.scale,y:offset.y as f32+y},&layout,&brush,D2D1_DRAW_TEXT_OPTIONS_CLIP);Ok(())
            };
            if t<0.05{
                let accent=context.CreateSolidColorBrush(&D2D1_COLOR_F{r:0.25,g:0.55,b:0.95,a:1.0},None)?;
                context.FillRectangle(&D2D_RECT_F{left:offset.x as f32,top:offset.y as f32,right:offset.x as f32+6.0*geometry.scale,bottom:offset.y as f32+geometry.height as f32},&accent);
            }else{
                text(if self.pinned{"Stages · unpin"}else{"Stages · pin"},6.0*geometry.scale,24.0*geometry.scale)?;
                for (index,group) in groups.iter().enumerate(){
                    let y=32.0*geometry.scale+index as f32*geometry.card_height;
                    text(&format!("{} ({})",group.title,group.windows.len()),y+4.0*geometry.scale,22.0*geometry.scale)?;
                    if group.windows.first().is_some_and(|window|self.failed.contains_key(&(window.hwnd,window.pid))){text("Preview unavailable",y+52.0*geometry.scale,24.0*geometry.scale)?;}
                }
                if groups.is_empty(){text("Other app windows appear here",60.0*geometry.scale,60.0*geometry.scale)?;}
                text(&format!("{} / {} · scroll to switch",self.page%self.model.pages(geometry.capacity)+1,self.model.pages(geometry.capacity)),geometry.height as f32-22.0*geometry.scale,20.0*geometry.scale)?;
            }Ok(())
        }})();let end=unsafe{self.surface.EndDraw()};draw.and(end).map_err(|e|e.to_string())
    }
}
impl Drop for NativeStage{fn drop(&mut self){
    let foreground=unsafe{GetForegroundWindow()};
    for (hwnd,pid) in self.restore_events.drain(){self.changes.observed_restore(hwnd,pid);}
    for error in self.changes.restore_all(&mut NativeOps){crate::diagnostics::write(&error);}
    unsafe{if IsWindow(foreground).as_bool(){let _=SetForegroundWindow(foreground);}}
}}

struct NativeOps;
impl crate::stage_actions::WindowOps for NativeOps{
 fn valid(&self,window:&AppInfo)->bool{valid(window)}
 fn minimized(&self,window:&AppInfo)->bool{iconic(window)}
 fn activate(&mut self,window:&AppInfo)->Result<(),String>{ProcessLauncher::activate_window(window.hwnd,window.pid)}
 fn restore(&mut self,window:&AppInfo)->Result<(),String>{
  if !valid(window){return Err("The window has closed or changed".into());}
  if iconic(window){unsafe{let _=ShowWindow(HWND(window.hwnd as *mut _),SW_RESTORE);}}
  if iconic(window){Err("Windows could not restore a group window".into())}else{Ok(())}
 }
 fn minimize(&mut self,window:&AppInfo)->Result<(),String>{
  if !valid(window){return Err("The window has closed or changed".into());}
  unsafe{let _=ShowWindow(HWND(window.hwnd as *mut _),SW_MINIMIZE);}
  if iconic(window){Ok(())}else{Err("Windows could not minimize a previous group window".into())}
 }
}
