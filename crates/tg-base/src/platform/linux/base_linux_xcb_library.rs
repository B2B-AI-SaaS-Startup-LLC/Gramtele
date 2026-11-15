//! Optional XCB ABI boundary. No link-time X11 dependency.
#![allow(unsafe_code)]
use super::base_linux_library::{function, Library};
use nix::libc;
use std::{
    ffi::{c_char, c_void, CString},
    marker::PhantomData,
    ptr::{self, NonNull},
    rc::Rc,
    sync::OnceLock,
};
#[repr(C)]
#[derive(Clone, Copy)]
struct Cookie {
    sequence: u32,
}
#[repr(C)]
struct Iterator {
    data: *const u8,
    rem: i32,
    index: i32,
}
#[repr(C)]
struct Extension {
    name: *const c_char,
    global_id: i32,
}
#[repr(C)]
struct Request {
    count: usize,
    ext: *mut Extension,
    opcode: u8,
    isvoid: u8,
}
static LIB: OnceLock<Option<Library>> = OnceLock::new();
fn library() -> Option<&'static Library> {
    LIB.get_or_init(|| Library::load("libxcb.so.1", libc::RTLD_NODELETE))
        .as_ref()
}
pub fn available() -> bool {
    library().is_some()
}
unsafe fn resolve<T: Copy>(name: &'static str) -> Option<T> {
    static SYMBOLS: OnceLock<
        std::sync::Mutex<std::collections::BTreeMap<&'static str, Option<usize>>>,
    > = OnceLock::new();
    let mut symbols = SYMBOLS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let pointer = (*symbols
        .entry(name)
        .or_insert_with(|| library()?.symbol(name).map(|s| s.as_ptr() as usize)))?;
    assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<usize>());
    // SAFETY: each caller supplies the exact ABI; static NODELETE library owns it.
    Some(unsafe { std::mem::transmute_copy(&pointer) })
}

