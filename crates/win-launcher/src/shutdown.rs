//! Installer shutdown request: same executable path only, normal WM_CLOSE, bounded wait.
use windows::core::{w,PWSTR};
use windows::Win32::{Foundation::*,System::Threading::*,UI::WindowsAndMessaging::*};
pub fn request()->Result<(),String>{
 let hwnd=match unsafe{FindWindowW(w!("MacWinLauncherDockClass"),None)}{Ok(hwnd)=>hwnd,Err(_)=>return Ok(())};
 let mut pid=0;unsafe{GetWindowThreadProcessId(hwnd,Some(&mut pid));}
 let handle=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,false,pid)}.map_err(|e|e.to_string())?;
 struct Process(HANDLE);impl Drop for Process{fn drop(&mut self){unsafe{let _=CloseHandle(self.0);}}}
 let process=Process(handle);
 let mut path=vec![0u16;32768];let mut length=path.len() as u32;
 unsafe{QueryFullProcessImageNameW(process.0,PROCESS_NAME_FORMAT(0),PWSTR(path.as_mut_ptr()),&mut length)}.map_err(|e|e.to_string())?;
 use std::os::windows::ffi::OsStringExt;
 let running=std::path::PathBuf::from(std::ffi::OsString::from_wide(&path[..length as usize])).canonicalize().map_err(|e|e.to_string())?;
 let current=std::env::current_exe().and_then(|path|path.canonicalize()).map_err(|e|e.to_string())?;
 if running!=current{return Err("Another MacLauncher installation is running; close it from its tray first".into());}
 unsafe{PostMessageW(hwnd,WM_CLOSE,WPARAM(0),LPARAM(0))}.map_err(|e|e.to_string())?;
 if unsafe{WaitForSingleObject(process.0,10000)}!=WAIT_OBJECT_0{return Err("MacLauncher did not finish shutting down; close it from the tray before updating".into());}
 Ok(())
}
