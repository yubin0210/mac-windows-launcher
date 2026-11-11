//! Unicode clipboard ownership. Native EDIT handles paste and selection itself.
pub fn unicode_text(text: &str) -> Result<Vec<u16>, String> {
    if text.contains('\0') { return Err("Clipboard text contains a NUL character".into()); }
    Ok(text.encode_utf16().chain(Some(0)).collect())
}
#[cfg(windows)]
pub fn copy(hwnd: windows::Win32::Foundation::HWND, text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    struct Clipboard;
    impl Drop for Clipboard { fn drop(&mut self) { unsafe { let _ = CloseClipboard(); } } }
    struct Block(Option<HGLOBAL>);
    impl Drop for Block { fn drop(&mut self) { if let Some(memory) = self.0 { unsafe { let _ = GlobalFree(memory); } } } }
    let units = unicode_text(text)?;
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2).map_err(|e| e.to_string())?;
        let mut block = Block(Some(memory));
        let pointer = GlobalLock(memory);
        if pointer.is_null() { return Err("Cannot allocate clipboard text".into()); }
        std::ptr::copy_nonoverlapping(units.as_ptr(), pointer as *mut u16, units.len());
        let _ = GlobalUnlock(memory);
        OpenClipboard(hwnd).map_err(|e| format!("Clipboard is busy: {e}"))?;
        let _clipboard = Clipboard;
        EmptyClipboard().map_err(|e| e.to_string())?;
        SetClipboardData(13, HANDLE(memory.0)).map_err(|e| e.to_string())?; // CF_UNICODETEXT
        block.0 = None; // The system owns the allocation after a successful transfer.
    }
    Ok(())
}
