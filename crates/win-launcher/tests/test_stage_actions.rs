use win_launcher::{AppInfo,stage_actions::{StageChanges,WindowOps},stage_model::ShelfGroup};
use std::collections::HashSet;
#[derive(Default)]
struct Windows{minimized:HashSet<usize>,fail:bool,events:Vec<String>}
impl WindowOps for Windows{
 fn valid(&self,_:&AppInfo)->bool{true}
 fn minimized(&self,w:&AppInfo)->bool{self.minimized.contains(&w.hwnd)}
 fn activate(&mut self,w:&AppInfo)->Result<(),String>{self.events.push(format!("activate:{}",w.hwnd));if self.fail{Err("activation denied".into())}else{Ok(())}}
 fn restore(&mut self,w:&AppInfo)->Result<(),String>{self.events.push(format!("restore:{}",w.hwnd));self.minimized.remove(&w.hwnd);Ok(())}
 fn minimize(&mut self,w:&AppInfo)->Result<(),String>{self.events.push(format!("minimize:{}",w.hwnd));self.minimized.insert(w.hwnd);Ok(())}
}
fn window(id:usize)->AppInfo{AppInfo{process_name:"app.exe".into(),pid:id as u32,hwnd:id,is_focused:false,app_id:None}}
fn group(id:usize)->ShelfGroup{ShelfGroup{id:id.to_string(),title:"App".into(),windows:vec![window(id)]}}
#[test]
fn failed_activation_does_not_minimize_previous_group(){
 let mut changes=StageChanges::default();let mut ops=Windows{fail:true,..Default::default()};
 assert!(changes.switch(Some(&group(1)),&group(2),&window(2),&mut ops).is_err());
 assert!(ops.minimized.is_empty());assert_eq!(ops.events,vec!["activate:2"]);
}
#[test]
fn disable_restores_only_stage_minimizations_and_respects_manual_restores(){
 let mut changes=StageChanges::default();let mut ops=Windows::default();ops.minimized.insert(9);
 let previous=ShelfGroup{id:"previous".into(),title:"App".into(),windows:vec![window(1),window(9)]};
 changes.switch(Some(&previous),&group(2),&window(2),&mut ops).unwrap();
 assert!(ops.minimized.contains(&1));
 changes.restore_all(&mut ops);assert!(!ops.minimized.contains(&1));assert!(ops.minimized.contains(&9));
 changes.switch(Some(&group(1)),&group(2),&window(2),&mut ops).unwrap();
 ops.minimized.remove(&1);ops.events.clear();changes.restore_all(&mut ops);assert!(ops.events.is_empty());
}
#[test]
fn external_restore_releases_ownership_before_a_later_manual_minimize(){
 let mut changes=StageChanges::default();let mut ops=Windows::default();
 changes.switch(Some(&group(1)),&group(2),&window(2),&mut ops).unwrap();
 changes.observed_restore(1,1); // Native EVENT_SYSTEM_MINIMIZEEND, not polling.
 ops.minimized.insert(1);ops.events.clear();changes.restore_all(&mut ops);
 assert!(ops.minimized.contains(&1));assert!(ops.events.is_empty());
}
