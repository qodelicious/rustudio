//! What differs between the desktops rustudio runs on (Linux, macOS): where
//! per-user files live, process statistics, the desktop trash.
//!
//! Folders, per [`Dir`]:
//!
//! | `Dir`  | Linux             | macOS                          |
//! |--------|-------------------|--------------------------------|
//! | Data   | `~/.local/share`  | `~/Library/Application Support` |
//! | Config | `~/.config`       | `~/Library/Application Support` |
//! | State  | `~/.local/state`  | `~/Library/Application Support` |
//! | Cache  | `~/.cache`        | `~/Library/Caches`             |
//!
//! [`xdg_dir`] also follows `$XDG_*_HOME` on Linux; macOS has no such
//! convention, so there it is the same as [`home_dir_of`]. The `RUSTUDIO_*`
//! overrides each caller reads come first everywhere.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Data,
    Config,
    State,
    Cache,
}

/// The home folder (`$HOME`; `.` when unset, as rustudio always did).
pub fn home() -> PathBuf {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// The per-user base folder for `d`, ignoring `$XDG_*_HOME` (the paths the
/// docs name, e.g. `~/.local/share/rustudio/studio` on Linux).
pub fn home_dir_of(d: Dir) -> PathBuf {
    base_in(&home(), d)
}

fn base_in(home: &Path, d: Dir) -> PathBuf {
    if cfg!(target_os = "macos") {
        match d {
            Dir::Cache => home.join("Library/Caches"),
            Dir::Data | Dir::Config | Dir::State => home.join("Library/Application Support"),
        }
    } else {
        match d {
            Dir::Data => home.join(".local/share"),
            Dir::Config => home.join(".config"),
            Dir::State => home.join(".local/state"),
            Dir::Cache => home.join(".cache"),
        }
    }
}

/// The per-user base folder for `d`; on Linux `$XDG_DATA_HOME` /
/// `$XDG_CONFIG_HOME` / `$XDG_STATE_HOME` / `$XDG_CACHE_HOME` win when set.
pub fn xdg_dir(d: Dir) -> PathBuf {
    if !cfg!(target_os = "macos") {
        let var = match d {
            Dir::Data => "XDG_DATA_HOME",
            Dir::Config => "XDG_CONFIG_HOME",
            Dir::State => "XDG_STATE_HOME",
            Dir::Cache => "XDG_CACHE_HOME",
        };
        if let Some(p) = std::env::var_os(var).filter(|p| !p.is_empty()) {
            return PathBuf::from(p);
        }
    }
    home_dir_of(d)
}

/// rustudio's own folder under [`home_dir_of`].
pub fn rustudio_dir(d: Dir) -> PathBuf {
    home_dir_of(d).join("rustudio")
}

/// Roblox Studio installed on this machine (macOS: `RobloxStudio.app` in
/// /Applications or ~/Applications): the folder holding `content/`,
/// `ExtraContent/` and `PlatformContent/`, the same layout as the Windows
/// install and the CDN packages. None elsewhere.
pub fn installed_studio() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    [PathBuf::from("/Applications"), home().join("Applications")]
        .into_iter()
        .map(|a| a.join("RobloxStudio.app/Contents/Resources"))
        .find(|r| r.join("ExtraContent/scripts/PlayerScripts").is_dir() && r.join("content").is_dir())
}

