//! The parts of the window that are macOS's own: the open panel for a game, which takes an
//! app bundle as well as a folder, and Quit, which goes through the window's close check.
//!
//! Finder and the open panel treat `Knights of the Old Republic.app` as one item, not a
//! folder, so a folder picker (rfd's) cannot select it, and the game files are inside it
//! (Contents/Assets, Contents/GameData). This panel lets the player pick the game's app, the
//! folder Steam keeps it in, or the folder of game files itself; games::resource_root then
//! finds the files. It runs on the window's (main) thread, application-modal: the window
//! takes no clicks while it is open, as with the folder dialog elsewhere.

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{msg_send, sel, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSModalResponseOK, NSOpenPanel, NSWindow};
use objc2_foundation::NSString;
use std::path::PathBuf;

/// Shows the panel with `message` and returns what was picked, or None if it was cancelled
/// or this is not the main thread.
pub fn pick_game_or_folder(message: &str) -> Option<PathBuf> {
    let mtm = MainThreadMarker::new()?;
    let app = NSApplication::sharedApplication(mtm);
    let key: Option<Retained<NSWindow>> = app.keyWindow();
    // every call is made on the main thread (mtm), on a panel this function owns
    let picked = {
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(true);
        panel.setAllowsMultipleSelection(false);
        panel.setResolvesAliases(true);
        // an app is one item to pick, not a folder to open (the player need not go inside it)
        panel.setTreatsFilePackagesAsDirectories(false);
        panel.setCanCreateDirectories(false);
        panel.setMessage(Some(&NSString::from_str(message)));
        panel.setPrompt(Some(&NSString::from_str("Choose")));
        if panel.runModal() == NSModalResponseOK {
            panel.URL().and_then(|u| u.path()).map(|p| PathBuf::from(p.to_string()))
        } else {
            None
        }
    };
    if let Some(w) = key {
        w.makeKeyAndOrderFront(None);
    }
    picked
}

/// Quit (the app menu's, Command-Q, the Dock's, a log out) asks the window to close, as its
/// close button does, so the editor's check for changes not saved runs first and the
/// program then ends the usual way (the spawn records and settings are tidied after the
/// window's loop). Without this, winit ended the program at once from
/// applicationWillTerminate: the window's close check never ran and unsaved changes were
/// dropped with no word (macOS port, 2026-11-08). Done by giving winit's application
/// delegate an applicationShouldTerminate: that closes the window and answers "cancel"; the
/// window's closing then ends the program. Call once the window exists.
pub fn route_quit_through_close() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    // the delegate winit set on the application (any object: only its class is used)
    let delegate: Option<Retained<AnyObject>> = unsafe { msg_send![&*app, delegate] };
    let Some(delegate) = delegate else { return };
    let class: &AnyClass = delegate.class();
    // NSApplicationTerminateReply applicationShouldTerminate:(NSApplication *)sender
    unsafe extern "C-unwind" fn should_terminate(_this: *mut AnyObject, _cmd: Sel, _sender: *mut AnyObject) -> usize {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let window = app.keyWindow().or_else(|| app.mainWindow()).or_else(|| app.windows().firstObject());
            if let Some(w) = window {
                w.performClose(None);
            }
        }
        0 // NSTerminateCancel: the window's close ends the program, after its check
    }
    let imp: objc2::runtime::Imp = unsafe { std::mem::transmute(should_terminate as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> usize) };
    // "Q@:@": an unsigned long returned, the object, the selector, one object argument.
    // class_addMethod leaves a method the class already has as it is.
    unsafe {
        objc2::ffi::class_addMethod(class as *const AnyClass as *mut AnyClass, sel!(applicationShouldTerminate:), imp, c"Q@:@".as_ptr());
    }
}
