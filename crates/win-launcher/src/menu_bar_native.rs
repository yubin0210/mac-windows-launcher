//! Production AppBar. Native popup menus never resize the reserved top strip.
use crate::{directx::DirectCompositionPipeline, menu_bar_model::{BarAction, BarModel}, app_catalog::AppCatalog, AppTracker};
use std::cell::{Cell, RefCell};
use std::time::{Instant, Duration};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::DirectComposition::IDCompositionSurface;
use windows::Win32::Graphics::Dxgi::Common::*;
const CALLBACK: u32 = WM_APP + 204;
struct State { paint: Cell<bool>, position: Cell<bool>, restart: Cell<bool>, clicks: RefCell<Vec<(f32,f32)>>, taskbar_message: u32, reserved: Cell<bool> }
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
    if pointer.is_null() { return DefWindowProcW(hwnd,message,wp,lp); }
    let state = &*pointer;
    match message {
        WM_ACTIVATE | WM_WINDOWPOSCHANGED if state.reserved.get() => {
            let mut data=APPBARDATA { cbSize:std::mem::size_of::<APPBARDATA>() as u32,hWnd:hwnd,lParam:LPARAM(((wp.0&0xFFFF)!=WA_INACTIVE as usize) as isize),..Default::default() };
            let _=SHAppBarMessage(if message==WM_ACTIVATE { ABM_ACTIVATE } else { ABM_WINDOWPOSCHANGED },&mut data);
            DefWindowProcW(hwnd,message,wp,lp)
        }
        WM_PAINT => { let _=ValidateRect(hwnd,None); state.paint.set(true); LRESULT(0) }
        WM_LBUTTONUP => { state.clicks.borrow_mut().push(((lp.0&0xFFFF) as i16 as f32, ((lp.0>>16)&0xFFFF) as i16 as f32)); LRESULT(0) }
        WM_DPICHANGED | WM_DISPLAYCHANGE | WM_SETTINGCHANGE => { state.position.set(true); LRESULT(0) }
        CALLBACK => {
            if wp.0 as u32 == ABN_POSCHANGED { state.position.set(true); }
            if wp.0 as u32 == ABN_FULLSCREENAPP {
                let _ = SetWindowPos(hwnd, if lp.0 != 0 { HWND_BOTTOM } else { HWND_TOPMOST },0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE);
            }
            LRESULT(0)
        }
        m if m == state.taskbar_message => { state.restart.set(true); LRESULT(0) }
        WM_CLOSE => LRESULT(0), // Lifecycle belongs to the controller/settings.
        _ => DefWindowProcW(hwnd,message,wp,lp),
    }
}
struct Window { hwnd: HWND, state: Box<State>,  }
impl Window {
    fn new() -> Result<Self,String> { unsafe {
        let state=Box::new(State { paint:Cell::new(true),position:Cell::new(true),restart:Cell::new(false),clicks:RefCell::new(Vec::new()),reserved:Cell::new(false),taskbar_message:RegisterWindowMessageW(w!("TaskbarCreated")) });
        let instance=windows::Win32::System::LibraryLoader::GetModuleHandleW(None).map_err(|e|e.to_string())?;
        let class=w!("MacLauncherProductionMenuBar");
        let wc=WNDCLASSEXW { cbSize:std::mem::size_of::<WNDCLASSEXW>() as u32,lpfnWndProc:Some(procedure),hInstance:instance.into(),lpszClassName:class,hCursor:LoadCursorW(None,IDC_ARROW).unwrap_or_default(),..Default::default() };
        let _=RegisterClassExW(&wc);
        let hwnd=CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP|WS_EX_TOOLWINDOW|WS_EX_TOPMOST|WS_EX_NOACTIVATE,class,w!("MacLauncher Menu Bar"),WS_POPUP,0,0,1,28,None,None,instance,None).map_err(|e|e.to_string())?;
        let window=Self { hwnd,state };
        SetWindowLongPtrW(hwnd,GWLP_USERDATA,window.state.as_ref() as *const _ as isize);
        Ok(window)
    } }
    fn register(&mut self) -> Result<(),String> {
        let mut data=self.data(); data.uCallbackMessage=CALLBACK;
        if unsafe { SHAppBarMessage(ABM_NEW,&mut data) }==0 { return Err("Windows could not register the menu AppBar".into()); }
        self.state.reserved.set(true);
        Ok(())
    }
    fn data(&self) -> APPBARDATA { APPBARDATA { cbSize:std::mem::size_of::<APPBARDATA>() as u32,hWnd:self.hwnd,uEdge:ABE_TOP,..Default::default() } }
}
impl Drop for Window { fn drop(&mut self) { unsafe {
    if self.state.reserved.replace(false) { let mut data=self.data(); let _=SHAppBarMessage(ABM_REMOVE,&mut data); }
    SetWindowLongPtrW(self.hwnd,GWLP_USERDATA,0);
    let _=DestroyWindow(self.hwnd);
} } }
#[derive(Clone,Copy)]
pub enum MenuCommand { Search, Settings, Network, Sound, DateTime, Power, About, TaskManager, Logs, Hide, Exit }
pub struct NativeMenuBar {
    // GPU references are released before the HWND and AppBar reservation.
    surface: IDCompositionSurface, pipeline: DirectCompositionPipeline,
    text_factory: IDWriteFactory, window: Window, rect: RECT, scale: f32,
    model: BarModel, last_status: Instant, monitor: HMONITOR, dpi: u32,
}
impl NativeMenuBar {
    pub fn new(shared:&DirectCompositionPipeline,dock:HWND) -> Result<Self,String> {
        let window=Window::new()?;
        let pipeline=shared.for_window(window.hwnd,true).map_err(|e|e.to_string())?;
        let surface=unsafe { pipeline.dcomp_device().CreateSurface(1,28,DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_ALPHA_MODE_PREMULTIPLIED) }.map_err(|e|e.to_string())?;
        let text_factory=unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.map_err(|e|e.to_string())?;
        let mut bar=Self { surface,pipeline,text_factory,window,rect:RECT::default(),scale:1.0,model:BarModel::default(),last_status:Instant::now()-Duration::from_secs(2),monitor:HMONITOR::default(),dpi:0 };
        bar.window.register()?;
        bar.place(dock,true)?;
        Ok(bar)
    }
    fn place(&mut self,dock:HWND,force:bool) -> Result<(),String> { unsafe {
        let monitor=MonitorFromWindow(dock,MONITOR_DEFAULTTOPRIMARY);
        let mut info=MONITORINFO { cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default() };
        if !GetMonitorInfoW(monitor,&mut info).as_bool() { return Err("Cannot locate menu bar monitor".into()); }
        let dpi=windows::Win32::UI::HiDpi::GetDpiForWindow(dock);
        self.monitor=monitor; self.dpi=dpi;
        let scale=dpi as f32/96.0;
        let height=(28.0*scale).round().max(1.0) as i32;
        let mut data=self.window.data(); data.rc=info.rcMonitor; data.rc.bottom=data.rc.top+height;
        let _=SHAppBarMessage(ABM_QUERYPOS,&mut data);
        data.rc.bottom=data.rc.top+height; // Respect the position negotiated with other AppBars.
        if !force && data.rc==self.rect && scale==self.scale { return Ok(()); }
        let _=SHAppBarMessage(ABM_SETPOS,&mut data);
        SetWindowPos(self.window.hwnd,None,data.rc.left,data.rc.top,data.rc.right-data.rc.left,data.rc.bottom-data.rc.top,SWP_NOACTIVATE|SWP_NOZORDER).map_err(|e|e.to_string())?;
        let width=(data.rc.right-data.rc.left).max(1) as u32;
        let height=(data.rc.bottom-data.rc.top).max(1) as u32;
        let surface=self.pipeline.dcomp_device().CreateSurface(width,height,DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_ALPHA_MODE_PREMULTIPLIED).map_err(|e|e.to_string())?;
        self.pipeline.root_visual().SetContent(&surface).map_err(|e|e.to_string())?;
        self.surface=surface; self.rect=data.rc; self.scale=scale; self.window.state.paint.set(true);
        Ok(())
    } }
    pub fn frame(&mut self,dock:HWND,catalog:&AppCatalog,tracker:&AppTracker) -> Result<Vec<MenuCommand>,String> {
        let restart=self.window.state.restart.replace(false);
        if restart { self.window.state.reserved.set(false); self.window.register()?; }
        let changed=unsafe { MonitorFromWindow(dock,MONITOR_DEFAULTTOPRIMARY)!=self.monitor || windows::Win32::UI::HiDpi::GetDpiForWindow(dock)!=self.dpi };
        if self.window.state.position.replace(false) || changed || restart { self.place(dock,restart)?; }
        if self.last_status.elapsed()>=Duration::from_secs(1) {
            let mut next=self.model.clone();
            next.status=crate::system_status::read_menu_status();
            if let Some(app)=tracker.focused_app() {
                next.app_title=catalog.apps().iter().find(|record|record.matches(app)).map(|record|record.title.clone()).unwrap_or_else(||app.process_name.rsplit(['\\','/']).next().unwrap_or(&app.process_name).trim_end_matches(".exe").into());
            }
            if next!=self.model { self.model=next; self.window.state.paint.set(true); }
            self.last_status=Instant::now();
        }
        if self.window.state.paint.replace(false) {
            self.draw()?;
            self.pipeline.commit().map_err(|e|e.to_string())?;
            unsafe { if !IsWindowVisible(self.window.hwnd).as_bool() { let _=ShowWindow(self.window.hwnd,SW_SHOWNOACTIVATE); } }
        }
        let clicks:Vec<_>=self.window.state.clicks.borrow_mut().drain(..).collect();
        let mut commands=Vec::new();
        for (x,y) in clicks {
            match self.model.hit((self.rect.right-self.rect.left) as f32,self.scale,x,y) {
                Some(BarAction::Menu)=>if let Some(command)=self.menu()? { commands.push(command); },
                Some(action)=>commands.push(match action { BarAction::Search=>MenuCommand::Search,BarAction::Sound=>MenuCommand::Sound,BarAction::Network=>MenuCommand::Network,BarAction::DateTime=>MenuCommand::DateTime,BarAction::Power=>MenuCommand::Power,BarAction::Menu=>unreachable!() }),
                None=>{}
            }
        }
        Ok(commands)
    }
    fn menu(&self)->Result<Option<MenuCommand>,String> { unsafe {
        let menu=CreatePopupMenu().map_err(|e|e.to_string())?;
        let commands=[("Search",MenuCommand::Search),("Windows settings",MenuCommand::Settings),("Network settings",MenuCommand::Network),("Sound settings",MenuCommand::Sound),("About this PC",MenuCommand::About),("Task Manager",MenuCommand::TaskManager),("Open logs",MenuCommand::Logs),("Hide menu bar",MenuCommand::Hide),("Exit MacLauncher",MenuCommand::Exit)];
        for (index,(label,_)) in commands.iter().enumerate() {
            let label:Vec<u16>=label.encode_utf16().chain(Some(0)).collect();
            if let Err(error)=AppendMenuW(menu,MF_STRING,index+1,PCWSTR(label.as_ptr())) { let _=DestroyMenu(menu); return Err(error.to_string()); }
        }
        let previous=GetForegroundWindow(); let _=SetForegroundWindow(self.window.hwnd);
        let selected=TrackPopupMenu(menu,TPM_RETURNCMD|TPM_LEFTALIGN,self.rect.left,self.rect.bottom,0,self.window.hwnd,None).0;
        let _=DestroyMenu(menu); let _=PostMessageW(self.window.hwnd,WM_NULL,WPARAM(0),LPARAM(0));
        if GetForegroundWindow()==self.window.hwnd { let _=SetForegroundWindow(previous); }
        Ok(selected.checked_sub(1).and_then(|index|commands.get(index as usize)).map(|(_,command)|*command))
    } }
    fn draw(&self)->Result<(),String> {
        let mut offset=POINT::default();
        let context:ID2D1DeviceContext=unsafe { self.surface.BeginDraw(None,&mut offset) }.map_err(|e|e.to_string())?;
        let draw=(||->windows::core::Result<()> { unsafe {
            context.Clear(Some(&D2D1_COLOR_F { r:0.11,g:0.12,b:0.14,a:1.0 }));
            let brush=context.CreateSolidColorBrush(&D2D1_COLOR_F { r:0.96,g:0.96,b:0.97,a:1.0 },None)?;
            let format=self.text_factory.CreateTextFormat(w!("Segoe UI"),None,DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,13.0*self.scale,w!(""))?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?; format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            let ellipsis=self.text_factory.CreateEllipsisTrimmingSign(&format)?;
            for item in self.model.layout((self.rect.right-self.rect.left) as f32,self.scale) {
                let text:Vec<u16>=item.label.encode_utf16().collect();
                let layout=self.text_factory.CreateTextLayout(&text,&format,(item.width-12.0*self.scale).max(1.0),28.0*self.scale)?;
                layout.SetTrimming(&DWRITE_TRIMMING { granularity:DWRITE_TRIMMING_GRANULARITY_CHARACTER,..Default::default() },&ellipsis)?;
                context.DrawTextLayout(D2D_POINT_2F { x:offset.x as f32+item.x+6.0*self.scale,y:offset.y as f32 },&layout,&brush,D2D1_DRAW_TEXT_OPTIONS_CLIP);
            }
            Ok(())
        } })();
        let end=unsafe { self.surface.EndDraw() };
        draw.and(end).map_err(|e|e.to_string())
    }
}
