#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    static SETTINGS_REQUESTED: AtomicBool = AtomicBool::new(false);
    static OPEN_LIBRARY_REQUESTED: AtomicBool = AtomicBool::new(false);
    static UNDO_REQUESTED: AtomicBool = AtomicBool::new(false);
    static ICON_INSTALLED: AtomicBool = AtomicBool::new(false);
    static MENU_INSTALLED: AtomicBool = AtomicBool::new(false);
    static MENU_TARGET: OnceLock<usize> = OnceLock::new();

    pub fn install_app_icon() {
        if ICON_INSTALLED.swap(true, Ordering::AcqRel) {
            return;
        }

        let Some(path) = app_icon_path() else {
            return;
        };
        let path = path.to_string_lossy();
        unsafe {
            let image: *mut Object = msg_send![class!(NSImage), alloc];
            let image: *mut Object = msg_send![image, initWithContentsOfFile: ns_string(&path)];
            if image.is_null() {
                return;
            }

            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setApplicationIconImage: image];
        }
    }

    pub fn install_app_menu() {
        unsafe {
            install_app_icon();
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setActivationPolicy: 0isize];
            let target = menu_target();
            let first_install = !MENU_INSTALLED.swap(true, Ordering::AcqRel);
            if first_install {
                install_activation_observer(target);
            }

            let main_menu: *mut Object = msg_send![class!(NSMenu), new];
            let app_menu_item: *mut Object = msg_send![class!(NSMenuItem), new];
            let _: () = msg_send![app_menu_item, setTitle: ns_string("ModelRack")];
            let _: () = msg_send![main_menu, addItem: app_menu_item];

            let app_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![app_menu, setTitle: ns_string("ModelRack")];
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
                "Settings…",
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
                "Open Library…",
                "o",
                sel!(openModelRackLibrary:),
                target,
            );
            add_item(
                file_menu,
                "Close Window",
                "w",
                sel!(hideModelRackWindow:),
                target,
            );

            let edit_menu_item = add_plain_item(main_menu, "Edit");
            let edit_menu: *mut Object = msg_send![class!(NSMenu), new];
            let _: () = msg_send![edit_menu, setTitle: ns_string("Edit")];
            let _: () = msg_send![edit_menu_item, setSubmenu: edit_menu];
            add_item(
                edit_menu,
                "Undo Remove from Library",
                "z",
                sel!(undoModelRackLibraryAction:),
                target,
            );
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
                sel!(toggleModelRackFullScreen:),
                target,
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

    fn app_icon_path() -> Option<PathBuf> {
        let bundled = std::env::current_exe()
            .ok()
            .and_then(|exe| {
                let contents = exe.parent()?.parent()?;
                Some(contents.join("Resources").join("AppIcon.icns"))
            })
            .filter(|path| path.exists());
        if bundled.is_some() {
            return bundled;
        }

        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("AppIcon.icns");
        source.exists().then_some(source)
    }

    pub fn take_settings_request() -> bool {
        SETTINGS_REQUESTED.swap(false, Ordering::AcqRel)
    }

    pub fn take_open_library_request() -> bool {
        OPEN_LIBRARY_REQUESTED.swap(false, Ordering::AcqRel)
    }

    pub fn take_undo_request() -> bool {
        UNDO_REQUESTED.swap(false, Ordering::AcqRel)
    }

    pub fn configure_native_window_chrome() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];

            // Dark appearance for the whole app.
            let name = ns_string("NSAppearanceNameDarkAqua");
            let appearance: *mut Object = msg_send![class!(NSAppearance), appearanceNamed: name];
            let _: () = msg_send![app, setAppearance: appearance];

            let windows: *mut Object = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            for idx in 0..count {
                let window: *mut Object = msg_send![windows, objectAtIndex: idx];

                // Keep the NSWindow wrapper/decorations alive for native
                // rounded corners, shadow, and full-screen transitions, but
                // let Slint draw the visible titlebar/traffic lights.
                let _: () = msg_send![window, setTitle: ns_string("")];
                let _: () = msg_send![window, setTitleVisibility: 1isize];
                let _: () = msg_send![window, setTitlebarAppearsTransparent: true];

                let style_mask: usize = msg_send![window, styleMask];
                let titled = 1usize << 0;
                let closable = 1usize << 1;
                let miniaturizable = 1usize << 2;
                let resizable = 1usize << 3;
                let full_size_content_view = 1usize << 15;
                let _: () = msg_send![
                    window,
                    setStyleMask: style_mask
                        | titled
                        | closable
                        | miniaturizable
                        | resizable
                        | full_size_content_view
                ];

                let collection_behavior: usize = msg_send![window, collectionBehavior];
                let full_screen_primary = 1usize << 7;
                let _: () = msg_send![
                    window,
                    setCollectionBehavior: collection_behavior | full_screen_primary
                ];

                let _: () =
                    msg_send![window, setBackgroundColor: ns_color(0.149, 0.157, 0.180, 1.0)];
                let _: () = msg_send![window, setHasShadow: true];

                for button in 0usize..=2 {
                    let standard_button: *mut Object =
                        msg_send![window, standardWindowButton: button];
                    if !standard_button.is_null() {
                        let _: () = msg_send![standard_button, setHidden: true];
                        let _: () = msg_send![standard_button, setEnabled: false];
                        let _: () = msg_send![standard_button, setAlphaValue: 0.0f64];
                    }
                }
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

    pub fn fullscreen_window() {
        unsafe {
            if let Some(window) = front_window() {
                let full_screen_primary = 1usize << 7;
                let collection_behavior: usize = msg_send![window, collectionBehavior];
                let _: () = msg_send![
                    window,
                    setCollectionBehavior: collection_behavior | full_screen_primary
                ];
                let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                let _: () = msg_send![window, toggleFullScreen: std::ptr::null_mut::<Object>()];
            }
        }
    }

    pub fn show_windows() {
        unsafe {
            show_all_windows();
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
                sel!(undoModelRackLibraryAction:),
                undo_modelrack_library_action as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(hideModelRackWindow:),
                hide_modelrack_window as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                sel!(toggleModelRackFullScreen:),
                toggle_modelrack_fullscreen as extern "C" fn(&Object, Sel, *mut Object),
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

    extern "C" fn undo_modelrack_library_action(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        UNDO_REQUESTED.store(true, Ordering::Release);
    }

    extern "C" fn hide_modelrack_window(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        hide_window();
    }

    extern "C" fn toggle_modelrack_fullscreen(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        fullscreen_window();
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
    configure_native_window_chrome, fullscreen_window, hide_window, install_app_icon,
    install_app_menu, minimize_window, show_windows, take_open_library_request,
    take_settings_request, take_undo_request,
};

#[cfg(not(target_os = "macos"))]
pub fn install_app_menu() {}

#[cfg(not(target_os = "macos"))]
pub fn install_app_icon() {}

#[cfg(not(target_os = "macos"))]
pub fn take_settings_request() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn take_open_library_request() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn take_undo_request() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn configure_native_window_chrome() {}

#[cfg(not(target_os = "macos"))]
pub fn hide_window() {}

#[cfg(not(target_os = "macos"))]
pub fn minimize_window() {}

#[cfg(not(target_os = "macos"))]
pub fn fullscreen_window() {}

#[cfg(not(target_os = "macos"))]
pub fn show_windows() {}