struct Allocation(NonNull<u8>);
impl Allocation {
    unsafe fn from_ptr(p: *mut c_void) -> Option<Self> {
        NonNull::new(p.cast()).map(Self)
    }
    fn bytes(&self, n: usize) -> &[u8] {
        // SAFETY: callers validate n from the XCB allocation's actual reply size.
        unsafe { std::slice::from_raw_parts(self.0.as_ptr(), n) }
    }
    fn u32(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(
            self.bytes(offset + 4)[offset..offset + 4]
                .try_into()
                .unwrap(),
        )
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        // SAFETY: XCB replies/events/errors are malloc-owned full-transfer allocations.
        unsafe {
            libc::free(self.0.as_ptr().cast());
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub type_: u32,
    pub format: u8,
    pub bytes_after: u32,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event(pub [u8; 32]);
impl Event {
    pub fn kind(&self) -> u8 {
        self.0[0] & 0x7f
    }
    pub fn u32(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(self.0[offset..offset + 4].try_into().unwrap())
    }
    pub fn u16(&self, offset: usize) -> u16 {
        u16::from_ne_bytes(self.0[offset..offset + 2].try_into().unwrap())
    }
}
/// Validated, owned connection; main-thread use like the source's GUI connection.
pub struct Connection {
    pointer: NonNull<c_void>,
    _owner: PhantomData<Rc<()>>,
}
impl Connection {
    pub fn connect(display: Option<&str>) -> Option<Self> {
        let display = display.map(CString::new).transpose().ok()?;
        // SAFETY: exact C signatures; a returned connection is disconnected on error.
        unsafe {
            let connect = resolve::<unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void>(
                "xcb_connect",
            )?;
            let disconnect = resolve::<unsafe extern "C" fn(*mut c_void)>("xcb_disconnect")?;
            let error =
                resolve::<unsafe extern "C" fn(*mut c_void) -> i32>("xcb_connection_has_error")?;
            let pointer = NonNull::new(connect(
                display.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                ptr::null_mut(),
            ))?;
            if error(pointer.as_ptr()) != 0 {
                disconnect(pointer.as_ptr());
                return None;
            }
            Some(Self {
                pointer,
                _owner: PhantomData,
            })
        }
    }
    pub fn has_error(&self) -> bool {
        // SAFETY: live validated XCB connection, matching signature.
        unsafe {
            resolve::<unsafe extern "C" fn(*mut c_void) -> i32>("xcb_connection_has_error")
                .is_none_or(|f| f(self.pointer.as_ptr()) != 0)
        }
    }
    pub fn file_descriptor(&self) -> Option<i32> {
        // SAFETY: live connection; descriptor remains borrowed from it.
        unsafe {
            Some(resolve::<unsafe extern "C" fn(*mut c_void) -> i32>(
                "xcb_get_file_descriptor",
            )?(self.pointer.as_ptr()))
        }
    }
    pub fn flush(&self) -> bool {
        // SAFETY: live connection and exact XCB ABI.
        unsafe {
            resolve::<unsafe extern "C" fn(*mut c_void) -> i32>("xcb_flush")
                .is_some_and(|f| f(self.pointer.as_ptr()) > 0)
        }
    }
    pub fn generate_id(&self) -> Option<u32> {
        // SAFETY: live connection and exact XCB ABI.
        unsafe {
            Some(resolve::<unsafe extern "C" fn(*mut c_void) -> u32>(
                "xcb_generate_id",
            )?(self.pointer.as_ptr()))
        }
    }
    pub fn root_window(&self) -> Option<u32> {
        if self.has_error() {
            return None;
        }
        // SAFETY: setup/screens belong to the connection, iterator data is checked.
        unsafe {
            let setup = resolve::<unsafe extern "C" fn(*mut c_void) -> *const c_void>(
                "xcb_get_setup",
            )?(self.pointer.as_ptr());
            if setup.is_null() {
                return None;
            }
            let iterator = resolve::<unsafe extern "C" fn(*const c_void) -> Iterator>(
                "xcb_setup_roots_iterator",
            )?(setup);
            (!iterator.data.is_null())
                .then(|| ptr::read_unaligned(iterator.data.cast::<u32>()))
                .filter(|v| *v != 0)
        }
    }
    pub fn atom(&self, name: &str) -> Option<u32> {
        if self.has_error() {
            return None;
        }
        // Preserve QString::size() (UTF-16), not the UTF-8 byte count.
        let len = name.encode_utf16().count() as u16;
        // SAFETY: len cannot exceed the UTF-8 allocation; reply header has atom at8.
        unsafe {
            let request =
                resolve::<unsafe extern "C" fn(*mut c_void, u8, u16, *const c_char) -> Cookie>(
                    "xcb_intern_atom",
                )?(self.pointer.as_ptr(), 0, len, name.as_ptr().cast());
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_intern_atom_reply")?(
                self.pointer.as_ptr(), request, ptr::null_mut()
            ))?;
            Some(reply.u32(8)).filter(|n| *n != 0)
        }
    }
    pub fn property(
        &self,
        window: u32,
        property: u32,
        type_: u32,
        offset: u32,
        length: u32,
    ) -> Option<Property> {
        if self.has_error() {
            return None;
        }
        // SAFETY: the reply allocation is32+length*4 bytes; validate value_len first.
        unsafe {
            let cookie = resolve::<
                unsafe extern "C" fn(*mut c_void, u8, u32, u32, u32, u32, u32) -> Cookie,
            >("xcb_get_property")?(
                self.pointer.as_ptr(),
                0,
                window,
                property,
                type_,
                offset,
                length,
            );
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_get_property_reply")?(
                self.pointer.as_ptr(), cookie, ptr::null_mut()
            ))?;
            let format = reply.bytes(32)[1];
            let units = reply.u32(16) as usize;
            let n = units.checked_mul(format as usize / 8)?;
            if n > (reply.u32(4) as usize).checked_mul(4)? {
                return None;
            }
            Some(Property {
                type_: reply.u32(8),
                format,
                bytes_after: reply.u32(12),
                data: reply.bytes(32 + n)[32..].to_vec(),
            })
        }
    }
    pub fn window_attributes(&self, window: u32) -> Option<(u32, u8)> {
        // SAFETY: fixed get-window-attributes reply is44 bytes incl. event mask.
        unsafe {
            let cookie = resolve::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>(
                "xcb_get_window_attributes",
            )?(self.pointer.as_ptr(), window);
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_get_window_attributes_reply")?(
                self.pointer.as_ptr(), cookie, ptr::null_mut()
            ))?;
            if reply.u32(4) < 3 {
                return None;
            }
            Some((reply.u32(36), reply.bytes(44)[26]))
        }
    }
    pub fn selection_owner(&self, atom: u32) -> Option<u32> {
        // SAFETY: matching request/reply ABI; fixed reply contains owner at8.
        unsafe {
            let cookie = resolve::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>(
                "xcb_get_selection_owner",
            )?(self.pointer.as_ptr(), atom);
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_get_selection_owner_reply")?(
                self.pointer.as_ptr(), cookie, ptr::null_mut()
            ))?;
            Some(reply.u32(8)).filter(|n| *n != 0)
        }
    }
    pub fn extension(&self, name: &str) -> Option<(u8, u8, u8)> {
        // SAFETY: bounded UTF-8 request string and fixed extension reply.
        unsafe {
            let cookie = resolve::<unsafe extern "C" fn(*mut c_void, u16, *const c_char) -> Cookie>(
                "xcb_query_extension",
            )?(
                self.pointer.as_ptr(),
                name.len().try_into().ok()?,
                name.as_ptr().cast(),
            );
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_query_extension_reply")?(
                self.pointer.as_ptr(), cookie, ptr::null_mut()
            ))?;
            let b = reply.bytes(32);
            (b[8] != 0).then_some((b[9], b[10], b[11]))
        }
    }
    unsafe fn check(&self, cookie: Cookie) -> bool {
        // SAFETY: cookie was returned by a checked void request on this connection.
        unsafe {
            let Some(check) = resolve::<unsafe extern "C" fn(*mut c_void, Cookie) -> *mut c_void>(
                "xcb_request_check",
            ) else {
                return false;
            };
            let error = Allocation::from_ptr(check(self.pointer.as_ptr(), cookie));
            error.is_none() && !self.has_error()
        }
    }
    pub fn change_property(
        &self,
        window: u32,
        atom: u32,
        type_: u32,
        format: u8,
        data: &[u8],
    ) -> bool {
        if !matches!(format, 8 | 16 | 32) || !data.len().is_multiple_of(format as usize / 8) {
            return false;
        }
        // SAFETY: element count matches the buffer and XCB copies it during request.
        unsafe {
            let Some(f) = resolve::<
                unsafe extern "C" fn(
                    *mut c_void,
                    u8,
                    u32,
                    u32,
                    u32,
                    u8,
                    u32,
                    *const c_void,
                ) -> Cookie,
            >("xcb_change_property_checked") else {
                return false;
            };
            self.check(f(
                self.pointer.as_ptr(),
                0,
                window,
                atom,
                type_,
                format,
                (data.len() / (format as usize / 8)) as u32,
                data.as_ptr().cast(),
            ))
        }
    }
    pub fn change_event_mask(&self, window: u32, mask: u32) -> bool {
        // SAFETY: EVENT_MASK bit11 selects one u32 value in the list.
        unsafe {
            let Some(f) = resolve::<
                unsafe extern "C" fn(*mut c_void, u32, u32, *const c_void) -> Cookie,
            >("xcb_change_window_attributes_checked") else {
                return false;
            };
            self.check(f(
                self.pointer.as_ptr(),
                window,
                2048,
                ptr::from_ref(&mask).cast(),
            ))
        }
    }
    pub fn map_window(&self, window: u32) -> bool {
        // SAFETY: live connection and checked void request.
        unsafe {
            resolve::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>("xcb_map_window_checked")
                .is_some_and(|f| self.check(f(self.pointer.as_ptr(), window)))
        }
    }
    pub fn raise_window(&self, window: u32) -> bool {
        // SAFETY: STACK_MODE bit6 selects the single Above(0) u32 value.
        unsafe {
            let Some(f) = resolve::<
                unsafe extern "C" fn(*mut c_void, u32, u16, *const c_void) -> Cookie,
            >("xcb_configure_window_checked") else {
                return false;
            };
            self.check(f(
                self.pointer.as_ptr(),
                window,
                64,
                ptr::from_ref(&0u32).cast(),
            ))
        }
    }
    pub fn send_event(&self, destination: u32, mask: u32, event: &Event) -> bool {
        // SAFETY: core event has exactly32 bytes as required by XCB SendEvent.
        unsafe {
            resolve::<unsafe extern "C" fn(*mut c_void, u8, u32, u32, *const c_char) -> Cookie>(
                "xcb_send_event_checked",
            )
            .is_some_and(|f| {
                self.check(f(
                    self.pointer.as_ptr(),
                    0,
                    destination,
                    mask,
                    event.0.as_ptr().cast(),
                ))
            })
        }
    }
    pub fn reset_screen_saver(&self) -> bool {
        // SAFETY: Reset(0), checked void request on live connection.
        unsafe {
            resolve::<unsafe extern "C" fn(*mut c_void, u8) -> Cookie>(
                "xcb_force_screen_saver_checked",
            )
            .is_some_and(|f| self.check(f(self.pointer.as_ptr(), 0)))
        }
    }
    pub fn poll_event(&self) -> Option<Event> {
        // SAFETY: core prefix always32 bytes; XCB's full allocation freed afterward.
        unsafe {
            let p =
                Allocation::from_ptr(resolve::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(
                    "xcb_poll_for_event",
                )?(self.pointer.as_ptr()))?;
            Some(Event(p.bytes(32).try_into().unwrap()))
        }
    }
    /// QueryInputFocus roundtrip also flushes pending events, as in GetTimestamp.
    pub fn sync(&self) -> bool {
        // SAFETY: matching fixed request/reply, reply freed whether unused or valid.
        unsafe {
            let Some(get) =
                resolve::<unsafe extern "C" fn(*mut c_void) -> Cookie>("xcb_get_input_focus")
            else {
                return false;
            };
            let Some(reply) = resolve::<
                unsafe extern "C" fn(*mut c_void, Cookie, *mut *mut c_void) -> *mut c_void,
            >("xcb_get_input_focus_reply") else {
                return false;
            };
            Allocation::from_ptr(reply(
                self.pointer.as_ptr(),
                get(self.pointer.as_ptr()),
                ptr::null_mut(),
            ))
            .is_some()
        }
    }
    pub fn screen_saver_idle(&self) -> Option<u32> {
        // SAFETY: MIT-SCREEN-SAVER extension descriptor must persist for XCB cache.
        unsafe {
            static EXT: std::sync::Mutex<Extension> = std::sync::Mutex::new(Extension {
                name: c"MIT-SCREEN-SAVER".as_ptr(),
                global_id: 0,
            });
            let mut ext = EXT.lock().unwrap_or_else(|e| e.into_inner());
            let data = resolve::<unsafe extern "C" fn(*mut c_void, *mut Extension) -> *const u8>(
                "xcb_get_extension_data",
            )?(self.pointer.as_ptr(), &mut *ext);
            if data.is_null() || *data.add(8) == 0 {
                return None;
            }
            let root = self.root_window()?;
            let mut body = [0u8; 8];
            body[4..8].copy_from_slice(&root.to_ne_bytes());
            let mut parts = [libc::iovec {
                iov_base: ptr::null_mut(),
                iov_len: 0,
            }; 4];
            parts[2] = libc::iovec {
                iov_base: body.as_mut_ptr().cast(),
                iov_len: 8,
            };
            let request = Request {
                count: 2,
                ext: &mut *ext,
                opcode: 1,
                isvoid: 0,
            };
            let send = resolve::<
                unsafe extern "C" fn(*mut c_void, i32, *mut libc::iovec, *const Request) -> u32,
            >("xcb_send_request")?;
            let sequence = send(
                self.pointer.as_ptr(),
                1,
                parts.as_mut_ptr().add(2),
                &request,
            );
            let reply = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, u32, *mut *mut c_void) -> *mut c_void,
            >("xcb_wait_for_reply")?(
                self.pointer.as_ptr(), sequence, ptr::null_mut()
            ))?;
            Some(reply.u32(16))
        }
    }
    /// Blocking descriptor readiness only; event dispatch remains in the owner.
    pub fn wait_readable(&self, timeout_ms: i32) -> bool {
        let Some(fd) = self.file_descriptor() else {
            return false;
        };
        let mut p = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd; no buffer reads or ownership transfer.
        unsafe { libc::poll(&mut p, 1, timeout_ms) > 0 }
    }
}
// SAFETY: descriptor name is static; mutable global_id is accessed under EXT mutex.
unsafe impl Send for Extension {}
impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: owned connection disconnected once, all dependent leases retain Rc.
        unsafe {
            if let Some(f) = resolve::<unsafe extern "C" fn(*mut c_void)>("xcb_disconnect") {
                f(self.pointer.as_ptr());
            }
        }
    }
}

