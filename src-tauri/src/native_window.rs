use std::{ffi::c_void, mem::size_of, ptr};

use tauri::WebviewWindow;
use windows_sys::Win32::{
    Graphics::Dwm::{
        DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, ICON_SMALL, ICON_SMALL2, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPos,
        WM_SETICON, WS_EX_DLGMODALFRAME,
    },
};

const LIGHT_CAPTION: u32 = color_ref(0xf4, 0xf5, 0xf7);
const DARK_CAPTION: u32 = color_ref(0x17, 0x19, 0x1c);

pub fn configure(window: &WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    let hwnd = native_handle(window)?;
    let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };

    if extended_style & WS_EX_DLGMODALFRAME as isize == 0 {
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                extended_style | WS_EX_DLGMODALFRAME as isize,
            );
        }
    }

    // Tauri assigns a window-level small icon, which takes precedence over the
    // dialog-frame style. Clear only the caption-sized icons; ICON_BIG remains
    // available to the taskbar and Alt+Tab switcher.
    unsafe {
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, 0);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL2 as usize, 0);
    }

    let refreshed = unsafe {
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        )
    };
    if refreshed == 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    apply_theme_to_handle(hwnd, false);
    Ok(())
}

pub fn apply_theme(window: &WebviewWindow, theme: &str) -> Result<(), String> {
    let hwnd = native_handle(window).map_err(|error| error.to_string())?;
    apply_theme_to_handle(hwnd, theme == "graphite-dark");
    Ok(())
}

fn native_handle(
    window: &WebviewWindow,
) -> Result<windows_sys::Win32::Foundation::HWND, tauri::Error> {
    Ok(window.hwnd()?.0 as windows_sys::Win32::Foundation::HWND)
}

fn apply_theme_to_handle(hwnd: windows_sys::Win32::Foundation::HWND, dark: bool) {
    let dark_mode = i32::from(dark);
    set_dwm_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode);

    let caption_color = if dark { DARK_CAPTION } else { LIGHT_CAPTION };
    set_dwm_attribute(hwnd, DWMWA_CAPTION_COLOR, &caption_color);
    set_dwm_attribute(hwnd, DWMWA_TEXT_COLOR, &caption_color);
}

fn set_dwm_attribute<T>(hwnd: windows_sys::Win32::Foundation::HWND, attribute: i32, value: &T) {
    // Caption color attributes require Windows 11. Unsupported systems retain
    // their native title-bar colors without preventing the app from starting.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            value as *const T as *const c_void,
            size_of::<T>() as u32,
        );
    }
}

const fn color_ref(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_ref_uses_win32_bgr_layout() {
        assert_eq!(color_ref(0x12, 0x34, 0x56), 0x0056_3412);
    }

    #[test]
    fn native_caption_colors_match_the_app_themes() {
        assert_eq!(LIGHT_CAPTION, 0x00f7_f5f4);
        assert_eq!(DARK_CAPTION, 0x001c_1917);
    }
}
