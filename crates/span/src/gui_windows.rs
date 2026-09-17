//! Native Windows settings window. Workers own data only; all HWND access stays on the UI thread.
#![allow(unsafe_op_in_unsafe_fn)]
use super::io;
use span_core::{DeviceId, DeviceInfo, TrustState};
use std::{cell::RefCell, ptr, sync::mpsc, time::Duration};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{COLOR_WINDOW, CreateFontW, DeleteObject, HFONT},
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, Input::KeyboardAndMouse::EnableWindow, WindowsAndMessaging::*},
};

const ADD: u16 = 1001;
const REMOVE: u16 = 1002;
const LIST: u16 = 1003;
const TIMER: usize = 1;
const WIDTH: i32 = 580;
const HEIGHT: i32 = 530;
const STYLE: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

struct Control {
    hwnd: HWND,
    rect: (i32, i32, i32, i32),
    heading: bool,
}
struct State {
    controls: Vec<Control>,
    fonts: Vec<HFONT>,
    dpi: u32,
    list: HWND,
    empty: HWND,
    add: HWND,
    remove: HWND,
    status: HWND,
    devices: Vec<DeviceInfo>,
    receiver: Option<mpsc::Receiver<io::Result<Outcome>>>,
    busy: bool,
}
enum Outcome {
    Discovered(Vec<DeviceInfo>),
    Done(String),
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn prompt_pairing(device_id: &str, name: &str, platform: &str) -> io::Result<()> {
    let id = DeviceId::new(device_id.to_owned())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid device id"))?;
    unsafe {
        if confirm(
            ptr::null_mut(),
            &format!("{name}（{platform}）请求连接 Span。\r\n\r\n是否信任此设备并开启剪贴板同步？"),
        ) {
            let mut store =
                crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
            store.trust_existing(&id)?;
            alert(
                ptr::null_mut(),
                &format!("已信任 {name}，剪贴板同步已开启。"),
                MB_OK,
            );
        }
    }
    Ok(())
}

pub fn open() -> io::Result<()> {
    let local = crate::config::load_or_create_local_device()?;
    let autostart_error = crate::autostart::install().err();
    let daemon_error = crate::daemon_control::start_daemon().err();
    unsafe {
        // Thread-local context also works if another entry point already set process awareness.
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let result = run_window(&local, autostart_error, daemon_error);
        if !previous.is_null() {
            SetThreadDpiAwarenessContext(previous);
        }
        result
    }
}

unsafe fn run_window(
    local: &crate::config::LocalDevice,
    autostart_error: Option<io::Error>,
    daemon_error: Option<io::Error>,
) -> io::Result<()> {
    let instance = GetModuleHandleW(ptr::null());
    if instance.is_null() {
        return Err(io::Error::last_os_error());
    }
    let class_name = wide("SpanGuiWindow");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        hbrBackground: (COLOR_WINDOW + 1) as usize as _,
        hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
        ..std::mem::zeroed()
    };
    RegisterClassW(&class);
    let hwnd = CreateWindowExW(
        WS_EX_CONTROLPARENT,
        class_name.as_ptr(),
        wide("Span · 跨设备剪贴板").as_ptr(),
        STYLE,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        WIDTH,
        HEIGHT,
        ptr::null_mut(),
        ptr::null_mut(),
        instance,
        ptr::null(),
    );
    if hwnd.is_null() {
        return Err(io::Error::last_os_error());
    }
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            controls: vec![],
            fonts: vec![],
            dpi: GetDpiForWindow(hwnd).max(96),
            list: ptr::null_mut(),
            empty: ptr::null_mut(),
            add: ptr::null_mut(),
            remove: ptr::null_mut(),
            status: ptr::null_mut(),
            devices: vec![],
            receiver: None,
            busy: false,
        })
    });
    if let Err(error) = create_controls(hwnd, local) {
        DestroyWindow(hwnd);
        return Err(error);
    }
    layout(hwnd, None);
    if SetTimer(hwnd, TIMER, 100, None) == 0 {
        let error = io::Error::last_os_error();
        DestroyWindow(hwnd);
        return Err(error);
    }
    if let Err(error) = refresh() {
        set_status(&format!("读取可信设备失败：{error}"));
    }
    ShowWindow(hwnd, SW_SHOW);
    if let Some(error) = daemon_error {
        set_status("后台同步启动失败，请重新打开 Span 重试。");
        alert(
            hwnd,
            &format!(
                "后台同步启动失败，当前无法保证剪贴板同步。\r\n\r\n{error}\r\n\r\n请重新打开 Span 重试。"
            ),
            MB_OK | MB_ICONERROR,
        );
    }
    if let Some(error) = autostart_error {
        alert(
            hwnd,
            &format!("未能设置开机自启，仍可使用此窗口。\r\n\r\n{error}"),
            MB_OK | MB_ICONWARNING,
        );
    }
    let mut message: MSG = std::mem::zeroed();
    loop {
        match GetMessageW(&mut message, ptr::null_mut(), 0, 0) {
            -1 => {
                let error = io::Error::last_os_error();
                DestroyWindow(hwnd);
                return Err(error);
            }
            0 => break,
            _ => {
                if IsDialogMessageW(hwnd, &message) == 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND => {
            let id = (wparam & 0xffff) as u16;
            let code = (wparam >> 16) as u32;
            if id == LIST && code == LBN_SELCHANGE {
                update_controls();
            } else if code == BN_CLICKED && (id == ADD || id == REMOVE) {
                start_action(hwnd, id);
            }
            0
        }
        WM_TIMER if wparam == TIMER => {
            poll_result(hwnd);
            0
        }
        WM_DPICHANGED => {
            STATE.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    s.dpi = (wparam & 0xffff) as u32;
                }
            });
            layout(hwnd, Some(*(lparam as *const RECT)));
            0
        }
        WM_CLOSE => {
            // Keep the window/receiver alive until a committed write has completed.
            let busy = STATE.with(|s| s.borrow().as_ref().is_some_and(|s| s.busy));
            if busy {
                set_status("正在处理设备操作，请稍候再关闭窗口。");
            } else {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER);
            // Dropping the receiver safely discards any late worker result (no HWND or raw payload).
            STATE.with(|s| {
                if let Some(s) = s.borrow_mut().take() {
                    for font in s.fonts {
                        DeleteObject(font);
                    }
                }
            });
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn control(
    parent: HWND,
    class: &str,
    text: &str,
    rect: (i32, i32, i32, i32),
    id: u16,
    style: u32,
    heading: bool,
) -> io::Result<HWND> {
    let hwnd = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        0,
        0,
        parent,
        id as usize as _,
        GetModuleHandleW(ptr::null()),
        ptr::null(),
    );
    if hwnd.is_null() {
        return Err(io::Error::last_os_error());
    }
    STATE.with(|s| {
        s.borrow_mut().as_mut().unwrap().controls.push(Control {
            hwnd,
            rect,
            heading,
        })
    });
    Ok(hwnd)
}
unsafe fn create_controls(hwnd: HWND, local: &crate::config::LocalDevice) -> io::Result<()> {
    control(hwnd, "STATIC", "Span", (28, 20, 520, 38), 0, 0, true)?;
    control(
        hwnd,
        "STATIC",
        "在可信设备之间自动同步文本剪贴板",
        (28, 62, 520, 26),
        0,
        0,
        false,
    )?;
    control(hwnd, "STATIC", "本机", (28, 106, 520, 28), 0, 0, true)?;
    control(
        hwnd,
        "STATIC",
        &format!("{} · Windows", local.name),
        (28, 142, 520, 26),
        0,
        0x00004000, /* SS_ENDELLIPSIS */
        false,
    )?;
    control(
        hwnd,
        "STATIC",
        "已添加可信设备",
        (28, 188, 520, 28),
        0,
        0,
        true,
    )?;
    control(
        hwnd,
        "STATIC",
        "仅向可信设备同步；已添加不代表设备当前在线。",
        (28, 224, 520, 25),
        0,
        0,
        false,
    )?;
    let list = control(
        hwnd,
        "LISTBOX",
        "",
        (28, 258, 520, 112),
        LIST,
        WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
        false,
    )?;
    let empty = control(
        hwnd,
        "STATIC",
        "暂无可信设备\r\n点击“添加设备…”以发现并信任同一网络中的设备。",
        (40, 276, 490, 66),
        0,
        0,
        false,
    )?;
    let add = control(
        hwnd,
        "BUTTON",
        "添加设备…",
        (28, 386, 200, 36),
        ADD,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
        false,
    )?;
    let remove = control(
        hwnd,
        "BUTTON",
        "移除设备",
        (348, 386, 200, 36),
        REMOVE,
        WS_TABSTOP | BS_PUSHBUTTON as u32,
        false,
    )?;
    let status = control(
        hwnd,
        "STATIC",
        "仅向可信设备同步剪贴板。",
        (28, 436, 520, 44),
        0,
        0,
        false,
    )?;
    control(
        hwnd,
        "STATIC",
        "关闭窗口后，后台仍会继续同步。",
        (28, 492, 520, 24),
        0,
        0,
        false,
    )?;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let s = s.as_mut().unwrap();
        s.list = list;
        s.empty = empty;
        s.add = add;
        s.remove = remove;
        s.status = status;
    });
    Ok(())
}
fn scale(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32
}
unsafe fn layout(hwnd: HWND, suggested: Option<RECT>) {
    // Do not hold a RefCell borrow across SetWindowPos (it can dispatch window messages).
    let dpi = STATE.with(|s| s.borrow().as_ref().map_or(96, |s| s.dpi));
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: scale(WIDTH, dpi),
        bottom: scale(HEIGHT, dpi),
    };
    AdjustWindowRectExForDpi(&mut rect, STYLE, 0, WS_EX_CONTROLPARENT, dpi);
    let (x, y, flags) = suggested.map_or((0, 0, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE), |r| {
        (r.left, r.top, SWP_NOZORDER | SWP_NOACTIVATE)
    });
    SetWindowPos(
        hwnd,
        ptr::null_mut(),
        x,
        y,
        rect.right - rect.left,
        rect.bottom - rect.top,
        flags,
    );
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(s) = state.as_mut() else { return };
        let normal = CreateFontW(
            -scale(15, dpi),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        let heading = CreateFontW(
            -scale(20, dpi),
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );
        for c in &s.controls {
            let (x, y, w, h) = c.rect;
            MoveWindow(
                c.hwnd,
                scale(x, dpi),
                scale(y, dpi),
                scale(w, dpi),
                scale(h, dpi),
                1,
            );
            SendMessageW(
                c.hwnd,
                WM_SETFONT,
                if c.heading { heading } else { normal } as usize,
                1,
            );
        }
        for font in s.fonts.drain(..) {
            DeleteObject(font);
        }
        s.fonts = vec![normal, heading];
    });
}