impl Connection {
    pub fn send_raw_core(&self, opcode: u8, body: &[u8], is_void: bool) -> Option<u32> {
        if body.len() < 4 || !body.len().is_multiple_of(4) {
            return None;
        }
        let mut bytes = body.to_vec();
        let mut parts = [libc::iovec {
            iov_base: ptr::null_mut(),
            iov_len: 0,
        }; 4];
        parts[2] = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        let request = Request {
            count: 2,
            ext: ptr::null_mut(),
            opcode,
            isvoid: u8::from(is_void),
        };
        // SAFETY: XCB requires two writable leading iovecs, followed by body/padding.
        unsafe {
            let sequence = resolve::<
                unsafe extern "C" fn(*mut c_void, i32, *mut libc::iovec, *const Request) -> u32,
            >("xcb_send_request")?(
                self.pointer.as_ptr(),
                1,
                parts.as_mut_ptr().add(2),
                &request,
            );
            if is_void && !self.check(Cookie { sequence }) {
                None
            } else {
                Some(sequence)
            }
        }
    }
    pub fn wait_reply(&self, sequence: u32) -> Option<Vec<u8>> {
        // SAFETY: generic replies have a32-byte header and length additional u32s.
        unsafe {
            let r = Allocation::from_ptr(resolve::<
                unsafe extern "C" fn(*mut c_void, u32, *mut *mut c_void) -> *mut c_void,
            >("xcb_wait_for_reply")?(
                self.pointer.as_ptr(), sequence, ptr::null_mut()
            ))?;
            let size = 32usize.checked_add((r.u32(4) as usize).checked_mul(4)?)?;
            Some(r.bytes(size).to_vec())
        }
    }
}
#[repr(C, align(2))]
struct RecordRange([u8; 24]);
static RECORD: OnceLock<Option<Library>> = OnceLock::new();
static KEYSYMS: OnceLock<Option<Library>> = OnceLock::new();
fn record_library() -> Option<&'static Library> {
    RECORD
        .get_or_init(|| Library::load("libxcb-record.so.0", libc::RTLD_NODELETE))
        .as_ref()
}
fn key_library() -> Option<&'static Library> {
    KEYSYMS
        .get_or_init(|| Library::load("libxcb-keysyms.so.1", libc::RTLD_NODELETE))
        .as_ref()
}
/// RECORD plus keysyms use the same optional loader, not a build-time X11 link.
pub struct Record {
    connection: Rc<Connection>,
    symbols: NonNull<c_void>,
    context: u32,
    sequence: u32,
}
impl Record {
    pub fn new(connection: Rc<Connection>) -> Option<Self> {
        if connection.has_error() {
            return None;
        }
        let record = record_library()?;
        let key = key_library()?;
        for name in [
            "xcb_record_id",
            "xcb_record_create_context_checked",
            "xcb_record_enable_context",
            "xcb_record_disable_context",
            "xcb_record_free_context",
            "xcb_record_enable_context_data",
            "xcb_record_enable_context_data_length",
        ] {
            if !record.has_symbol(name) {
                return None;
            }
        }
        for name in [
            "xcb_key_symbols_alloc",
            "xcb_key_symbols_free",
            "xcb_key_symbols_get_keysym",
            "xcb_is_keypad_key",
            "xcb_is_private_keypad_key",
        ] {
            if !key.has_symbol(name) {
                return None;
            }
        }
        // SAFETY: all symbols prevalidated, xcb_record_id is a data symbol.
        unsafe {
            let ext = record.symbol("xcb_record_id")?.as_ptr().cast::<Extension>();
            let available =
                resolve::<unsafe extern "C" fn(*mut c_void, *mut Extension) -> *const u8>(
                    "xcb_get_extension_data",
                )?(connection.pointer.as_ptr(), ext);
            if available.is_null() || *available.add(8) == 0 {
                return None;
            }
            let symbols = NonNull::new(function::<
                unsafe extern "C" fn(*mut c_void) -> *mut c_void,
            >(key, "xcb_key_symbols_alloc")?(
                connection.pointer.as_ptr()
            ))?;
            let mut state = Self {
                connection,
                symbols,
                context: 0,
                sequence: 0,
            };
            let context = state.connection.generate_id()?;
            let mut range = RecordRange([0; 24]);
            range.0[18] = 2;
            range.0[19] = 5;
            let create = function::<
                unsafe extern "C" fn(
                    *mut c_void,
                    u32,
                    u8,
                    u32,
                    u32,
                    *const u32,
                    *const RecordRange,
                ) -> Cookie,
            >(record, "xcb_record_create_context_checked")?;
            if !state.connection.check(create(
                state.connection.pointer.as_ptr(),
                context,
                0,
                1,
                1,
                &3,
                &range,
            )) {
                return None;
            }
            state.context = context;
            state.sequence = function::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>(
                record,
                "xcb_record_enable_context",
            )?(state.connection.pointer.as_ptr(), context)
            .sequence;
            state.connection.flush();
            Some(state)
        }
    }
    pub fn available(&self) -> bool {
        self.sequence != 0
    }
    pub fn connection(&self) -> Rc<Connection> {
        self.connection.clone()
    }
    pub fn keysym(&self, keycode: u8, column: i32) -> Option<u32> {
        // SAFETY: symbols allocation belongs to this live connection.
        unsafe {
            Some(
                function::<unsafe extern "C" fn(*mut c_void, u8, i32) -> u32>(
                    key_library()?,
                    "xcb_key_symbols_get_keysym",
                )?(self.symbols.as_ptr(), keycode, column),
            )
        }
    }
    pub fn poll(&self) -> Vec<Vec<Event>> {
        let mut batches = vec![];
        // SAFETY: XCB poll returns owned reply/error or both null on I/O failure.
        unsafe {
            while self.connection.poll_event().is_some() {}
            let Some(poll) = resolve::<
                unsafe extern "C" fn(*mut c_void, u32, *mut *mut c_void, *mut *mut c_void) -> i32,
            >("xcb_poll_for_reply") else {
                return batches;
            };
            if self.sequence == 0 {
                return batches;
            }
            loop {
                let mut reply = ptr::null_mut();
                let mut error = ptr::null_mut();
                if poll(
                    self.connection.pointer.as_ptr(),
                    self.sequence,
                    &mut reply,
                    &mut error,
                ) == 0
                {
                    break;
                }
                let r = Allocation::from_ptr(reply);
                let e = Allocation::from_ptr(error);
                if self.connection.has_error() || e.is_some() {
                    break;
                }
                let Some(r) = r else {
                    continue;
                };
                let record = record_library().unwrap();
                let data = function::<unsafe extern "C" fn(*const c_void) -> *const u8>(
                    record,
                    "xcb_record_enable_context_data",
                )
                .unwrap()(r.0.as_ptr().cast());
                let n = function::<unsafe extern "C" fn(*const c_void) -> i32>(
                    record,
                    "xcb_record_enable_context_data_length",
                )
                .unwrap()(r.0.as_ptr().cast());
                if n < 0 || n as usize > r.u32(4) as usize * 4 || data.is_null() {
                    break;
                }
                let events = std::slice::from_raw_parts(data, n as usize)
                    .chunks_exact(32)
                    .map(|b| Event(b.try_into().unwrap()))
                    .collect();
                batches.push(events);
            }
        }
        batches
    }
}
impl Drop for Record {
    fn drop(&mut self) {
        // SAFETY: context/symbols created by these libraries, destroyed before connection.
        unsafe {
            if let Some(r) = record_library() {
                if self.sequence != 0 {
                    if let Some(f) = function::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>(
                        r,
                        "xcb_record_disable_context",
                    ) {
                        f(self.connection.pointer.as_ptr(), self.context);
                    }
                }
                if self.context != 0 {
                    if let Some(f) = function::<unsafe extern "C" fn(*mut c_void, u32) -> Cookie>(
                        r,
                        "xcb_record_free_context",
                    ) {
                        f(self.connection.pointer.as_ptr(), self.context);
                    }
                }
            }
            if let Some(k) = key_library() {
                if let Some(f) =
                    function::<unsafe extern "C" fn(*mut c_void)>(k, "xcb_key_symbols_free")
                {
                    f(self.symbols.as_ptr());
                }
            }
        }
    }
}
