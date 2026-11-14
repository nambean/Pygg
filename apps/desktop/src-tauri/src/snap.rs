//! Windows 11 snap layouts over the green window button (D12).
//!
//! Windows shows the snap layouts flyout only when its hit test (`WM_NCHITTEST`) answers `HTMAXBUTTON` under the
//! pointer. Pygg draws its own buttons in the web page, and WebView2 is a child window covering the whole client
//! area, so the main window's procedure never sees that hit test; subclassing it would not help, and WebView2's
//! own windows cannot be subclassed. So, as `tauri-plugin-snap-layout` and `tauri-plugin-frame` do, a small
//! invisible child window sits above the web view exactly over the green button and answers `HTMAXBUTTON`. It
//! never paints. It also takes the clicks there (the page no longer sees them) and maximizes or restores the
//! window itself, and it tells the page when the pointer enters or leaves so the button still looks hovered.
//!
//! The page sends the button's rectangle in physical pixels (CSS pixels times `devicePixelRatio`, which covers
//! display scaling and page zoom) on layout, resize and scale changes, or nothing when there is no button.
//! On macOS and Linux the command does nothing: macOS draws its own buttons, and Linux has no snap flyout.

use serde::Deserialize;

/// The event the page listens to: `true` when the pointer enters the green button, `false` when it leaves.
pub const HOVER_EVENT: &str = "window-snap-hover";

/// The green button's rectangle, in physical pixels from the window's client area's top left.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct SnapArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The largest side the overlay may have. The button is 12 CSS pixels; this leaves room for scaling and zoom
/// while a wrong rectangle can never cover the page.
const MAX_SIDE: i32 = 256;

/// The rectangle as whole pixels (x, y, width, height), or `None` when it is not a sensible button: not finite,
/// empty, off the window's top left, or larger than [`MAX_SIDE`].
pub fn pixels(area: &SnapArea) -> Option<(i32, i32, i32, i32)> {
    let SnapArea { x, y, width, height } = *area;
    if ![x, y, width, height].iter().all(|v| v.is_finite()) || x < 0.0 || y < 0.0 || x > 1e6 || y > 1e6 {
        return None;
    }
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let right = (x + width).ceil() as i32;
    let bottom = (y + height).ceil() as i32;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 || w > MAX_SIDE || h > MAX_SIDE || width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((left, top, w, h))
}