unsafe fn refresh() -> io::Result<()> {
    let store = crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
    let devices: Vec<_> = store.trusted_devices().into_iter().cloned().collect();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let s = state.as_mut().unwrap();
        let index = SendMessageW(s.list, LB_GETCURSEL, 0, 0);
        let selected = usize::try_from(index)
            .ok()
            .and_then(|i| s.devices.get(i))
            .map(|d| d.id.clone());
        SendMessageW(s.list, LB_RESETCONTENT, 0, 0);
        for device in &devices {
            let text = wide(&format!(
                "{} · {}",
                device.name,
                crate::config::platform_name(device.platform)
            ));
            SendMessageW(s.list, LB_ADDSTRING, 0, text.as_ptr() as LPARAM);
        }
        if let Some(index) = devices
            .iter()
            .position(|d| Some(&d.id) == selected.as_ref())
        {
            SendMessageW(s.list, LB_SETCURSEL, index, 0);
        }
        s.devices = devices;
        ShowWindow(
            s.list,
            if s.devices.is_empty() {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        ShowWindow(
            s.empty,
            if s.devices.is_empty() {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        SetWindowTextW(
            s.add,
            wide(if s.devices.is_empty() {
                "添加设备…"
            } else {
                "添加另一台设备…"
            })
            .as_ptr(),
        );
    });
    update_controls();
    Ok(())
}
unsafe fn update_controls() {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(s) = state.as_ref() else { return };
        let selected = SendMessageW(s.list, LB_GETCURSEL, 0, 0);
        EnableWindow(s.add, (!s.busy) as i32);
        EnableWindow(s.list, (!s.busy && !s.devices.is_empty()) as i32);
        EnableWindow(
            s.remove,
            (!s.busy && selected >= 0 && (selected as usize) < s.devices.len()) as i32,
        );
    });
}
unsafe fn set_status(text: &str) {
    STATE.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            SetWindowTextW(s.status, wide(text).as_ptr());
        }
    });
}
unsafe fn set_busy(busy: bool) {
    STATE.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.busy = busy;
        }
    });
    update_controls();
}
unsafe fn launch(work: impl FnOnce() -> io::Result<Outcome> + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    STATE.with(|s| s.borrow_mut().as_mut().unwrap().receiver = Some(receiver));
    if let Err(error) = std::thread::Builder::new()
        .name("span-gui-action".into())
        .spawn(move || {
            let _ = sender.send(work());
        })
    {
        STATE.with(|s| s.borrow_mut().as_mut().unwrap().receiver = None);
        set_busy(false);
        set_status(&format!("无法启动操作：{error}"));
    }
}
unsafe fn start_action(hwnd: HWND, id: u16) {
    if STATE.with(|s| s.borrow().as_ref().is_none_or(|s| s.busy)) {
        return;
    }
    if id == ADD {
        set_busy(true);
        set_status("正在发现同一网络中的设备…");
        launch(|| {
            let local = crate::config::load_or_create_local_device()?;
            let devices = crate::scan_devices(&local, Duration::from_millis(700))?;
            Ok(Outcome::Discovered(
                devices
                    .into_iter()
                    .filter(|d| d.trust_state != TrustState::Trusted && d.id != local.id)
                    .collect(),
            ))
        });
    } else {
        // Snapshot the displayed device ID on the UI thread, never re-index a newly loaded store.
        let device = STATE.with(|s| {
            let s = s.borrow();
            let s = s.as_ref().unwrap();
            usize::try_from(SendMessageW(s.list, LB_GETCURSEL, 0, 0))
                .ok()
                .and_then(|i| s.devices.get(i))
                .cloned()
        });
        let Some(device) = device else { return };
        set_busy(true);
        if !confirm(
            hwnd,
            &format!(
                "移除设备“{}”？\r\n\r\n移除后将停止与此设备同步剪贴板。再次同步需要重新添加并信任。",
                device.name
            ),
        ) {
            set_busy(false);
            set_status("已取消移除。");
            return;
        }
        set_status("正在移除设备…");
        launch(move || {
            let mut store =
                crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
            store.revoke(&device.id)?;
            Ok(Outcome::Done(format!("已移除 {}。", device.name)))
        });
    }
}
unsafe fn poll_result(hwnd: HWND) {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let s = state.as_mut()?;
        let result = match s.receiver.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(io::Error::other("后台操作意外结束，请重试。"))
            }
        };
        s.receiver = None;
        Some(result)
    });
    let Some(result) = result else { return };
    match result {
        Ok(Outcome::Discovered(devices)) => {
            if devices.is_empty() {
                set_status("没有发现新设备。请确认另一台设备已打开 Span，且位于同一网络。");
                set_busy(false);
                return;
            }
            let mut accepted = vec![];
            for device in devices {
                if confirm(
                    hwnd,
                    &format!(
                        "添加设备“{}”（{}）？\r\n\r\n设备标识：{}\r\n\r\n仅信任你认识的设备。信任后将开启剪贴板同步。",
                        device.name,
                        crate::config::platform_name(device.platform),
                        device.id
                    ),
                ) {
                    accepted.push(device.id);
                }
            }
            if accepted.is_empty() {
                set_status("已取消添加，未共享剪贴板内容。");
                set_busy(false);
                return;
            }
            set_status("正在添加可信设备…");
            launch(move || {
                let mut store =
                    crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
                for id in &accepted {
                    store.trust_existing(id)?;
                }
                Ok(Outcome::Done(format!(
                    "已添加 {} 台可信设备。",
                    accepted.len()
                )))
            });
        }
        result => {
            let refresh_result = refresh();
            set_busy(false);
            match result {
                Ok(Outcome::Done(text)) => set_status(&text),
                Err(error) => {
                    let text = format!("操作失败：{error}");
                    set_status(&text);
                    alert(hwnd, &text, MB_OK | MB_ICONERROR);
                }
                _ => unreachable!(),
            }
            if let Err(error) = refresh_result {
                set_status(&format!("刷新设备列表失败：{error}"));
            }
        }
    }
}
unsafe fn confirm(hwnd: HWND, text: &str) -> bool {
    // Default to No for both trust and destructive operations.
    alert(hwnd, text, MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES
}
unsafe fn alert(hwnd: HWND, text: &str, flags: u32) -> i32 {
    MessageBoxW(hwnd, wide(text).as_ptr(), wide("Span").as_ptr(), flags)
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpi_scaling_preserves_logical_layout() {
        assert_eq!(scale(WIDTH, 96), WIDTH);
        assert_eq!(scale(WIDTH, 144), 870);
        assert_eq!(scale(HEIGHT, 192), 1060);
    }
    #[test]
    fn windows_text_is_utf16_and_terminated() {
        let text = wide("添加设备…");
        assert_eq!(text.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&text[..text.len() - 1]).unwrap(),
            "添加设备…"
        );
    }
}
