//! AUMID extraction for shortcuts, AppsFolder entries and live Windows applications.
use windows::core::{BSTR, GUID, PWSTR};
use windows::Win32::Foundation::{HANDLE, HWND, ERROR_SUCCESS};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY, SHGetPropertyStoreForWindow};

// PKEY_AppUserModel_ID from the Windows Property System schema.
pub const APP_ID_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3), pid: 5,
};
pub unsafe fn store_app_id(store: &IPropertyStore) -> Option<String> {
    let value = store.GetValue(&APP_ID_KEY).ok()?;
    let text = BSTR::try_from(&value).ok()?.to_string();
    if text.is_empty() { None } else { Some(text) }
}
pub unsafe fn window_app_id(hwnd: HWND, process: HANDLE) -> Option<String> {
    if let Ok(store) = SHGetPropertyStoreForWindow::<_, IPropertyStore>(hwnd) {
        if let Some(id) = store_app_id(&store) { return Some(id); }
    }
    let mut buffer = [0u16; 512];
    let mut length = buffer.len() as u32;
    let result = windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId(process, &mut length, PWSTR(buffer.as_mut_ptr()));
    if result != ERROR_SUCCESS || length == 0 || length as usize > buffer.len() { return None; }
    let count = buffer.iter().position(|c| *c == 0).unwrap_or(length as usize);
    String::from_utf16(&buffer[..count]).ok().filter(|text| !text.is_empty())
}
