//! Own only minimizations performed by Stage. Never infer ownership from current minimized state.
use crate::{AppInfo,stage_model::ShelfGroup};
use std::collections::HashMap;
pub trait WindowOps {
 fn valid(&self,window:&AppInfo)->bool;
 fn minimized(&self,window:&AppInfo)->bool;
 fn activate(&mut self,window:&AppInfo)->Result<(),String>;
 fn restore(&mut self,window:&AppInfo)->Result<(),String>;
 fn minimize(&mut self,window:&AppInfo)->Result<(),String>;
}
#[derive(Default)]
pub struct StageChanges{owned:HashMap<usize,AppInfo>}
impl StageChanges{
 pub fn observed_restore(&mut self,hwnd:usize,pid:u32){if self.owned.get(&hwnd).is_some_and(|window|window.pid==pid){self.owned.remove(&hwnd);}}

 pub fn switch(&mut self,previous:Option<&ShelfGroup>,target:&ShelfGroup,primary:&AppInfo,ops:&mut impl WindowOps)->Result<Vec<String>,String>{
  ops.activate(primary)?; // No other window changes before this succeeds.
  let mut errors=Vec::new();
  for window in &target.windows{
   if !ops.valid(window){continue;}
   if ops.minimized(window){if let Err(error)=ops.restore(window){errors.push(error);}}
   if !ops.minimized(window){self.owned.remove(&window.hwnd);}
  }
  if let Some(previous)=previous.filter(|group|group.id!=target.id){
   for window in &previous.windows{
    if ops.valid(window)&&!ops.minimized(window){
     match ops.minimize(window){
      Ok(()) if ops.minimized(window)=>{self.owned.insert(window.hwnd,window.clone());}
      Ok(())=>errors.push("A previous window did not minimize".into()),
      Err(error)=>errors.push(error),
     }
    }
   }
  }
  ops.activate(primary)?;
  Ok(errors)
 }
 pub fn restore_all(&mut self,ops:&mut impl WindowOps)->Vec<String>{
  let mut errors=Vec::new();
  self.owned.retain(|_,window|{
   if !ops.valid(window)||!ops.minimized(window){return false;}
   match ops.restore(window){
    Ok(()) if !ops.minimized(window)=>false,
    Ok(())=>{errors.push(format!("Stage window {} remains minimized",window.hwnd));true}
    Err(error)=>{errors.push(error);true}
   }
  });errors
 }
}
