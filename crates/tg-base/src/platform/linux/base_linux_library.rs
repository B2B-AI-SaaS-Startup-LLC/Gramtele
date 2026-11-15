//! Narrow POSIX FFI boundary; optional libraries/symbols fail without substitutes.
#![allow(unsafe_code)]
use nix::libc;
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr::NonNull,
};
pub struct Library {
    handle: NonNull<libc::c_void>,
}
// SAFETY: loader handles permit concurrent dlsym; Drop requires exclusive ownership.
unsafe impl Send for Library {}
// SAFETY: shared borrows prevent dlclose while a symbol lookup is in flight.
unsafe impl Sync for Library {}
pub struct Symbol<'a> {
    pointer: NonNull<libc::c_void>,
    _library: PhantomData<&'a Library>,
}
impl Symbol<'_> {
    pub fn as_ptr(&self) -> *mut libc::c_void {
        self.pointer.as_ptr()
    }
}
impl Library {
    pub fn load(name: &str, flags: i32) -> Option<Self> {
        let name = CString::new(name).ok()?;
        // SAFETY: live NUL-terminated name; dlopen returns an owned loader handle.
        NonNull::new(unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LAZY | flags) })
            .map(|handle| Self { handle })
    }
    pub fn symbol(&self, name: &str) -> Option<Symbol<'_>> {
        let name = CString::new(name).ok()?;
        // SAFETY: the library handle is alive and name is NUL-terminated.
        NonNull::new(unsafe { libc::dlsym(self.handle.as_ptr(), name.as_ptr()) }).map(|pointer| {
            Symbol {
                pointer,
                _library: PhantomData,
            }
        })
    }
    pub fn has_symbol(&self, name: &str) -> bool {
        self.symbol(name).is_some()
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: this handle was returned by dlopen and is closed exactly once.
        unsafe {
            libc::dlclose(self.handle.as_ptr());
        }
    }
}
/// Source battery checks the symbols visible in its already-linked process.
pub fn process_has_symbol(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: RTLD_DEFAULT is the POSIX pseudo-handle; reset error after lookup.
    unsafe {
        let result = !libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()).is_null();
        libc::dlerror();
        result
    }
}
/// Signature validation stays inside this FFI module, never in application code.
pub(super) unsafe fn function<T: Copy>(lib: &Library, name: &str) -> Option<T> {
    let symbol = lib.symbol(name)?;
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<*mut libc::c_void>()
    );
    // SAFETY: callers use the exact C ABI declaration of the named symbol.
    Some(unsafe { std::mem::transmute_copy(&symbol.as_ptr()) })
}
pub fn libc_version() -> (String, String) {
    #[cfg(target_env = "gnu")]
    {
        let Some(lib) = Library::load("libc.so.6", 0) else {
            return ("glibc".into(), String::new());
        };
        // SAFETY: glibc declares this exact no-argument, static string function.
        let version = unsafe {
            function::<unsafe extern "C" fn() -> *const libc::c_char>(&lib, "gnu_get_libc_version")
                .map(|f| {
                    let p = f();
                    if p.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(p).to_string_lossy().into_owned()
                    }
                })
        };
        ("glibc".into(), version.unwrap_or_default())
    }
    #[cfg(not(target_env = "gnu"))]
    {
        (String::new(), String::new())
    }
}
pub fn uname() -> Option<(String, String)> {
    let mut value = std::mem::MaybeUninit::<libc::utsname>::uninit();
    // SAFETY: uname initializes utsname on success, including terminated fields.
    unsafe {
        if libc::uname(value.as_mut_ptr()) != 0 {
            return None;
        }
        let value = value.assume_init();
        Some((
            CStr::from_ptr(value.sysname.as_ptr())
                .to_string_lossy()
                .into_owned(),
            CStr::from_ptr(value.release.as_ptr())
                .to_string_lossy()
                .into_owned(),
        ))
    }
}
/// None means loader/symbol unavailable: only that branch falls back to env.
pub fn probe_wayland() -> Option<bool> {
    #[cfg(target_os = "linux")]
    let flags = libc::RTLD_NODELETE;
    #[cfg(not(target_os = "linux"))]
    let flags = 0;
    let lib = Library::load("libwayland-client.so.0", flags)?;
    // SAFETY: wl_display C ABI; only a non-null connection is disconnected.
    unsafe {
        let connect = function::<unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void>(
            &lib,
            "wl_display_connect",
        )?;
        let disconnect =
            function::<unsafe extern "C" fn(*mut libc::c_void)>(&lib, "wl_display_disconnect")?;
        let display = connect(std::ptr::null());
        if !display.is_null() {
            disconnect(display);
        }
        Some(!display.is_null())
    }
}
struct Gio {
    lib: Library,
    unref: unsafe extern "C" fn(*mut libc::c_void),
}
impl Gio {
    fn new() -> Option<Self> {
        // GTypes retain function pointers just like the original linked GIO.
        #[cfg(target_os = "linux")]
        let flags = libc::RTLD_NODELETE;
        #[cfg(not(target_os = "linux"))]
        let flags = {
            static RESIDENT: std::sync::Once = std::sync::Once::new();
            RESIDENT.call_once(|| {
                if let Some(lib) = Library::load("libgio-2.0.so.0", 0) {
                    std::mem::forget(lib);
                }
            });
            0
        };
        let lib = Library::load("libgio-2.0.so.0", flags)?;
        // SAFETY: g_object_unref has this exact C ABI and is in GIO's dependencies.
        let unref = unsafe { function(&lib, "g_object_unref")? };
        Some(Self { lib, unref })
    }
    fn text(&self, object: *mut libc::c_void, name: &str) -> Option<String> {
        // SAFETY: callers pass an owned live GAppInfo and a string getter symbol.
        unsafe {
            let get = function::<unsafe extern "C" fn(*mut libc::c_void) -> *const libc::c_char>(
                &self.lib, name,
            )?;
            let p = get(object);
            (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
        }
    }
    fn set_default(&self, app: *mut libc::c_void, mime: &CStr) -> bool {
        // SAFETY: app is a live GAppInfo; a null GError** discards errors as allowed.
        unsafe {
            function::<
                unsafe extern "C" fn(
                    *mut libc::c_void,
                    *const libc::c_char,
                    *mut *mut libc::c_void,
                ) -> i32,
            >(&self.lib, "g_app_info_set_as_default_for_type")
            .is_some_and(|f| f(app, mime.as_ptr(), std::ptr::null_mut()) != 0)
        }
    }
    fn release(&self, app: *mut libc::c_void) {
        if !app.is_null() {
            // SAFETY: each full-transfer GAppInfo reference is released once.
            unsafe {
                (self.unref)(app);
            }
        }
    }
}
pub fn gio_check_url(mime: &str, needed: &str) -> bool {
    let Some(gio) = Gio::new() else {
        return false;
    };
    let Ok(mime) = CString::new(mime) else {
        return false;
    };
    // SAFETY: exact GIO signature, valid MIME string; returned full reference owned.
    unsafe {
        let Some(get) = function::<
            unsafe extern "C" fn(*const libc::c_char, i32) -> *mut libc::c_void,
        >(&gio.lib, "g_app_info_get_default_for_type") else {
            return false;
        };
        let app = get(mime.as_ptr(), 1);
        if app.is_null() {
            return false;
        }
        let result = gio
            .text(app, "g_app_info_get_commandline")
            .is_some_and(|s| s == format!("{needed} %u") || s == format!("{needed} %U"));
        gio.release(app);
        result
    }
}
pub fn gio_register_url(mime: &str, command: &str, app_id: &str, display_name: &str) -> bool {
    let Some(gio) = Gio::new() else {
        return false;
    };
    let (Ok(mime), Ok(command), Ok(id), Ok(display)) = (
        CString::new(mime),
        CString::new(command),
        CString::new(format!("{app_id}.desktop")),
        CString::new(display_name),
    ) else {
        return false;
    };
    // SAFETY: GIO C ABI matches declarations; strings live across every call.
    unsafe {
        if !app_id.is_empty() {
            if let Some(new) = function::<
                unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void,
            >(&gio.lib, "g_desktop_app_info_new")
            {
                let app = new(id.as_ptr());
                if !app.is_null() {
                    let matches = gio
                        .text(app, "g_app_info_get_commandline")
                        .is_some_and(|s| {
                            s == format!("{} %u", command.to_string_lossy())
                                || s == format!("{} %U", command.to_string_lossy())
                        });
                    if matches {
                        let ok = gio.set_default(app, &mime);
                        gio.release(app);
                        return ok;
                    }
                    gio.release(app);
                }
            }
        }
        let Some(create) = function::<
            unsafe extern "C" fn(
                *const libc::c_char,
                *const libc::c_char,
                i32,
                *mut *mut libc::c_void,
            ) -> *mut libc::c_void,
        >(&gio.lib, "g_app_info_create_from_commandline") else {
            return false;
        };
        let app = create(command.as_ptr(), display.as_ptr(), 6, std::ptr::null_mut());
        if app.is_null() {
            return false;
        }
        let result = gio.set_default(app, &mime);
        gio.release(app);
        result
    }
}
#[repr(C)]
struct GList {
    data: *mut libc::c_void,
    next: *mut GList,
    prev: *mut GList,
}
pub fn gio_unregister_url(mime: &str, needed: &str) {
    let Some(gio) = Gio::new() else {
        return;
    };
    let Ok(mime) = CString::new(mime) else {
        return;
    };
    // SAFETY: returned GList owns GAppInfo references; unref entries then free list.
    unsafe {
        let Some(get) = function::<unsafe extern "C" fn(*const libc::c_char) -> *mut GList>(
            &gio.lib,
            "g_app_info_get_recommended_for_type",
        ) else {
            return;
        };
        let Some(delete) = function::<unsafe extern "C" fn(*mut libc::c_void) -> i32>(
            &gio.lib,
            "g_app_info_delete",
        ) else {
            return;
        };
        let Some(free) = function::<unsafe extern "C" fn(*mut GList)>(&gio.lib, "g_list_free")
        else {
            return;
        };
        let list = get(mime.as_ptr());
        let mut node = list;
        while !node.is_null() {
            let app = (*node).data;
            if !app.is_null() {
                if gio.text(app, "g_app_info_get_commandline").as_deref() == Some(needed)
                    && gio
                        .text(app, "g_app_info_get_id")
                        .is_some_and(|s| s.starts_with("userapp-"))
                {
                    delete(app);
                }
                gio.release(app);
            }
            node = (*node).next;
        }
        free(list);
    }
}

pub fn filename_to_uri(path: &std::path::Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let gio = Gio::new()?;
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: GLib signature, allocated result is released with its matching free.
    unsafe {
        let convert = function::<
            unsafe extern "C" fn(
                *const libc::c_char,
                *const libc::c_char,
                *mut *mut libc::c_void,
            ) -> *mut libc::c_char,
        >(&gio.lib, "g_filename_to_uri")?;
        let free = function::<unsafe extern "C" fn(*mut libc::c_void)>(&gio.lib, "g_free")?;
        let result = convert(path.as_ptr(), std::ptr::null(), std::ptr::null_mut());
        if result.is_null() {
            return None;
        }
        let text = CStr::from_ptr(result).to_string_lossy().into_owned();
        free(result.cast());
        Some(text)
    }
}
/// The C++ binary links GIO; the Rust binding resolves the optional SONAME.
pub fn gio_power_capabilities() -> (bool, bool) {
    let Some(gio) = Gio::new() else {
        return (false, false);
    };
    (
        gio.lib.has_symbol("g_power_profile_monitor_dup_default"),
        gio.lib
            .has_symbol("g_power_profile_monitor_get_power_saver_enabled"),
    )
}
