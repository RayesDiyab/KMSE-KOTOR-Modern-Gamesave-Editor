//! A saves folder the system will not let the editor read is reported, not skipped with no
//! word (macOS port, 2026-11-08). On a Mac the Mac App Store copies keep their saves in their
//! sandbox containers, which macOS guards from other apps. Here a made-up home folder with a
//! port's saves folder closed to its owner stands in for that. Alone in its own test program,
//! since it points HOME elsewhere while it runs.

#[cfg(unix)]
#[test]
fn a_closed_saves_folder_is_reported() {
    use kse_core::games::{self, Game};
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir().join("kmse-tests").join("blocked-home");
    let saves = home.join("Library/Application Support/Knights of the Old Republic/saves");
    let _ = std::fs::set_permissions(&saves, std::fs::Permissions::from_mode(0o755));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::set_permissions(&saves, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&saves).is_ok() {
        // run as root, which every folder lets in
        let _ = std::fs::set_permissions(&saves, std::fs::Permissions::from_mode(0o755));
        return eprintln!("this user can read a closed folder: not checked");
    }
    let real_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &home);
    let blocked = games::blocked_save_folders(&[], Game::K1);
    // an App Store container closed whole is named once, not once more for its saves folder
    let container = home.join("Library/Containers/com.aspyr.kotor");
    std::fs::create_dir_all(container.join("Data")).unwrap();
    std::fs::set_permissions(container.join("Data"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let with_container = games::blocked_save_folders(&[], Game::K1);
    std::fs::set_permissions(container.join("Data"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let none_k2 = games::blocked_save_folders(&[], Game::K2);
    match real_home {
        Some(h) => std::env::set_var("HOME", h),
        None => std::env::remove_var("HOME"),
    }
    std::fs::set_permissions(&saves, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&home);
    assert_eq!(blocked, vec![saves.clone()]);
    assert_eq!(with_container, vec![saves.clone(), container.join("Data")], "the container's Data once");
    assert!(none_k2.is_empty(), "{:?}", none_k2);
}
