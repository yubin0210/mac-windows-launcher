//! Out-of-context restore observations invalidate Stage's minimization ownership.
//! The registry contains values only; late callbacks never dereference controller pointers.
use std::{cell::RefCell,collections::HashMap};
use windows::Win32::{Foundation::*,UI::{Accessibility::*,WindowsAndMessaging::*}};
thread_local!{static OBSERVATIONS:RefCell<HashMap<usize,Vec<(usize,u32)>>>=RefCell::new(HashMap::new());}
unsafe extern "system" fn restored(hook:HWINEVENTHOOK,_:u32,hwnd:HWND,_:i32,_:i32,_:u32,_:u32){
 let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
 if pid!=0{OBSERVATIONS.with(|registry|{if let Some(events)=registry.borrow_mut().get_mut(&(hook.0 as usize)){events.push((hwnd.0 as usize,pid));}});}
}
pub struct RestoreEvents(HWINEVENTHOOK);
impl RestoreEvents{
 pub fn new()->Result<Self,String>{
  let hook=unsafe{SetWinEventHook(EVENT_SYSTEM_MINIMIZEEND,EVENT_SYSTEM_MINIMIZEEND,None,Some(restored),0,0,WINEVENT_OUTOFCONTEXT|WINEVENT_SKIPOWNPROCESS)};
  if hook.0.is_null(){return Err("Cannot observe external window restoration".into());}
  OBSERVATIONS.with(|registry|{registry.borrow_mut().insert(hook.0 as usize,Vec::new());});Ok(Self(hook))
 }
 pub fn drain(&self)->Vec<(usize,u32)>{OBSERVATIONS.with(|registry|registry.borrow_mut().get_mut(&(self.0.0 as usize)).map(std::mem::take).unwrap_or_default())}
}
impl Drop for RestoreEvents{fn drop(&mut self){OBSERVATIONS.with(|registry|{registry.borrow_mut().remove(&(self.0.0 as usize));});unsafe{let _=UnhookWinEvent(self.0);}}}
