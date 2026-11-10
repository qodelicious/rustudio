//! macOS: the app menu in the menu bar (rustudio's own menus stay in the
//! window, like Studio's ribbon).
//!
//! winit's default app menu quits with `terminate:`, which ends the process
//! at once: no "Discard unsaved changes?" and no saved layout. Here Quit (⌘Q)
//! closes the window instead (`performClose:`), the same path as its close
//! button, so the editor asks first. The menu has only the standard
//! Hide / Hide Others / Show All / Quit items: any other key equivalent here
//! would take that key from the editor (⌘M is Studio's Import, ⌘W unused).

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

/// Replaces the menu bar's app menu. Call on the main thread once the app
/// has finished launching (from the eframe app's constructor).
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let item = |title: &str, action: Option<Sel>, key: &str| -> Retained<NSMenuItem> {
        // SAFETY: standard NSResponder / NSApplication action selectors.
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        }
    };
    let menu = NSMenu::new(mtm);
    menu.addItem(&item("Hide rustudio", Some(sel!(hide:)), "h"));
    let others = item("Hide Others", Some(sel!(hideOtherApplications:)), "h");
    others.setKeyEquivalentModifierMask(NSEventModifierFlags::Option | NSEventModifierFlags::Command);
    menu.addItem(&others);
    menu.addItem(&item("Show All", Some(sel!(unhideAllApplications:)), ""));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&item("Quit rustudio", Some(sel!(performClose:)), "q"));

    let app_item = NSMenuItem::new(mtm);
    app_item.setSubmenu(Some(&menu));
    let bar = NSMenu::new(mtm);
    bar.addItem(&app_item);
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&bar));
}
