#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    static SETTINGS_REQUESTED: AtomicBool = AtomicBool::new(false);
    static OPEN_LIBRARY_REQUESTED: AtomicBool = AtomicBool::new(false);
    static MENU_INSTALLED: AtomicBool = AtomicBool::new(false);
    static MENU_TARGET: OnceLock<usize> = OnceLock::new();
    static RESTORE_FRAME: Mutex<Option<NSRect>> = Mutex::new(None);

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }

    pub fn install_app_menu() {
        if MENU_INSTALLED.swap(true, Ordering::AcqRel) {
            return;
        }

        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setActivationPolicy: 0isize];
            let target = menu_target();
            let _: () = msg_send![app, setDelegate: target];
            install_activation_observer(target);

            let main_menu: *mut Object = msg_send![class!(NSMenu), new];
            let app_menu_item: *mut Object = msg_send![class!(NSMenuItem), new];
            let _: () = msg_send![main_menu, addItem: app_menu_item];

            let app_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![app_menu_item, setSubmenu: app_menu];

            add_item(
                app_menu,
                "About ModelRack",
                "",
                sel!(orderFrontStandardAboutPanel:),
                std::ptr::null_mut(),
            );
            add_separator(app_menu);

            add_item(
                app_menu,
                "Settings...",
                ",",
                sel!(openModelRackSettings:),
                target,
            );
            add_separator(app_menu);

            let services_item = add_plain_item(app_menu, "Services");
            let services_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![services_item, setSubmenu: services_menu];
            let _: () = msg_send![app, setServicesMenu: services_menu];
            add_separator(app_menu);

            add_item(
                app_menu,
                "Hide ModelRack",
                "h",
                sel!(hide:),
                std::ptr::null_mut(),
            );
            add_item(
                app_menu,
                "Hide Others",
                "h",
                sel!(hideOtherApplications:),
                std::ptr::null_mut(),
            );
            add_item(
                app_menu,
                "Show All",
                "",
                sel!(unhideAllApplications:),
                std::ptr::null_mut(),
            );
            add_separator(app_menu);
            add_item(
                app_menu,
                "Quit ModelRack",
                "q",
                sel!(terminate:),
                std::ptr::null_mut(),
            );

            let file_menu_item = add_plain_item(main_menu, "File");
            let file_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![file_menu, setTitle: ns_string("File")];
            let _: () = msg_send![file_menu_item, setSubmenu: file_menu];
            add_item(
                file_menu,
                "Open Library...",
                "o",
                sel!(openModelRackLibrary:),
                target,
            );
            add_item(
                file_menu,
                "Close Window",
                "w",
                sel!(performClose:),
                std::ptr::null_mut(),
            );

            let edit_menu_item = add_plain_item(main_menu, "Edit");
            let edit_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![edit_menu, setTitle: ns_string("Edit")];
            let _: () = msg_send![edit_menu_item, setSubmenu: edit_menu];
            add_item(edit_menu, "Undo", "z", sel!(undo:), std::ptr::null_mut());
            add_item(edit_menu, "Redo", "Z", sel!(redo:), std::ptr::null_mut());
            add_separator(edit_menu);
            add_item(edit_menu, "Cut", "x", sel!(cut:), std::ptr::null_mut());
            add_item(edit_menu, "Copy", "c", sel!(copy:), std::ptr::null_mut());
            add_item(edit_menu, "Paste", "v", sel!(paste:), std::ptr::null_mut());
            add_item(
                edit_menu,
                "Select All",
                "a",
                sel!(selectAll:),
                std::ptr::null_mut(),
            );

            let view_menu_item = add_plain_item(main_menu, "View");
            let view_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![view_menu, setTitle: ns_string("View")];
            let _: () = msg_send![view_menu_item, setSubmenu: view_menu];
            add_item(
                view_menu,
                "Enter Full Screen",
                "f",
                sel!(toggleFullScreen:),
                std::ptr::null_mut(),
            );

            let window_menu_item = add_plain_item(main_menu, "Window");
            let window_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![window_menu, setTitle: ns_string("Window")];
            let _: () = msg_send![window_menu_item, setSubmenu: window_menu];
            add_item(
                window_menu,
                "Minimize",
                "m",
                sel!(performMiniaturize:),
                std::ptr::null_mut(),
            );
            add_item(
                window_menu,
                "Zoom",
                "",
                sel!(performZoom:),
                std::ptr::null_mut(),
            );
            add_separator(window_menu);
            add_item(
                window_menu,
                "Bring All to Front",
                "",
                sel!(arrangeInFront:),
                std::ptr::null_mut(),
            );
            let _: () = msg_send![app, setWindowsMenu: window_menu];

            let help_menu_item = add_plain_item(main_menu, "Help");
            let help_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![help_menu, setTitle: ns_string("Help")];
            let _: () = msg_send![help_menu_item, setSubmenu: help_menu];
            add_item(
                help_menu,
                "ModelRack Help",
                "?",
                sel!(showHelp:),
                std::ptr::null_mut(),
            );
            let _: () = msg_send![app, setHelpMenu: help_menu];

            let _: () = msg_send![app, setMainMenu: main_menu];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }

    pub fn take_settings_request() -> bool {
        SETTINGS_REQUESTED.swap(false, Ordering::AcqRel)
    }

    pub fn take_open_library_request() -> bool {
        OPEN_LIBRARY_REQUESTED.swap(false, Ordering::AcqRel)
    }

    pub fn hide_application() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, hide: std::ptr::null_mut::<Object>()];
        }
    }

    pub fn configure_window_appearance() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let windows: *mut Object = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            for idx in 0..count {
                let window: *mut Object = msg_send![windows, objectAtIndex: idx];
                let content_view: *mut Object = msg_send![window, contentView];
                let _: () = msg_send![content_view, setWantsLayer: true];
                let layer: *mut Object = msg_send![content_view, layer];
                let _: () = msg_send![layer, setCornerRadius: 12.0f64];
                let _: () = msg_send![layer, setMasksToBounds: true];
                let _: () = msg_send![window, setBackgroundColor: ns_color(0.0, 0.0, 0.0, 0.0)];
                let _: () = msg_send![window, setOpaque: false];
                let _: () = msg_send![window, setHasShadow: true];
            }
        }
    }

    pub fn hide_window() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, hide: std::ptr::null_mut::<Object>()];
        }
    }

    pub fn minimize_window() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let window: *mut Object = msg_send![app, keyWindow];
            if !window.is_null() {
                let _: () = msg_send![window, miniaturize: std::ptr::null_mut::<Object>()];
            }
        }
    }

    pub fn zoom_window() {
        unsafe {
            if let Some(window) = front_window() {
                toggle_window_zoom(window);
            }
        }
    }

    pub fn show_windows() {
        unsafe {
            show_all_windows();
        }
    }

    pub fn configure_transparent_titlebar() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];

            // Dark appearance for the whole app
            let name = ns_string("NSAppearanceNameDarkAqua");
            let appearance: *mut Object = msg_send![class!(NSAppearance), appearanceNamed: name];
            let _: () = msg_send![app, setAppearance: appearance];

            let windows: *mut Object = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            for idx in 0..count {
                let window: *mut Object = msg_send![windows, objectAtIndex: idx];

                // Hide native title text
                let _: () = msg_send![window, setTitle: ns_string("")];
                let _: () = msg_send![window, setTitleVisibility: 1isize];

                // Transparent titlebar — window bg shows through
                let _: () = msg_send![window, setTitlebarAppearsTransparent: true];

                // Let Slint draw into the native titlebar area so we don't get
                // a separate macOS titlebar stacked above the custom one.
                let style_mask: usize = msg_send![window, styleMask];
                let full_size_content_view = 1usize << 15;
                let resizable = 1usize << 3;
                let _: () = msg_send![window, setStyleMask: style_mask | full_size_content_view | resizable];

                // bg-1 = #26282e exactly
                let _: () = msg_send![window, setBackgroundColor: ns_color(0.149, 0.157, 0.180, 1.0)];
            }
        }
    }

    unsafe fn ns_color(r: f64, g: f64, b: f64, a: f64) -> *mut Object {
        msg_send![class!(NSColor), colorWithRed:r green:g blue:b alpha:a]
    }

    unsafe fn front_window() -> Option<*mut Object> {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let key_window: *mut Object = msg_send![app, keyWindow];
        if !key_window.is_null() {
            return Some(key_window);
        }

        let main_window: *mut Object = msg_send![app, mainWindow];
        if !main_window.is_null() {
            return Some(main_window);
        }

        let windows: *mut Object = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        if count == 0 {
            None
        } else {
            let window: *mut Object = msg_send![windows, objectAtIndex: 0usize];
            (!window.is_null()).then_some(window)
        }
    }

    unsafe fn show_all_windows() {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, unhide: std::ptr::null_mut::<Object>()];
        let windows: *mut Object = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        for idx in 0..count {
            let window: *mut Object = msg_send![windows, objectAtIndex: idx];
            let _: () = msg_send![window, deminiaturize: std::ptr::null_mut::<Object>()];
            let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
            let _: () = msg_send![window, orderFrontRegardless];
        }
        let _: () = msg_send![app, activateIgnoringOtherApps: true];
    }

    unsafe fn toggle_window_zoom(window: *mut Object) {
        let frame: NSRect = msg_send![window, frame];
        let screen: *mut Object = msg_send![window, screen];
        let screen = if screen.is_null() {
            msg_send![class!(NSScreen), mainScreen]
        } else {
            screen
        };
        if screen.is_null() {
            let _: () = msg_send![window, performZoom: std::ptr::null_mut::<Object>()];
            return;
        }

        let visible_frame: NSRect = msg_send![screen, visibleFrame];
        if frame_matches(frame, visible_frame) {
            if let Some(restore_frame) = RESTORE_FRAME.lock().ok().and_then(|mut frame| frame.take()) {
                let _: () = msg_send![window, setFrame: restore_frame display: true animate: false];
                let _: () = msg_send![window, displayIfNeeded];
            }
            return;
        }

        if let Ok(mut restore_frame) = RESTORE_FRAME.lock() {
            *restore_frame = Some(frame);
        }
        let _: () = msg_send![window, setFrame: visible_frame display: true animate: false];
        let _: () = msg_send![window, displayIfNeeded];
    }

    fn frame_matches(a: NSRect, b: NSRect) -> bool {
        const TOLERANCE: f64 = 2.0;
        (a.origin.x - b.origin.x).abs() < TOLERANCE
            && (a.origin.y - b.origin.y).abs() < TOLERANCE
            && (a.size.width - b.size.width).abs() < TOLERANCE
            && (a.size.height - b.size.height).abs() < TOLERANCE
    }

    unsafe fn add_item(
        menu: *mut Object,
        title: &str,
        key: &str,
        action: Sel,
        target: *mut Object,
    ) -> *mut Object {
        let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let item: *mut Object = msg_send![item, initWithTitle: ns_string(title) action: action keyEquivalent: ns_string(key)];
        if !target.is_null() {
            let _: () = msg_send![item, setTarget: target];
        }
        let _: () = msg_send![menu, addItem: item];
        item
    }

    unsafe fn add_plain_item(menu: *mut Object, title: &str) -> *mut Object {
        let item: *mut Object = msg_send![class!(NSMenuItem), new];
        let _: () = msg_send![item, setTitle: ns_string(title)];
        let _: () = msg_send![menu, addItem: item];
        item
    }

    unsafe fn add_separator(menu: *mut Object) {
        let item: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: item];
    }

    unsafe fn ns_string(value: &str) -> *mut Object {
        let ns_string: *mut Object = msg_send![class!(NSString), alloc];
        msg_send![
            ns_string,
            initWithBytes: value.as_ptr()
            length: value.len()
            encoding: 4usize
        ]
    }

    unsafe fn menu_target() -> *mut Object {
        *MENU_TARGET.get_or_init(|| {
            let class = menu_target_class();
            let target: *mut Object = msg_send![class, new];
            target as usize
        }) as *mut Object
    }

    fn menu_target_class() -> &'static Class {
        if let Some(class) = Class::get("ModelRackMenuTarget") {
            return class;
        }

        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("ModelRackMenuTarget", superclass).unwrap();
        unsafe {
            decl.add_method(
                sel!(openModelRackSettings:),
                open_modelrack_settings as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(openModelRackLibrary:),
                open_modelrack_library as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(modelRackApplicationDidBecomeActive:),
                application_did_become_active as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                application_should_handle_reopen
                    as extern "C" fn(&Object, Sel, *mut Object, bool) -> bool,
            );
        }
        decl.register()
    }

    extern "C" fn open_modelrack_settings(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        SETTINGS_REQUESTED.store(true, Ordering::Release);
    }

    extern "C" fn open_modelrack_library(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        OPEN_LIBRARY_REQUESTED.store(true, Ordering::Release);
    }

    unsafe fn install_activation_observer(target: *mut Object) {
        let center: *mut Object = msg_send![class!(NSNotificationCenter), defaultCenter];
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![
            center,
            addObserver: target
            selector: sel!(modelRackApplicationDidBecomeActive:)
            name: ns_string("NSApplicationDidBecomeActiveNotification")
            object: app
        ];
    }

    extern "C" fn application_did_become_active(
        _this: &Object,
        _cmd: Sel,
        _notification: *mut Object,
    ) {
        unsafe {
            show_all_windows();
        }
    }

    extern "C" fn application_should_handle_reopen(
        _this: &Object,
        _cmd: Sel,
        _app: *mut Object,
        _has_visible_windows: bool,
    ) -> bool {
        unsafe {
            show_all_windows();
        }
        true
    }
}

#[cfg(target_os = "macos")]
pub use imp::{
    configure_transparent_titlebar, configure_window_appearance, hide_application, hide_window,
    install_app_menu, minimize_window, take_open_library_request, take_settings_request,
    show_windows, zoom_window,
};

#[cfg(not(target_os = "macos"))]
pub fn install_app_menu() {}

#[cfg(not(target_os = "macos"))]
pub fn take_settings_request() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn take_open_library_request() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn hide_application() {}

#[cfg(not(target_os = "macos"))]
pub fn configure_transparent_titlebar() {}

#[cfg(not(target_os = "macos"))]
pub fn configure_window_appearance() {}

#[cfg(not(target_os = "macos"))]
pub fn hide_window() {}

#[cfg(not(target_os = "macos"))]
pub fn minimize_window() {}

#[cfg(not(target_os = "macos"))]
pub fn show_windows() {}

#[cfg(not(target_os = "macos"))]
pub fn zoom_window() {}
