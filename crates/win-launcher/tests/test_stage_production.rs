use win_launcher::{app_catalog::{AppCatalog, AppRecord}, AppTracker};
use win_launcher::stage_model::{ShelfModel, ShelfGeometry};
#[test]
fn groups_use_app_identity_and_actual_focus_not_display_name() {
    let a=AppRecord::new(r"C:\one\editor.exe","Editor",Some(r"C:\one\editor.exe".into()));
    let b=AppRecord::new(r"D:\two\editor.exe","Editor",Some(r"D:\two\editor.exe".into()));
    let mut tracker=AppTracker::new();
    tracker.register_mock_running_app_with_hwnd(r"C:\one\editor.exe",1,false,100);
    tracker.register_mock_running_app_with_hwnd(r"C:\one\editor.exe",1,false,101);
    tracker.register_mock_running_app_with_hwnd(r"D:\two\editor.exe",2,true,200);
    let model=ShelfModel::new(&AppCatalog::new(vec![a.clone(),b.clone()]),&tracker.all_apps());
    assert_eq!(model.groups.len(),2);
    assert_eq!(model.active.as_deref(),Some(b.id.as_str()));
    assert_eq!(model.page(0,5)[0].id,a.id);
    assert_eq!(model.page(0,5)[0].windows.len(),2);
    assert!(model.page(1,1).iter().all(|group|group.id==a.id));
    assert!(!win_launcher::settings::Settings::default().stage_enabled);
}
#[test]
fn shelf_fits_work_area_and_collapsed_input_is_small() {
    use win_launcher::display::WorkArea;
    for dpi in [96,144,192] {
        let area=WorkArea{left:-1920,top:28,right:0,bottom:1000};
        let layout=ShelfGeometry::fit(area,dpi,10).unwrap();
        assert!(layout.capacity>0);
        assert!(layout.y>=area.top && layout.y+layout.height<=area.bottom);
        assert!(layout.width<1920);
        let (width,height,y)=layout.visible_bounds(0.0);
        assert!(width<=12 && height<=96);
        assert!(y>=area.top);
        assert_eq!(layout.visible_bounds(1.0).0,layout.width);
    }
}
#[test]
fn generic_host_does_not_merge_explicit_app_ids(){
 let generic=AppRecord::new(r"C:\host.exe","AAA Generic",Some(r"C:\host.exe".into()));
 let one=AppRecord::new("shell:AppsFolder\\One!App","One",None).with_app_id("One!App".into());
 let mut windows=vec![win_launcher::AppInfo{process_name:r"C:\host.exe".into(),pid:11,hwnd:100,is_focused:true,app_id:Some("One!App".into())}];
 windows.push(win_launcher::AppInfo{hwnd:101,pid:12,app_id:Some("Two!App".into()),is_focused:false,..windows[0].clone()});
 let model=ShelfModel::new(&AppCatalog::new(vec![generic,one.clone()]),&windows);
 assert_eq!(model.groups.len(),2);assert_eq!(model.active.as_deref(),Some(one.id.as_str()));
}
