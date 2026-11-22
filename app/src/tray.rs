//! Menu-bar icon (macOS: top right, Windows: notification area) to call up the program at any time.

#[allow(unused_imports)]
use eframe::egui;
#[allow(unused_imports)]
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayCmd {
    Show,
    New,
    Snippets,
    Sync,
    Quit,
}

pub struct Tray {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _icon: tray_icon::TrayIcon,
    pub rx: Receiver<TrayCmd>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn create(ctx: egui::Context, ns_view: usize) -> Option<Tray> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let menu = Menu::new();
    let show = MenuItem::new("hmb öffnen", true, None);
    let new = MenuItem::new("Neuer Eintrag", true, None);
    let snippets = MenuItem::new("Snippets", true, None);
    let sync = MenuItem::new("Synchronisieren", true, None);
    let quit = MenuItem::new("Beenden", true, None);
    menu.append_items(&[
        &show,
        &new,
        &snippets,
        &PredefinedMenuItem::separator(),
        &sync,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .ok()?;
    let ids = [
        (show.id().clone(), TrayCmd::Show),
        (new.id().clone(), TrayCmd::New),
        (snippets.id().clone(), TrayCmd::Snippets),
        (sync.id().clone(), TrayCmd::Sync),
        (quit.id().clone(), TrayCmd::Quit),
    ];
    let size = 44;
    let icon = Icon::from_rgba(crate::icon::tray_icon_rgba(size), size, size).ok()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("hmb – Hugo Microblogger")
        .with_icon(icon)
        .with_icon_as_template(true)
        .build()
        .ok()?;

    let (tx, rx) = channel();
    MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
        if let Some((_, cmd)) = ids.iter().find(|(id, _)| *id == ev.id) {
            // bring the window back first so it gets drawn and can react
            native_show(ns_view);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            let _ = tx.send(*cmd);
            ctx.request_repaint();
        }
    }));
    Some(Tray { _icon: tray, rx })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn create(_ctx: egui::Context, _ns_view: usize) -> Option<Tray> {
    None
}

/// Un-minimises the window and brings the app to the front (macOS).
#[cfg(target_os = "macos")]
pub fn native_show(ns_view: usize) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
        if ns_view != 0 {
            let view = ns_view as *mut AnyObject;
            let win: *mut AnyObject = msg_send![view, window];
            if !win.is_null() {
                let none: *mut AnyObject = std::ptr::null_mut();
                let _: () = msg_send![win, deminiaturize: none];
                let _: () = msg_send![win, makeKeyAndOrderFront: none];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn native_show(_ns_view: usize) {}
