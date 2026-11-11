use win_launcher::icon_cache::IconPixels;
#[test]
fn pixels_preserve_premultiplied_alpha_and_reject_invalid_dimensions() {
    let bytes = vec![10, 20, 30, 64, 0, 0, 0, 0];
    let pixels = IconPixels::new(2, 1, bytes.clone()).unwrap();
    assert_eq!(pixels.bgra, bytes);
    assert!(IconPixels::new(2, 2, vec![0; 4]).is_err());
    assert!(IconPixels::new(0, 1, vec![]).is_err());
    assert!(IconPixels::new(u32::MAX, u32::MAX, vec![]).is_err());
}

#[test]
fn an_invisible_shell_bitmap_uses_the_identifiable_fallback() {
    assert!(IconPixels::new(1, 1, vec![80, 40, 20, 0]).is_err());
}