/// This process's resident memory in bytes.
pub fn resident_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let pages: u64 = std::fs::read_to_string("/proc/self/statm").ok()?.split_whitespace().nth(1)?.parse().ok()?;
        // SAFETY: sysconf has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Some(pages * u64::try_from(page).ok()?)
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: task_info on our own task with a buffer of the size it is told.
        unsafe {
            let mut info: libc::mach_task_basic_info = std::mem::zeroed();
            let mut count = (std::mem::size_of::<libc::mach_task_basic_info>() / std::mem::size_of::<libc::natural_t>())
                as libc::mach_msg_type_number_t;
            #[allow(deprecated)]
            let task = libc::mach_task_self();
            let r = libc::task_info(
                task,
                libc::MACH_TASK_BASIC_INFO,
                (&raw mut info).cast(),
                &mut count,
            );
            (r == libc::KERN_SUCCESS).then_some(info.resident_size)
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// The 1-minute load average, as text ("?" when unknown).
pub fn load_average() -> String {
    let mut l = [0f64; 3];
    // SAFETY: getloadavg writes at most the 1 entry asked for.
    if unsafe { libc::getloadavg(l.as_mut_ptr(), 1) } == 1 {
        format!("{:.2}", l[0])
    } else {
        "?".into()
    }
}

/// Moves `path` to the desktop's trash the native way (macOS: Finder's
/// Trash, so Put Back works). Some(Ok) when moved; None where the caller
/// keeps its own trash (Linux: the XDG trash in project_write.rs).
pub fn native_trash(path: &Path) -> Option<Result<(), String>> {
    #[cfg(target_os = "macos")]
    {
        Some(trash_item(path).map(|_| ()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

/// NSFileManager's trash: where the item ended up.
#[cfg(target_os = "macos")]
fn trash_item(path: &Path) -> Result<Option<PathBuf>, String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let abs = std::path::absolute(path).map_err(|e| e.to_string())?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&abs.to_string_lossy()));
    let mut out = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, Some(&mut out))
        .map_err(|e| e.localizedDescription().to_string())?;
    Ok(out.and_then(|u| u.path()).map(|p| PathBuf::from(p.to_string())))
}

/// Font files the system has for symbols and scripts rustudio's own fonts
/// lack, in the order to try them.
pub fn fallback_font_files() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["/System/Library/Fonts/Apple Symbols.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"]
    } else {
        &[
            "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
    }
}

/// The names to dlopen a shared library by: `stem` is the library without
/// prefix or suffix ("vpx"), `linux_versions` the sonames to try first
/// ("12" -> libvpx.so.12). macOS also looks in Homebrew's and MacPorts'
/// lib folders, which aren't on the default search path.
pub fn shared_library_names(stem: &str, linux_versions: &[&str]) -> Vec<std::ffi::CString> {
    let mut v: Vec<String> = Vec::new();
    if cfg!(target_os = "macos") {
        for dir in ["", "/opt/homebrew/lib/", "/usr/local/lib/", "/opt/local/lib/"] {
            for ver in linux_versions {
                v.push(format!("{dir}lib{stem}.{ver}.dylib"));
            }
            v.push(format!("{dir}lib{stem}.dylib"));
        }
    } else {
        for ver in linux_versions {
            v.push(format!("lib{stem}.so.{ver}"));
        }
        v.push(format!("lib{stem}.so"));
    }
    v.into_iter().filter_map(|s| std::ffi::CString::new(s).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_per_platform() {
        let h = Path::new("/home/u");
        if cfg!(target_os = "macos") {
            assert_eq!(base_in(h, Dir::Data), h.join("Library/Application Support"));
            assert_eq!(base_in(h, Dir::Config), h.join("Library/Application Support"));
            assert_eq!(base_in(h, Dir::Cache), h.join("Library/Caches"));
        } else {
            assert_eq!(base_in(h, Dir::Data), h.join(".local/share"));
            assert_eq!(base_in(h, Dir::Config), h.join(".config"));
            assert_eq!(base_in(h, Dir::State), h.join(".local/state"));
            assert_eq!(base_in(h, Dir::Cache), h.join(".cache"));
        }
    }

    #[test]
    fn process_stats() {
        let rss = resident_bytes().expect("resident memory");
        assert!(rss > 1 << 20, "{rss}");
        assert_ne!(load_average(), "?");
    }

    #[test]
    fn library_names() {
        let n: Vec<String> = shared_library_names("vpx", &["9"]).iter().map(|c| c.to_string_lossy().into_owned()).collect();
        if cfg!(target_os = "macos") {
            assert!(n.contains(&"/opt/homebrew/lib/libvpx.dylib".to_string()), "{n:?}");
        } else {
            assert_eq!(n, ["libvpx.so.9", "libvpx.so"]);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn trash_moves_to_finder_trash() {
        let dir = std::env::temp_dir().join(format!("rustudio-trash-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("gone.txt");
        std::fs::write(&f, "x").unwrap();
        match trash_item(&f) {
            Ok(put) => {
                assert!(!f.exists());
                // Leave nothing behind in the person's Trash.
                if let Some(p) = put {
                    let _ = std::fs::remove_file(p);
                }
            }
            // A runner may have no Trash.
            Err(e) => eprintln!("trash unavailable here: {e}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
