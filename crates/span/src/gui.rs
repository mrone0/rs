//! Small native device manager.
//!
//! The daemon remains completely headless. This module is only the optional
//! user-facing window opened by `span` (or `span gui`). No web runtime,
//! Electron, Tauri, GTK, or other GUI framework is bundled.

use std::io;

#[cfg(target_os = "macos")]
mod macos {
    #![allow(unsafe_op_in_unsafe_fn)]
    use super::io;
    use std::ffi::{CString, c_char, c_void};
    use std::sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    use span_core::TrustState;

    use crate::config::{load_or_create_local_device, platform_name, trust_store_path};
    use crate::daemon_control::start_daemon;
    use crate::trust_store::TrustStore;

    type Id = *mut c_void;
    type Sel = *mut c_void;
    type NSInteger = isize;
    type NSUInteger = usize;
    type BOOL = i8;
    type IMP = *const c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        fn NSApplicationLoad() -> BOOL;
    }


    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
        fn class_addMethod(cls: Id, name: Sel, imp: IMP, types: *const c_char) -> BOOL;
        fn objc_registerClassPair(cls: Id);
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    static STATUS_LABEL: OnceLock<usize> = OnceLock::new();
    static TRUSTED_SUMMARY_LABEL: OnceLock<usize> = OnceLock::new();
    static TRUSTED_POPUP: OnceLock<usize> = OnceLock::new();
    static CONTROLLER_CLASS: OnceLock<usize> = OnceLock::new();
    static CONTROLLER_INSTANCE: OnceLock<usize> = OnceLock::new();
    static ACTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

    pub fn open() -> io::Result<()> {
        // The GUI is the normal entry point: make the daemon and auto-start
        // available without exposing setup controls to ordinary users.
        let _ = crate::autostart::install();
        let _ = start_daemon();
        unsafe {
            if NSApplicationLoad() == 0 {
                return Err(io::Error::other("could not load AppKit"));
            }
            let pool = objc_autoreleasePoolPush();
            let result = build_and_run();
            objc_autoreleasePoolPop(pool);
            result
        }
    }

    pub fn prompt_pairing(device_id: &str, name: &str, platform: &str) -> io::Result<()> {
        unsafe {
            if NSApplicationLoad() == 0 {
                return Err(io::Error::other("could not load AppKit"));
            }
            let pool = objc_autoreleasePoolPush();
            let app = send_id(class("NSApplication")?, sel("sharedApplication")?);
            send_void_integer(app, sel("setActivationPolicy:")?, 0);
            send_void_bool(app, sel("activateIgnoringOtherApps:")?, 1);

            let message = format!(
                "{}（{}）请求连接 Span。\n\n是否信任此设备并开启剪贴板同步？",
                name, platform
            );
            let accepted = confirm_accept(&message);
            if accepted {
                let id = span_core::DeviceId::new(device_id.to_string()).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid device id")
                })?;
                let mut store = TrustStore::load(trust_store_path()?)?;
                store.trust_existing(&id)?;
                show_alert(&format!("已信任 {name}，剪贴板同步已开启。"));
            }
            objc_autoreleasePoolPop(pool);
        }
        Ok(())
    }

    unsafe fn build_and_run() -> io::Result<()> {
        let app = send_id(class("NSApplication")?, sel("sharedApplication")?);
        if app.is_null() {
            return Err(io::Error::other("NSApplication unavailable"));
        }
        send_void_integer(app, sel("setActivationPolicy:")?, 0);

        let local = load_or_create_local_device()?;
        let store = TrustStore::load(trust_store_path()?)?;
        let trusted_count = store.trusted_devices().len();
        let devices = store.devices().to_vec();

        let controller_class = controller_class()?;
        let controller = send_id(controller_class, sel("new")?);
        if controller.is_null() {
            return Err(io::Error::other("could not create GUI controller"));
        }
        let _ = CONTROLLER_INSTANCE.set(controller as usize);
        send_void_id(app, sel("setDelegate:")?, controller);

        let rect = Rect {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size {
                width: 520.0,
                height: 360.0,
            },
        };
        let window = send_id_rect_integer_integer_bool(
            send_id(class("NSWindow")?, sel("alloc")?),
            sel("initWithContentRect:styleMask:backing:defer:")?,
            rect,
            1 | 2 | 4 | 8,
            2,
            0,
        );
        if window.is_null() {
            return Err(io::Error::other("could not create Span window"));
        }

        send_void_id(window, sel("setTitle:")?, ns_string("Span"));
        send_void(window, sel("center")?);
        let content = send_id(window, sel("contentView")?);
        let background = send_id(content, sel("window")?);
        let _ = background;

        add_label(
            content,
            "SPAN",
            Rect {
                origin: Point { x: 28.0, y: 305.0 },
                size: Size {
                    width: 450.0,
                    height: 30.0,
                },
            },
            24.0,
            true,
        )?;
        add_label(
            content,
            "在可信设备之间自动同步剪贴板",
            Rect {
                origin: Point { x: 30.0, y: 280.0 },
                size: Size {
                    width: 450.0,
                    height: 20.0,
                },
            },
            13.0,
            false,
        )?;
        add_label(
            content,
            &format!("本机：{}", local.name),
            Rect {
                origin: Point { x: 30.0, y: 250.0 },
                size: Size {
                    width: 450.0,
                    height: 22.0,
                },
            },
            13.0,
            true,
        )?;
        let mut rows = format!("可信设备（{trusted_count}）\n");
        for device in devices
            .iter()
            .filter(|d| d.trust_state == TrustState::Trusted)
        {
            rows.push_str(&format!(
                "• {} · {}\n",
                device.name,
                platform_name(device.platform)
            ));
        }
        if trusted_count == 0 {
            rows.push_str("暂无设备，请点击“发现设备”进行配对。");
        }
        let trusted_summary = add_label(
            content,
            &rows,
            Rect {
                origin: Point { x: 30.0, y: 125.0 },
                size: Size {
                    width: 450.0,
                    height: 105.0,
                },
            },
            13.0,
            false,
        )?;
        let _ = TRUSTED_SUMMARY_LABEL.set(trusted_summary as usize);

        let status = add_label(
            content,
            "运行中 · 仅向可信设备同步剪贴板",
            Rect {
                origin: Point { x: 30.0, y: 92.0 },
                size: Size {
                    width: 450.0,
                    height: 20.0,
                },
            },
            11.0,
            false,
        )?;
        let _ = STATUS_LABEL.set(status as usize);

        add_button(
            content,
            controller,
            "发现设备",
            "spanDiscover:",
            1,
            Rect {
                origin: Point { x: 30.0, y: 35.0 },
                size: Size {
                    width: 150.0,
                    height: 34.0,
                },
            },
        )?;
        add_button(
            content,
            controller,
            "移除设备",
            "spanRemove:",
            2,
            Rect {
                origin: Point { x: 195.0, y: 35.0 },
                size: Size {
                    width: 150.0,
                    height: 34.0,
                },
            },
        )?;
        let popup = add_trusted_popup(
            content,
            &devices,
            Rect {
                origin: Point { x: 360.0, y: 35.0 },
                size: Size {
                    width: 130.0,
                    height: 34.0,
                },
            },
        )?;
        let _ = TRUSTED_POPUP.set(popup as usize);
        send_void_id(window, sel("makeKeyAndOrderFront:")?, app);
        send_void_bool(app, sel("activateIgnoringOtherApps:")?, 1);
        send_void(app, sel("run")?);
        Ok(())
    }

    unsafe fn add_label(
        parent: Id,
        value: &str,
        frame: Rect,
        size: f64,
        bold: bool,
    ) -> io::Result<Id> {
        let label = send_id_id(
            class("NSTextField")?,
            sel("labelWithString:")?,
            ns_string(value),
        );
        if label.is_null() {
            return Err(io::Error::other("could not create label"));
        }
        send_void_rect(label, sel("setFrame:")?, frame);
        let font_class = class("NSFont")?;
        let font_selector = if bold {
            "boldSystemFontOfSize:"
        } else {
            "systemFontOfSize:"
        };
        let font = send_id_double(font_class, sel(font_selector)?, size);
        send_void_id(label, sel("setFont:")?, font);
        send_void_id(parent, sel("addSubview:")?, label);
        Ok(label)
    }

    unsafe fn add_trusted_popup(
        parent: Id,
        devices: &[span_core::DeviceInfo],
        frame: Rect,
    ) -> io::Result<Id> {
        let popup = send_id_rect_bool(
            send_id(class("NSPopUpButton")?, sel("alloc")?),
            sel("initWithFrame:pullsDown:")?,
            frame,
            0,
        );
        if popup.is_null() {
            return Err(io::Error::other("could not create trusted-device selector"));
        }
        let trusted = devices
            .iter()
            .filter(|device| device.trust_state == TrustState::Trusted);
        let mut count = 0;
        for device in trusted {
            send_void_id(
                popup,
                sel("addItemWithTitle:")?,
                ns_string(&format!("移除 {}", device.name)),
            );
            count += 1;
        }
        if count == 0 {
            send_void_id(
                popup,
                sel("addItemWithTitle:")?,
                ns_string("暂无可信设备"),
            );
        }
        send_void_id(parent, sel("addSubview:")?, popup);
        Ok(popup)
    }

    unsafe fn add_button(
        parent: Id,
        target: Id,
        title: &str,
        action: &str,
        tag: NSInteger,
        frame: Rect,
    ) -> io::Result<Id> {
        let button = send_id_id_id_id(
            class("NSButton")?,
            sel("buttonWithTitle:target:action:")?,
            ns_string(title),
            target,
            sel(action)?,
        );
        if button.is_null() {
            return Err(io::Error::other("could not create button"));
        }
        send_void_rect(button, sel("setFrame:")?, frame);
        send_void_integer(button, sel("setTag:")?, tag);
        send_void_id(parent, sel("addSubview:")?, button);
        Ok(button)
    }

    unsafe fn controller_class() -> io::Result<Id> {
        if let Some(value) = CONTROLLER_CLASS.get() {
            return Ok(class_id(*value));
        }
        let superclass = class("NSObject")?;
        let name = CString::new("SpanGuiController").unwrap();
        let cls = objc_allocateClassPair(superclass, name.as_ptr(), 0);
        if cls.is_null() {
            return Err(io::Error::other("could not register GUI controller"));
        }
        let type_encoding = CString::new("v@:@").unwrap();
        for (name, callback) in [
            ("spanDiscover:", action_discover as IMP),
            ("spanRemove:", action_remove as IMP),
            ("spanCompleteDiscover:", complete_discover as IMP),
            ("spanCompleteRemove:", complete_remove as IMP),
        ] {
            let selector = sel(name)?;
            if class_addMethod(cls, selector, callback, type_encoding.as_ptr()) == 0 {
                return Err(io::Error::other(format!("could not add GUI action {name}")));
            }
        }
        let bool_encoding = CString::new("c@:@").unwrap();
        if class_addMethod(
            cls,
            sel("applicationShouldTerminateAfterLastWindowClosed:")?,
            application_should_terminate_after_last_window_closed
                as unsafe extern "C" fn(Id, Sel, Id) -> BOOL as IMP,
            bool_encoding.as_ptr(),
        ) == 0
        {
            return Err(io::Error::other("could not add GUI close action"));
        }
        objc_registerClassPair(cls);
        let _ = CONTROLLER_CLASS.set(cls as usize);
        Ok(cls)
    }

    unsafe extern "C" fn action_discover(_: Id, _: Sel, _: Id) {
        if ACTION_IN_PROGRESS.swap(true, Ordering::AcqRel) { return; }
        set_status("正在发现设备…");
        std::thread::spawn(|| {
            let result = load_or_create_local_device()
                .and_then(|local| crate::scan_devices(&local, Duration::from_millis(700)))
                .map(|_| "")
                .unwrap_or("操作失败，请检查网络设置。");
            let Some(controller) = CONTROLLER_INSTANCE.get().copied() else {
                ACTION_IN_PROGRESS.store(false, Ordering::Release);
                return;
            };
            unsafe {
                send_void_id_bool(
                    controller as Id,
                    sel("performSelectorOnMainThread:withObject:waitUntilDone:").unwrap(),
                    sel("spanCompleteDiscover:").unwrap() as Id,
                    ns_string(result),
                    0,
                );
            }
        });
    }

    unsafe extern "C" fn complete_discover(_: Id, _: Sel, error_message: Id) {
        let result = (|| -> io::Result<String> {
            if send_u64(error_message, sel("length")?) > 0 {
                return Ok("发现失败，请检查网络设置。".into());
            }
            let store = TrustStore::load(trust_store_path()?)?;
            refresh_trusted_controls(&store)?;
            let available: Vec<_> = store.devices().iter()
                .filter(|device| device.trust_state != TrustState::Trusted).collect();
            if available.is_empty() { return Ok("没有发现新设备。可信设备会自动同步。".into()); }
            let names = available.iter()
                .map(|device| format!("{} ({})", device.name, platform_name(device.platform)))
                .collect::<Vec<_>>().join("\n");
            if !confirm_accept(&format!("发现新设备：\n\n{names}\n\n是否信任并开启剪贴板同步？")) {
                return Ok("已取消配对，未共享剪贴板内容。".into());
            }
            let mut store = TrustStore::load(trust_store_path()?)?;
            let mut accepted = 0;
            for device in &available { if store.trust_existing(&device.id)? { accepted += 1; } }
            refresh_trusted_controls(&store)?;
            Ok(format!("已信任 {} 台设备，剪贴板同步已开启。", accepted))
        })();
        set_status(&result.unwrap_or_else(|error| format!("操作失败：{error}")));
        ACTION_IN_PROGRESS.store(false, Ordering::Release);
    }
    unsafe extern "C" fn action_remove(_: Id, _: Sel, _: Id) {
        if ACTION_IN_PROGRESS.swap(true, Ordering::AcqRel) { return; }
        let result = (|| -> io::Result<(span_core::DeviceId, String)> {
            let store = TrustStore::load(trust_store_path()?)?;
            let trusted = store.trusted_devices();
            let Some(popup) = TRUSTED_POPUP.get().copied() else {
                return Err(io::Error::other("暂无可移除的可信设备。"));
            };
            let index = send_integer(popup as Id, sel("indexOfSelectedItem")?);
            let Some(device) = trusted.get(index.max(0) as usize) else {
                return Err(io::Error::other("暂无可移除的可信设备。"));
            };
            Ok((device.id.clone(), device.name.clone()))
        })();
        let Ok((id, name)) = result else {
            set_status("暂无可移除的可信设备。");
            ACTION_IN_PROGRESS.store(false, Ordering::Release);
            return;
        };
        if !confirm_accept(&format!("是否移除可信设备“{name}”？")) {
            set_status("已取消移除。");
            ACTION_IN_PROGRESS.store(false, Ordering::Release);
            return;
        }
        set_status("正在移除设备…");
        std::thread::spawn(move || {
            let message = trust_store_path()
                .and_then(TrustStore::load)
                .and_then(|mut store| { store.revoke(&id)?; Ok(format!("已移除 {name}。")) })
                .unwrap_or_else(|error| format!("操作失败：{error}"));
            let Some(controller) = CONTROLLER_INSTANCE.get().copied() else {
                ACTION_IN_PROGRESS.store(false, Ordering::Release);
                return;
            };
            unsafe {
                send_void_id_bool(
                    controller as Id,
                    sel("performSelectorOnMainThread:withObject:waitUntilDone:").unwrap(),
                    sel("spanCompleteRemove:").unwrap() as Id,
                    ns_string(&message),
                    0,
                );
            }
        });
    }

    unsafe extern "C" fn complete_remove(_: Id, _: Sel, message: Id) {
        if let Ok(store) = trust_store_path().and_then(TrustStore::load) {
            let _ = refresh_trusted_controls(&store);
        }
        let value = ns_string_to_string(message).unwrap_or_else(|| "操作完成。".into());
        set_status(&value);
        ACTION_IN_PROGRESS.store(false, Ordering::Release);
    }

    unsafe fn refresh_trusted_controls(store: &TrustStore) -> io::Result<()> {
        let trusted = store.trusted_devices();

        if let Some(pointer) = TRUSTED_SUMMARY_LABEL.get() {
            let mut rows = format!("可信设备（{}）\n", trusted.len());
            for device in &trusted {
                rows.push_str(&format!(
                    "• {} · {}\n",
                    device.name,
                    platform_name(device.platform)
                ));
            }
            if trusted.is_empty() {
                rows.push_str("暂无设备，请点击“发现设备”进行配对。");
            }
            send_void_id(*pointer as Id, sel("setStringValue:")?, ns_string(&rows));
        }

        if let Some(pointer) = TRUSTED_POPUP.get() {
            let popup = *pointer as Id;
            send_void(popup, sel("removeAllItems")?);
            for device in &trusted {
                send_void_id(
                    popup,
                    sel("addItemWithTitle:")?,
                    ns_string(&format!("移除 {}", device.name)),
                );
            }
            if trusted.is_empty() {
                send_void_id(
                    popup,
                    sel("addItemWithTitle:")?,
                    ns_string("暂无可信设备"),
                );
            }
            send_void_integer(popup, sel("selectItemAtIndex:")?, 0);
        }

        Ok(())
    }

    unsafe extern "C" fn application_should_terminate_after_last_window_closed(
        _: Id,
        _: Sel,
        _: Id,
    ) -> BOOL {
        1
    }

    unsafe fn confirm_accept(message: &str) -> bool {
        let alert = send_id(class("NSAlert").unwrap(), sel("new").unwrap());
        send_void_id(alert, sel("setMessageText:").unwrap(), ns_string("Span"));
        send_void_id(
            alert,
            sel("setInformativeText:").unwrap(),
            ns_string(message),
        );
        send_id_id(
            alert,
            sel("addButtonWithTitle:").unwrap(),
            ns_string("信任"),
        );
        send_id_id(
            alert,
            sel("addButtonWithTitle:").unwrap(),
            ns_string("暂不"),
        );
        send_integer(alert, sel("runModal").unwrap()) == 1000
    }

    fn set_status(value: &str) {
        if let Some(pointer) = STATUS_LABEL.get() {
            unsafe {
                send_void_id(
                    *pointer as Id,
                    sel("setStringValue:").unwrap(),
                    ns_string(value),
                );
            }
        }
    }

    unsafe fn show_alert(message: &str) {
        let alert = send_id(class("NSAlert").unwrap(), sel("new").unwrap());
        send_void_id(alert, sel("setMessageText:").unwrap(), ns_string("Span"));
        send_void_id(
            alert,
            sel("setInformativeText:").unwrap(),
            ns_string(message),
        );
        send_id_id(alert, sel("addButtonWithTitle:").unwrap(), ns_string("知道了"));
        send_integer(alert, sel("runModal").unwrap());
    }

    fn ns_string_to_string(value: Id) -> Option<String> {
        unsafe {
            let pointer = send_id(value, sel("UTF8String").ok()?) as *const c_char;
            if pointer.is_null() { return None; }
            Some(std::ffi::CStr::from_ptr(pointer).to_string_lossy().into_owned())
        }
    }

    fn ns_string(value: &str) -> Id {
        let value = CString::new(value.replace('\0', " ")).unwrap();
        unsafe {
            send_id_cstr(
                class("NSString").unwrap(),
                sel("stringWithUTF8String:").unwrap(),
                value.as_ptr(),
            )
        }
    }
    fn class(name: &str) -> io::Result<Id> {
        let name = CString::new(name).unwrap();
        let value = unsafe { objc_getClass(name.as_ptr()) };
        if value.is_null() {
            Err(io::Error::other(format!(
                "Objective-C class unavailable: {name:?}"
            )))
        } else {
            Ok(value)
        }
    }
    fn class_id(value: usize) -> Id {
        value as Id
    }
    fn sel(name: &str) -> io::Result<Sel> {
        let name = CString::new(name).unwrap();
        let value = unsafe { sel_registerName(name.as_ptr()) };
        if value.is_null() {
            Err(io::Error::other("Objective-C selector unavailable"))
        } else {
            Ok(value)
        }
    }

    unsafe fn send_id(receiver: Id, selector: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector)
    }
    unsafe fn send_id_id(receiver: Id, selector: Sel, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_id_id_id_id(receiver: Id, selector: Sel, a: Id, b: Id, c: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id, Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, a, b, c)
    }

    unsafe fn send_id_rect_integer_integer_bool(
        receiver: Id,
        selector: Sel,
        rect: Rect,
        a: NSUInteger,
        b: NSUInteger,
        c: BOOL,
    ) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Rect, NSUInteger, NSUInteger, BOOL) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, rect, a, b, c)
    }
    unsafe fn send_id_rect_bool(receiver: Id, selector: Sel, rect: Rect, flag: BOOL) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Rect, BOOL) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, rect, flag)
    }
    unsafe fn send_id_cstr(receiver: Id, selector: Sel, arg: *const c_char) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_id_double(receiver: Id, selector: Sel, arg: f64) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, f64) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_void(receiver: Id, selector: Sel) {
        let f: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector)
    }
    unsafe fn send_void_id(receiver: Id, selector: Sel, arg: Id) {
        let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_void_id_bool(receiver: Id, selector: Sel, arg1: Id, arg2: Id, arg3: BOOL) {
        let f: unsafe extern "C" fn(Id, Sel, Id, Id, BOOL) =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg1, arg2, arg3)
    }
    unsafe fn send_u64(receiver: Id, selector: Sel) -> u64 {
        let f: unsafe extern "C" fn(Id, Sel) -> u64 = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector)
    }
    unsafe fn send_void_rect(receiver: Id, selector: Sel, arg: Rect) {
        let f: unsafe extern "C" fn(Id, Sel, Rect) = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_void_integer(receiver: Id, selector: Sel, arg: NSInteger) {
        let f: unsafe extern "C" fn(Id, Sel, NSInteger) =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_void_bool(receiver: Id, selector: Sel, arg: BOOL) {
        let f: unsafe extern "C" fn(Id, Sel, BOOL) = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector, arg)
    }
    unsafe fn send_integer(receiver: Id, selector: Sel) -> NSInteger {
        let f: unsafe extern "C" fn(Id, Sel) -> NSInteger =
            std::mem::transmute(objc_msgSend as *const ());
        f(receiver, selector)
    }
}

