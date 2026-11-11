//! Shell icon extraction runs off the UI thread. Only CPU pixels cross the boundary.
#[derive(Debug, Clone)]
pub struct IconPixels {
    pub width: u32,
    pub height: u32,
    /// Top-down BGRA with premultiplied alpha, matching the Shell bitmap.
    pub bgra: Vec<u8>,
}
impl IconPixels {
    pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 512 || height > 512 {
            return Err("Invalid icon dimensions".into());
        }
        if bgra.len() != width as usize * height as usize * 4 {
            return Err("Invalid icon pixel buffer".into());
        }
        if !bgra.chunks_exact(4).any(|pixel| pixel[3] != 0) { return Err("Shell icon is fully transparent".into()); }
        Ok(Self { width, height, bgra })
    }
}

#[cfg(windows)]
pub mod native {
    use super::*;
    use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
    use std::collections::HashSet;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{SHCreateItemFromParsingName, IShellItem, IShellItemImageFactory, SIIGBF_ICONONLY};

    pub struct IconReply { pub id: String, pub source: String, pub pixels: Result<IconPixels, String> }
    pub struct IconLoader {
        sender: SyncSender<(String, String)>,
        replies: Receiver<IconReply>,
        requested: HashSet<(String, String)>,
    }
    impl IconLoader {
        pub fn new() -> Self {
            let (sender, requests) = sync_channel::<(String, String)>(32);
            let (results, replies) = sync_channel(32);
            std::thread::spawn(move || {
                let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok();
                while let Ok((id, source)) = requests.recv() {
                    let pixels = match &initialized {
                        Ok(()) => extract(&source, 256),
                        Err(error) => Err(format!("Icon COM initialization: {error}")),
                    };
                    if results.send(IconReply { id, source, pixels }).is_err() { break; }
                }
                if initialized.is_ok() { unsafe { CoUninitialize(); } }
            });
            Self { sender, replies, requested: HashSet::new() }
        }
        pub fn request(&mut self, id: &str, source: &str) {
            let key = (id.to_string(), source.to_string());
            if self.requested.contains(&key) { return; }
            match self.sender.try_send(key.clone()) {
                Ok(()) => { self.requested.insert(key); }
                Err(TrySendError::Full(_)) => {} // Retry next frame without blocking UI.
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
        pub fn drain(&self) -> Vec<IconReply> { self.replies.try_iter().collect() }
        pub fn retain(&mut self, sources: &[(String, String)]) {
            self.requested.retain(|key| sources.contains(key));
        }
    }
    impl Default for IconLoader { fn default() -> Self { Self::new() } }

    struct Bitmap(HBITMAP);
    impl Drop for Bitmap { fn drop(&mut self) { unsafe { let _ = DeleteObject(self.0); } } }
    struct Dc(HDC);
    impl Drop for Dc { fn drop(&mut self) { unsafe { let _ = DeleteDC(self.0); } } }
    fn extract(source: &str, size: i32) -> Result<IconPixels, String> {
        unsafe {
            let wide: Vec<u16> = source.encode_utf16().chain(Some(0)).collect();
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(|e| e.to_string())?;
            let factory: IShellItemImageFactory = item.cast().map_err(|e| e.to_string())?;
            let bitmap = Bitmap(factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).map_err(|e| e.to_string())?);
            let mut details = BITMAP::default();
            if GetObjectW(bitmap.0, std::mem::size_of::<BITMAP>() as i32, Some(&mut details as *mut _ as *mut core::ffi::c_void)) == 0 {
                return Err("Cannot inspect Shell bitmap".into());
            }
            let width = details.bmWidth;
            let height = details.bmHeight;
            if width <= 0 || height <= 0 || width > 512 || height > 512 { return Err("Unexpected Shell bitmap dimensions".into()); }
            let dc = Dc(CreateCompatibleDC(None));
            if dc.0.0.is_null() { return Err("Cannot create icon conversion DC".into()); }
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB.0;
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            if GetDIBits(dc.0, bitmap.0, 0, height as u32, Some(pixels.as_mut_ptr() as *mut core::ffi::c_void), &mut info, DIB_RGB_COLORS) != height {
                return Err("Cannot read Shell bitmap pixels".into());
            }
            IconPixels::new(width as u32, height as u32, pixels)
        }
    }
}