/// Places the snap overlay over the green button, or removes it when `area` is `None`.
#[tauri::command]
pub fn window_snap_area(window: tauri::WebviewWindow, area: Option<SnapArea>) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri::Manager;
        let rect = area.as_ref().and_then(pixels);
        let parent = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let app = window.app_handle().clone();
        let label = window.label().to_string();
        // Windows and their procedures belong to the thread that runs the event loop.
        window.run_on_main_thread(move || win::place(app, label, parent, rect)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    let _ = (window, area);
    Ok(())
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use tauri::{AppHandle, Emitter};
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::ValidateRect;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetAncestor, IsWindow, IsZoomed, LoadCursorW, PostMessageW, RegisterClassW,
        SetWindowPos, ShowWindow, GA_ROOT, HTMAXBUTTON, HWND_TOP, IDC_ARROW, SC_MAXIMIZE, SC_RESTORE, SWP_NOACTIVATE,
        SWP_SHOWWINDOW, SW_HIDE, WINDOW_EX_STYLE, WM_ERASEBKGND, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN,
        WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCRBUTTONDOWN, WM_NCRBUTTONUP, WM_PAINT, WM_SYSCOMMAND,
        WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    };

    /// The overlay of the one window that has custom buttons. Only the main thread touches it.
    struct Overlay {
        app: AppHandle,
        label: String,
        parent: isize,
        child: isize,
        hovering: bool,
        /// A press began on the overlay, so a release there is a click.
        pressed: bool,
    }

    thread_local! {
        static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
    }

    fn hwnd(raw: isize) -> HWND {
        HWND(raw as *mut core::ffi::c_void)
    }

    pub fn place(app: AppHandle, label: String, parent: isize, rect: Option<(i32, i32, i32, i32)>) {
        // The overlay of a window that is gone (or a different window) is dropped and made again.
        let current = OVERLAY.with_borrow(|o| o.as_ref().map(|o| (o.parent, o.child)));
        let child = match current {
            Some((p, c)) if p == parent && unsafe { IsWindow(Some(hwnd(c))) }.as_bool() => Some(c),
            _ => None,
        };
        let Some((x, y, w, h)) = rect else {
            if let Some(c) = child {
                unsafe {
                    let _ = ShowWindow(hwnd(c), SW_HIDE);
                }
                leave(c);
            }
            return;
        };
        let child = match child {
            Some(c) => c,
            None => match create(parent) {
                Some(c) => {
                    OVERLAY.set(Some(Overlay { app, label, parent, child: c, hovering: false, pressed: false }));
                    c
                }
                None => return,
            },
        };
        // On top of the web view's windows every time: they are siblings, and the web view may have moved up.
        unsafe {
            let _ = SetWindowPos(hwnd(child), Some(HWND_TOP), x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
    }

    fn create(parent: isize) -> Option<isize> {
        let class = w!("PyggSnapOverlay");
        unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            // Registering twice fails harmlessly; the first registration stays.
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                // No brush: the overlay never paints, so the button under it shows through.
                lpszClassName: class,
                ..Default::default()
            });
            // No WS_EX_LAYERED or WS_EX_TRANSPARENT: either makes the window transparent to the hit test.
            let child = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                None,
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(hwnd(parent)),
                None,
                Some(instance.into()),
                None,
            )
            .ok()?;
            Some(child.0 as isize)
        }
    }

    fn emit_hover(child: isize, on: bool) {
        let target = OVERLAY.with_borrow_mut(|o| match o {
            Some(o) if o.child == child && o.hovering != on => {
                o.hovering = on;
                if !on {
                    o.pressed = false;
                }
                Some((o.app.clone(), o.label.clone()))
            }
            _ => None,
        });
        if let Some((app, label)) = target {
            let _ = app.emit_to(label.as_str(), super::HOVER_EVENT, on);
        }
    }

    fn leave(child: isize) {
        emit_hover(child, false);
    }

    fn set_pressed(child: isize, on: bool) -> bool {
        OVERLAY.with_borrow_mut(|o| match o {
            Some(o) if o.child == child => std::mem::replace(&mut o.pressed, on),
            _ => false,
        })
    }

    unsafe extern "system" fn proc(window: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let raw = window.0 as isize;
        match msg {
            // The whole overlay is the maximize button: this is what makes Windows 11 offer snap layouts.
            WM_NCHITTEST => LRESULT(HTMAXBUTTON as isize),
            WM_NCMOUSEMOVE => {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: window,
                    dwHoverTime: 0,
                };
                let _ = unsafe { TrackMouseEvent(&mut track) };
                emit_hover(raw, true);
                LRESULT(0)
            }
            WM_NCMOUSELEAVE => {
                leave(raw);
                LRESULT(0)
            }
            // The default handling would track a caption button of its own; the press is only remembered.
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                set_pressed(raw, true);
                LRESULT(0)
            }
            WM_NCLBUTTONUP => {
                if set_pressed(raw, false) {
                    let root = unsafe { GetAncestor(window, GA_ROOT) };
                    let command = if unsafe { IsZoomed(root) }.as_bool() { SC_RESTORE } else { SC_MAXIMIZE };
                    let _ = unsafe { PostMessageW(Some(root), WM_SYSCOMMAND, WPARAM(command as usize), LPARAM(0)) };
                }
                LRESULT(0)
            }
            // No window menu from a right click on the button, as on the page.
            WM_NCRBUTTONDOWN | WM_NCRBUTTONUP => LRESULT(0),
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let _ = unsafe { ValidateRect(Some(window), None) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(window, msg, wparam, lparam) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: f64, y: f64, width: f64, height: f64) -> SnapArea {
        SnapArea { x, y, width, height }
    }

    #[test]
    fn a_button_at_any_scale_becomes_whole_pixels_that_cover_it() {
        // 12 CSS pixels at (60, 20): 100%, 150% and 175% scaling.
        assert_eq!(pixels(&area(60.0, 20.0, 12.0, 12.0)), Some((60, 20, 12, 12)));
        assert_eq!(pixels(&area(90.0, 30.0, 18.0, 18.0)), Some((90, 30, 18, 18)));
        assert_eq!(pixels(&area(105.0, 35.0, 21.0, 21.0)), Some((105, 35, 21, 21)));
        // Fractions round outwards.
        assert_eq!(pixels(&area(60.4, 20.6, 12.2, 12.0)), Some((60, 20, 13, 13)));
    }

    #[test]
    fn a_rectangle_that_is_not_a_button_places_nothing() {
        assert_eq!(pixels(&area(0.0, 0.0, 0.0, 12.0)), None);
        assert_eq!(pixels(&area(10.0, 10.0, -5.0, 12.0)), None);
        assert_eq!(pixels(&area(-1.0, 10.0, 12.0, 12.0)), None);
        assert_eq!(pixels(&area(f64::NAN, 10.0, 12.0, 12.0)), None);
        assert_eq!(pixels(&area(10.0, 10.0, f64::INFINITY, 12.0)), None);
        // A wrong rectangle must never cover the page.
        assert_eq!(pixels(&area(0.0, 0.0, 1440.0, 900.0)), None);
    }
}