#[cfg(target_os = "windows")]
mod windows {
    #![allow(unsafe_op_in_unsafe_fn)]
    use super::io;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use span_core::TrustState;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BN_CLICKED, BS_PUSHBUTTON, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
        DispatchMessageW, GetMessageW, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LBS_NOTIFY,
        MB_ICONERROR, MB_ICONQUESTION, MB_OK, MB_YESNO, MSG, MessageBoxW, PostMessageW,
        PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW, SetWindowTextW, ShowWindow,
        TranslateMessage, WM_APP, WM_COMMAND, WM_CREATE, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CHILD,
        WS_OVERLAPPEDWINDOW,
        WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    };

    const ID_DISCOVER: u16 = 1001;
    const ID_REMOVE: u16 = 1002;
    const ID_TRUSTED_LIST: u16 = 2001;
    const WM_SPAN_ACTION_DONE: u32 = WM_APP + 1;

    static STATUS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    static TRUSTED_LIST: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    static ACTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

    pub fn prompt_pairing(device_id: &str, name: &str, platform: &str) -> io::Result<()> {
        let prompt = format!(
            "{}（{}）请求连接 Span。\r\n\r\n是否信任此设备并开启剪贴板同步？",
            name, platform
        );
        if unsafe { ask_yes_no(&prompt) } {
            let id = span_core::DeviceId::new(device_id.to_string())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid device id"))?;
            let mut store =
                crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
            store.trust_existing(&id)?;
            unsafe {
                show_info(
                    ptr::null_mut(),
                    &format!("已信任 {name}，剪贴板同步已开启。"),
                );
            }
        }
        Ok(())
    }

    pub fn open() -> io::Result<()> {
        // The daemon is the real background process. The GUI only provides
        // discovery, pairing and trusted-device management.
        let _ = crate::autostart::install();
        let _ = crate::daemon_control::start_daemon();
        let class_name = wide("SpanGuiWindow");
        let title = wide("Span · 跨设备剪贴板");

        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            if instance.is_null() {
                return Err(io::Error::last_os_error());
            }

            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                hbrBackground: ptr::null_mut(),
                ..std::mem::zeroed()
            };
            let _ = RegisterClassW(&class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                560,
                420,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            );
            if hwnd.is_null() {
                return Err(io::Error::last_os_error());
            }
            ShowWindow(hwnd, SW_SHOW);

            let mut message: MSG = std::mem::zeroed();
            loop {
                let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                if result == -1 {
                    return Err(io::Error::last_os_error());
                }
                if result == 0 {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => {
                if let Err(error) = create_controls(hwnd) {
                    show_error(hwnd, &error.to_string());
                }
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u16;
                let code = ((wparam >> 16) & 0xffff) as u16;
                if u32::from(code) == BN_CLICKED {
                    start_action(hwnd, id);
                }
                0
            }
            WM_SPAN_ACTION_DONE => {
                finish_action(hwnd, wparam as u16, _lparam as *mut String);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, _lparam),
        }
    }

    unsafe fn create_controls(hwnd: HWND) -> io::Result<()> {
        let local = crate::config::load_or_create_local_device()?;
        add_control(hwnd, "STATIC", "SPAN", 22, 18, 500, 34, 0, 0)?;
        add_control(
            hwnd,
            "STATIC",
            "在可信设备之间自动同步剪贴板",
            24,
            54,
            500,
            24,
            0,
            0,
        )?;
        add_control(
            hwnd,
            "STATIC",
            &format!("本机：{}", local.name),
            24,
            84,
            500,
            24,
            0,
            0,
        )?;
        add_control(
            hwnd,
            "STATIC",
            "仅向可信设备同步剪贴板。",
            24,
            108,
            500,
            24,
            0,
            0,
        )?;
        add_control(hwnd, "STATIC", "可信设备", 24, 142, 500, 22, 0, 0)?;

        let list = add_control(
            hwnd,
            "LISTBOX",
            "",
            24,
            166,
            500,
            105,
            ID_TRUSTED_LIST,
            WS_BORDER | WS_VSCROLL | LBS_NOTIFY as u32,
        )?;
        let _ = TRUSTED_LIST.set(list as usize);
        refresh_trusted_list();

        let status = add_control(hwnd, "STATIC", "运行中 · 仅向可信设备同步剪贴板", 24, 282, 500, 32, 0, 0)?;
        let _ = STATUS.set(status as usize);
        button(hwnd, "发现设备", ID_DISCOVER, 24, 326, 140, 34)?;
        button(hwnd, "移除选中设备", ID_REMOVE, 180, 326, 140, 34)?;
        Ok(())
    }

    unsafe fn button(
        hwnd: HWND,
        title: &str,
        id: u16,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> io::Result<HWND> {
        add_control(
            hwnd,
            "BUTTON",
            title,
            x,
            y,
            width,
            height,
            id,
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        )
    }

    unsafe fn add_control(
        parent: HWND,
        class: &str,
        title: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        id: u16,
        extra_style: u32,
    ) -> io::Result<HWND> {
        let class = wide(class);
        let title = wide(title);
        let control = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_CHILD | WS_VISIBLE | extra_style,
            x,
            y,
            width,
            height,
            parent,
            id as usize as *mut _,
            ptr::null_mut(),
            ptr::null(),
        );
        if control.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(control)
        }
    }

    unsafe fn start_action(hwnd: HWND, id: u16) {
        if ACTION_IN_PROGRESS.swap(true, Ordering::AcqRel) {
            return;
        }
        set_status("处理中，请稍候…");
        let hwnd_value = hwnd as usize;
        std::thread::spawn(move || {
            let result = match id {
                ID_DISCOVER => discover_and_pair(),
                ID_REMOVE => remove_selected(),
                _ => Ok(String::new()),
            };
            let message = match result {
                Ok(message) => message,
                Err(error) => format!("操作失败：{error}"),
            };
            let boxed = Box::new(message);
            unsafe {
                PostMessageW(
                    hwnd_value as HWND,
                    WM_SPAN_ACTION_DONE,
                    id as usize,
                    Box::into_raw(boxed) as LPARAM,
                );
            }
        });
    }

    unsafe fn finish_action(hwnd: HWND, _id: u16, raw: *mut String) {
        if raw.is_null() {
            ACTION_IN_PROGRESS.store(false, Ordering::Release);
            return;
        }
        let message = *Box::from_raw(raw);
        refresh_trusted_list();
        set_status(&message);
        if message.starts_with("操作失败：") {
            show_error(hwnd, &message);
        }
        ACTION_IN_PROGRESS.store(false, Ordering::Release);
    }

    fn discover_and_pair() -> io::Result<String> {
        let local = crate::config::load_or_create_local_device()?;
        let devices = crate::scan_devices(&local, Duration::from_millis(700))?;
        let available: Vec<_> = devices
            .iter()
            .filter(|device| device.trust_state != TrustState::Trusted)
            .collect();
        if available.is_empty() {
            return Ok("没有发现新设备，可信设备会自动同步。".into());
        }

        let names = available
            .iter()
            .map(|device| {
                format!(
                    "{} ({})",
                    device.name,
                    crate::config::platform_name(device.platform)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "发现新设备：\r\n\r\n{names}\r\n\r\n是否信任并开启剪贴板同步？"
        );
        if unsafe { ask_yes_no(&prompt) } {
            let mut store =
                crate::trust_store::TrustStore::load(crate::config::trust_store_path()?)?;
            for device in available {
                store.trust_existing(&device.id)?;
            }
            Ok(format!(
                "已信任 {} 台设备，剪贴板同步已开启。",
                names.lines().count()
            ))
        } else {
            Ok("已取消配对，未共享剪贴板内容。".into())
        }
    }

    fn remove_selected() -> io::Result<String> {
        let Some(list) = TRUSTED_LIST.get().copied() else {
            return Ok("暂无可移除的可信设备。".into());
        };
        let selected = unsafe { SendMessageW(list as HWND, LB_GETCURSEL, 0, 0) };
        if selected < 0 {
            return Ok("请先选择一个可信设备。".into());
        }
        let path = crate::config::trust_store_path()?;
        let store = crate::trust_store::TrustStore::load(&path)?;
        let trusted = store.trusted_devices();
        let Some(device) = trusted.get(selected as usize) else {
            return Ok("设备列表已变化，请重试。".into());
        };
        let id = device.id.clone();
        let name = device.name.clone();
        if unsafe { ask_yes_no(&format!("是否移除可信设备“{name}”？")) } {
            let mut store = crate::trust_store::TrustStore::load(path)?;
            store.revoke(&id)?;
            Ok(format!("已移除 {name}。"))
        } else {
            Ok("已取消移除。".into())
        }
    }

    unsafe fn refresh_trusted_list() {
        let Some(list) = TRUSTED_LIST.get().copied() else {
            return;
        };
        SendMessageW(list as HWND, LB_RESETCONTENT, 0, 0);
        let Ok(path) = crate::config::trust_store_path() else {
            return;
        };
        let Ok(store) = crate::trust_store::TrustStore::load(path) else {
            return;
        };
        let trusted = store.trusted_devices();
        if trusted.is_empty() {
            let text = wide("暂无可信设备，请点击“发现设备”。");
            SendMessageW(list as HWND, LB_ADDSTRING, 0, text.as_ptr() as LPARAM);
            return;
        }
        for device in trusted {
            let text = wide(&format!(
                "{} · {}",
                device.name,
                crate::config::platform_name(device.platform)
            ));
            SendMessageW(list as HWND, LB_ADDSTRING, 0, text.as_ptr() as LPARAM);
        }
    }

    unsafe fn set_status(value: &str) {
        if let Some(status) = STATUS.get() {
            let value = wide(value);
            SetWindowTextW(*status as HWND, value.as_ptr());
        }
    }

    unsafe fn ask_yes_no(message: &str) -> bool {
        let title = wide("Span");
        let message = wide(message);
        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONQUESTION,
        ) == 6 // IDYES
    }

    unsafe fn show_info(hwnd: HWND, message: &str) {
        let title = wide("Span");
        let message = wide(message);
        MessageBoxW(hwnd, message.as_ptr(), title.as_ptr(), MB_OK);
    }

    unsafe fn show_error(hwnd: HWND, message: &str) {
        let title = wide("Span");
        let message = wide(message);
        MessageBoxW(hwnd, message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod unix_other {
    use super::io;

    pub fn prompt_pairing(_device_id: &str, _name: &str, _platform: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native pairing prompt is not available on this build",
        ))
    }

    pub fn open() -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native GUI is not available on this build; use the CLI",
        ))
    }
}

pub fn prompt_pairing(device_id: &str, name: &str, platform: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::prompt_pairing(device_id, name, platform);
    #[cfg(target_os = "windows")]
    return windows::prompt_pairing(device_id, name, platform);
    #[cfg(all(unix, not(target_os = "macos")))]
    return unix_other::prompt_pairing(device_id, name, platform);
    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "pairing prompt is not available on this platform",
    ))
}

pub fn open() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open();
    #[cfg(target_os = "windows")]
    return windows::open();
    #[cfg(all(unix, not(target_os = "macos")))]
    return unix_other::open();
    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "GUI is not available on this platform",
    ))
}
