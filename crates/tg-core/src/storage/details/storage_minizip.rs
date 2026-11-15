//! Narrow FFI to the same minizip/zlib as storage_folder_archive.cpp.
#![allow(unsafe_code)]
use std::{
    ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
#[repr(C)]
#[derive(Default)]
struct ZipInfo {
    date: [c_int; 6],
    dos_date: c_ulong,
    internal: c_ulong,
    external: c_ulong,
}
#[repr(C)]
struct FileFunctions {
    open: unsafe extern "C" fn(*mut c_void, *const c_void, c_int) -> *mut c_void,
    read: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, c_ulong) -> c_ulong,
    write: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, c_ulong) -> c_ulong,
    tell: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64,
    seek: unsafe extern "C" fn(*mut c_void, *mut c_void, u64, c_int) -> c_long,
    close: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    error: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    opaque: *mut c_void,
}
#[link(name = "minizip")]
unsafe extern "C" {
    fn zipOpen2_64(
        path: *const c_void,
        append: c_int,
        comment: *mut *const c_char,
        funcs: *mut FileFunctions,
    ) -> *mut c_void;
    fn zipOpenNewFileInZip4_64(
        file: *mut c_void,
        name: *const c_char,
        info: *const ZipInfo,
        local: *const c_void,
        local_size: c_uint,
        global: *const c_void,
        global_size: c_uint,
        comment: *const c_char,
        method: c_int,
        level: c_int,
        raw: c_int,
        window_bits: c_int,
        mem_level: c_int,
        strategy: c_int,
        password: *const c_char,
        crc: c_ulong,
        version: c_ulong,
        flag: c_ulong,
        zip64: c_int,
    ) -> c_int;
    fn zipWriteInFileInZip(file: *mut c_void, buffer: *const c_void, len: c_uint) -> c_int;
    fn zipCloseFileInZip(file: *mut c_void) -> c_int;
    fn zipClose(file: *mut c_void, comment: *const c_char) -> c_int;
}
struct Io {
    file: Option<File>,
    path: std::path::PathBuf,
    error: bool,
}
// SAFETY for callbacks: minizip calls synchronously, opaque is the stable Box<Io>
// owned by Archive until zipClose completes. Buffers are valid for their given
// lengths per minizip's ioapi64 contract. No pointer escapes that lifetime.
unsafe extern "C" fn open(p: *mut c_void, _: *const c_void, _: c_int) -> *mut c_void {
    // SAFETY: opaque lifetime and exclusive call established above.
    let io = unsafe { &mut *p.cast::<Io>() };
    match OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&io.path)
    {
        Ok(f) => {
            io.file = Some(f);
            p
        }
        Err(_) => {
            io.error = true;
            std::ptr::null_mut()
        }
    }
}
unsafe extern "C" fn read(
    p: *mut c_void,
    _: *mut c_void,
    buf: *mut c_void,
    size: c_ulong,
) -> c_ulong {
    // SAFETY: minizip supplies a writable size-byte buffer and valid opaque.
    let (io, buf) = unsafe {
        (
            &mut *p.cast::<Io>(),
            std::slice::from_raw_parts_mut(buf.cast::<u8>(), size as usize),
        )
    };
    match io.file.as_mut().map(|f| f.read(buf)) {
        Some(Ok(n)) => n as c_ulong,
        _ => {
            io.error = true;
            0
        }
    }
}
unsafe extern "C" fn write(
    p: *mut c_void,
    _: *mut c_void,
    buf: *const c_void,
    size: c_ulong,
) -> c_ulong {
    // SAFETY: minizip supplies a readable size-byte buffer and valid opaque.
    let (io, buf) = unsafe {
        (
            &mut *p.cast::<Io>(),
            std::slice::from_raw_parts(buf.cast::<u8>(), size as usize),
        )
    };
    match io.file.as_mut().map(|f| f.write(buf)) {
        Some(Ok(n)) => n as c_ulong,
        _ => {
            io.error = true;
            0
        }
    }
}
unsafe extern "C" fn tell(p: *mut c_void, _: *mut c_void) -> u64 {
    // SAFETY: valid opaque as above.
    let io = unsafe { &mut *p.cast::<Io>() };
    io.file
        .as_mut()
        .and_then(|f| f.stream_position().ok())
        .unwrap_or(u64::MAX)
}
unsafe extern "C" fn seek(p: *mut c_void, _: *mut c_void, offset: u64, origin: c_int) -> c_long {
    // SAFETY: valid opaque as above.
    let io = unsafe { &mut *p.cast::<Io>() };
    let target = match origin {
        0 => SeekFrom::Start(offset),
        1 => SeekFrom::Current(offset as i64),
        _ => SeekFrom::End(offset as i64),
    };
    if io.file.as_mut().is_some_and(|f| f.seek(target).is_ok()) {
        0
    } else {
        io.error = true;
        -1
    }
}
unsafe extern "C" fn close(p: *mut c_void, _: *mut c_void) -> c_int {
    // SAFETY: valid opaque as above; file is independently owned by Io.
    unsafe { &mut *p.cast::<Io>() }.file.take();
    0
}
unsafe extern "C" fn error(p: *mut c_void, _: *mut c_void) -> c_int {
    // SAFETY: valid opaque as above.
    if unsafe { &*p.cast::<Io>() }.error {
        -1
    } else {
        0
    }
}
pub(crate) struct Archive {
    handle: std::ptr::NonNull<c_void>,
    io: Box<Io>,
    closed: bool,
}
impl Archive {
    pub(crate) fn open(path: &Path) -> Option<Self> {
        let mut io = Box::new(Io {
            file: None,
            path: path.to_owned(),
            error: false,
        });
        let mut funcs = FileFunctions {
            open,
            read,
            write,
            tell,
            seek,
            close,
            error,
            opaque: (&mut *io as *mut Io).cast(),
        };
        // SAFETY: valid callbacks/opaque retained by Archive; filename unused by callback.
        let handle = std::ptr::NonNull::new(unsafe {
            zipOpen2_64(c"".as_ptr().cast(), 0, std::ptr::null_mut(), &mut funcs)
        })?;
        Some(Self {
            handle,
            io,
            closed: false,
        })
    }
    pub(crate) fn size(&self) -> i64 {
        std::fs::metadata(&self.io.path).map_or(0, |m| m.len() as i64)
    }
    pub(crate) fn open_entry(
        &mut self,
        name: &str,
        date: [i32; 6],
        directory: bool,
        stored: bool,
        size: i64,
    ) -> bool {
        let Ok(name) = CString::new(name.split('\0').next().unwrap_or_default()) else {
            return false;
        };
        let mut date = date;
        date[5] = date[5].max(1980);
        let info = ZipInfo {
            date,
            external: if directory { 0x10 } else { 0 },
            ..Default::default()
        };
        // SAFETY: live zip handle, all input pointers valid for this synchronous call.
        unsafe {
            zipOpenNewFileInZip4_64(
                self.handle.as_ptr(),
                name.as_ptr(),
                &info,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                if stored { 0 } else { 8 },
                if stored { 0 } else { -1 },
                0,
                -15,
                8,
                0,
                std::ptr::null(),
                0,
                0,
                1 << 11,
                i32::from(size >= 0xffff_ffff),
            ) == 0
        }
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };
        // SAFETY: live handle and immutable buffer valid for len bytes.
        unsafe { zipWriteInFileInZip(self.handle.as_ptr(), bytes.as_ptr().cast(), len) == 0 }
    }
    pub(crate) fn close_entry(&mut self) -> bool {
        // SAFETY: live handle, minizip validates whether an entry is open.
        unsafe { zipCloseFileInZip(self.handle.as_ptr()) == 0 }
    }
    pub(crate) fn finish(mut self) -> bool {
        // SAFETY: handle closed exactly once; prevent Drop from closing it again.
        let result = unsafe { zipClose(self.handle.as_ptr(), std::ptr::null()) == 0 };
        self.closed = true;
        result
    }
}
impl Drop for Archive {
    fn drop(&mut self) {
        // SAFETY: Archive uniquely owns handle and io, still live until close returns.
        if !self.closed {
            unsafe {
                zipClose(self.handle.as_ptr(), std::ptr::null());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_minizip_abi_layout_matches() {
        let row = include_str!("../../../tests/fixtures/storage-files/native.jsonl")
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|r| r["op"] == "abi")
            .unwrap();
        assert_eq!(
            std::mem::size_of::<super::ZipInfo>() as u64,
            row["zip_info"].as_u64().unwrap()
        );
        assert_eq!(
            std::mem::size_of::<super::FileFunctions>() as u64,
            row["file_funcs"].as_u64().unwrap()
        );
    }
}
