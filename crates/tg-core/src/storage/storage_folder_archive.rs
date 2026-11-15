//! Folder/file gathering and minizip archive writer. UI PreparedFile construction
//! belongs to stage5 attach_prepare; ArchiveJob is its actual folder/root/paths data.
use super::details::storage_minizip::Archive;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub const ARCHIVE_CHUNK_SIZE: usize = 1024 * 1024;
pub const ARCHIVE_ENTRY_OVERHEAD: i64 = 128;
pub const STALE_ARCHIVE_TIMEOUT: u64 = 24 * 60 * 60;
#[derive(Clone, Debug)]
pub struct ArchiveEntry {
    pub absolute: PathBuf,
    pub name: String,
    /// seconds, minutes, hours, day, month-1, full year
    pub modified: [i32; 6],
    pub size: i64,
    pub directory: bool,
}
#[derive(Clone, Debug, Default)]
pub struct ArchiveEntries {
    pub list: Vec<ArchiveEntry>,
    pub total: i64,
}
#[derive(Clone, Debug, Default)]
pub struct ArchiveJob {
    pub folder: PathBuf,
    pub root: String,
    pub paths: Vec<PathBuf>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Done,
    TooLarge,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug)]
pub struct ArchiveWriteResult {
    pub status: Status,
    pub path: PathBuf,
    pub size: i64,
}
pub fn archive_root_name(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Archive")
        .into()
}
pub fn common_parent_name(paths: &[PathBuf]) -> String {
    let Some(first) = paths.first().and_then(|p| p.parent()) else {
        return String::new();
    };
    if paths.iter().all(|p| p.parent() == Some(first)) {
        archive_root_name(first)
    } else {
        String::new()
    }
}
pub fn already_compressed_name(name: &str) -> bool {
    let slash = name.rfind('/').map_or(-1, |i| i as isize);
    let Some(dot) = name.rfind('.') else {
        return false;
    };
    if dot as isize <= slash + 1 {
        return false;
    }
    matches!(
        name[dot + 1..].to_lowercase().as_str(),
        "3gp"
            | "7z"
            | "aac"
            | "apk"
            | "avi"
            | "avif"
            | "bz2"
            | "cab"
            | "docx"
            | "epub"
            | "flac"
            | "flv"
            | "gif"
            | "gz"
            | "heic"
            | "heif"
            | "jar"
            | "jpeg"
            | "jpg"
            | "jxl"
            | "m4a"
            | "m4v"
            | "mkv"
            | "mov"
            | "mp3"
            | "mp4"
            | "odp"
            | "ods"
            | "odt"
            | "oga"
            | "ogg"
            | "opus"
            | "png"
            | "pptx"
            | "rar"
            | "tgz"
            | "webm"
            | "webp"
            | "wma"
            | "wmv"
            | "xlsx"
            | "xz"
            | "zip"
            | "zst"
    )
}
pub fn deduped_name(counts: &mut BTreeMap<String, i32>, name: &str) -> String {
    let key = name.to_lowercase();
    let count = counts.entry(key.clone()).or_default();
    *count += 1;
    if *count == 1 {
        return name.into();
    }
    let dot = name.rfind('.').filter(|i| *i > 0);
    loop {
        let suffix = format!(" ({})", counts[&key]);
        let result = if let Some(i) = dot {
            format!("{}{}{}", &name[..i], suffix, &name[i..])
        } else {
            format!("{name}{suffix}")
        };
        let count = counts.entry(result.to_lowercase()).or_default();
        *count += 1;
        if *count == 1 {
            return result;
        }
        *counts.get_mut(&key).unwrap() += 1;
    }
}
pub fn archive_size_estimate(entries: &ArchiveEntries) -> i64 {
    entries.list.iter().fold(ARCHIVE_ENTRY_OVERHEAD, |sum, e| {
        sum.wrapping_add(e.size)
            .wrapping_add(ARCHIVE_ENTRY_OVERHEAD)
    })
}
pub fn single_folder_path(urls: &[reqwest::Url]) -> Option<PathBuf> {
    if urls.len() != 1 {
        return None;
    }
    let path = urls[0].to_file_path().ok()?;
    path.is_dir().then_some(path)
}
pub fn folder_files_for_sending(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    fn visible(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
        let mut entries = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|s| s.to_string_lossy().starts_with('.'))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        Ok(entries)
    }
    let entries = visible(folder)?;
    let mut result = entries
        .iter()
        .filter(|p| p.is_file())
        .cloned()
        .collect::<Vec<_>>();
    for directory in entries.iter().filter(|p| p.is_dir()) {
        result.extend(visible(directory)?.into_iter().filter(|p| p.is_file()));
    }
    Ok(result)
}
/// Local civil time is an FFI boundary; no process TZ mutation.
#[cfg(unix)]
#[allow(unsafe_code)]
fn local_date(time: SystemTime) -> Option<[i32; 6]> {
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).ok()?,
        Err(e) => i64::try_from(e.duration().as_secs())
            .ok()?
            .checked_add(i64::from(e.duration().subsec_nanos() != 0))?
            .checked_neg()?,
    };
    let seconds = libc::time_t::try_from(seconds).ok()?;
    let mut out = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: localtime_r writes only the supplied tm and reads a valid time_t.
    if unsafe { libc::localtime_r(&seconds, out.as_mut_ptr()) }.is_null() {
        return None;
    }
    // SAFETY: successful localtime_r initialized tm.
    let out = unsafe { out.assume_init() };
    Some([
        out.tm_sec,
        out.tm_min,
        out.tm_hour,
        out.tm_mday,
        out.tm_mon,
        (out.tm_year + 1900).max(1980),
    ])
}
#[cfg(not(unix))]
fn local_date(_time: SystemTime) -> Option<[i32; 6]> {
    None
} // Platform date adapter33, no invented timestamp.
fn entry(path: &Path, name: String, directory: bool) -> Option<ArchiveEntry> {
    let meta = fs::metadata(path).ok()?;
    if !directory {
        File::open(path).ok()?;
    }
    Some(ArchiveEntry {
        absolute: path.to_owned(),
        name,
        modified: local_date(meta.modified().ok()?)?,
        size: if directory {
            0
        } else {
            i64::try_from(meta.len()).ok()?
        },
        directory,
    })
}
pub fn gather_archive_entries(job: &ArchiveJob) -> Option<ArchiveEntries> {
    let mut result = ArchiveEntries::default();
    if !job.folder.as_os_str().is_empty() {
        fn walk(folder: &Path, path: &Path, root: &str, out: &mut ArchiveEntries) -> Option<()> {
            // QDirIterator treats missing/unreadable directories as no entries.
            let Ok(entries) = fs::read_dir(path) else {
                return Some(());
            };
            for item in entries {
                let path = item.ok()?.path();
                let meta = fs::symlink_metadata(&path).ok()?;
                if meta.is_symlink() {
                    continue;
                }
                let relative = path.strip_prefix(folder).ok()?.to_str()?.replace('\\', "/");
                if meta.is_dir() {
                    out.list
                        .push(entry(&path, format!("{root}/{relative}/"), true)?);
                    walk(folder, &path, root, out)?;
                } else if meta.is_file() {
                    let e = entry(&path, format!("{root}/{relative}"), false)?;
                    out.total = out.total.checked_add(e.size)?;
                    out.list.push(e);
                }
            }
            Some(())
        }
        walk(&job.folder, &job.folder, &job.root, &mut result)?;
    } else {
        let mut counts = BTreeMap::new();
        for path in &job.paths {
            if path.is_dir() {
                continue;
            }
            let meta = fs::metadata(path).ok()?;
            if !meta.is_file() {
                continue;
            }
            let absolute = std::path::absolute(path).ok()?;
            let name = deduped_name(&mut counts, path.file_name()?.to_str()?);
            let e = entry(&absolute, name, false)?;
            result.total = result.total.checked_add(e.size)?;
            result.list.push(e);
        }
        if result.list.is_empty() {
            return None;
        }
    }
    Some(result)
}
struct TempArchive {
    path: PathBuf,
    retain: bool,
}
impl Drop for TempArchive {
    fn drop(&mut self) {
        if !self.retain {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn temporary_archive(directory: &Path) -> Option<TempArchive> {
    fs::create_dir_all(directory).ok()?;
    for _ in 0..128 {
        let mut random = [0; 16];
        tg_base::random::random_fill(&mut random);
        let name = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = directory.join(format!("{name}.zip"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(_) => {
                return Some(TempArchive {
                    path,
                    retain: false,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            _ => return None,
        }
    }
    None
}
/// Explicit directory permits isolated tests. The real owner's default is
/// std::env::temp_dir()/tdarchive, exactly ArchiveDirectory in C++.
pub fn write_archive(
    entries: ArchiveEntries,
    limit: i64,
    directory: &Path,
    mut progress: impl FnMut(f64) -> bool,
) -> ArchiveWriteResult {
    let failed = |status, size| ArchiveWriteResult {
        status,
        path: PathBuf::new(),
        size,
    };
    let Some(mut temp) = temporary_archive(directory) else {
        return failed(Status::Failed, 0);
    };
    let Some(mut zip) = Archive::open(&temp.path) else {
        return failed(Status::Failed, 0);
    };
    let mut processed = 0i64;
    let mut status = Status::Done;
    let mut buffer = vec![0; ARCHIVE_CHUNK_SIZE];
    let step = |processed: i64, progress: &mut dyn FnMut(f64) -> bool| {
        progress(if entries.total > 0 {
            (processed as f64 / entries.total as f64).clamp(0., 1.)
        } else {
            0.
        })
    };
    for e in &entries.list {
        if !step(processed, &mut progress) {
            status = Status::Cancelled;
            break;
        }
        if !e.directory && !e.absolute.exists() {
            processed = processed.wrapping_add(e.size);
            continue;
        }
        if !zip.open_entry(
            &e.name,
            e.modified,
            e.directory,
            e.directory || already_compressed_name(&e.name),
            e.size,
        ) {
            status = Status::Failed;
            break;
        }
        if !e.directory {
            let Ok(mut file) = File::open(&e.absolute) else {
                status = Status::Failed;
                break;
            };
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if !zip.write(&buffer[..n]) {
                            status = Status::Failed;
                            break;
                        }
                        processed = processed.wrapping_add(n as i64);
                        if !step(processed, &mut progress) {
                            status = Status::Cancelled;
                            break;
                        }
                        if zip.size() > limit {
                            status = Status::TooLarge;
                            break;
                        }
                    }
                    Err(_) => {
                        status = Status::Failed;
                        break;
                    }
                }
            }
            if status != Status::Done {
                break;
            }
        }
        if !zip.close_entry() {
            status = Status::Failed;
            break;
        }
    }
    // Final ZIP central directory can exceed limit: Source deliberately checks
    // only after data chunks, not after entry/central-directory close.
    let closed = zip.finish();
    if !closed && status == Status::Done {
        status = Status::Failed;
    }
    let size = fs::metadata(&temp.path).map_or(0, |m| m.len() as i64);
    if status != Status::Done {
        return failed(status, size);
    }
    temp.retain = true;
    ArchiveWriteResult {
        status,
        path: temp.path.clone(),
        size,
    }
}
/// Worker phase of ClearStaleArchiveFiles; caller schedules on tg-runtime Queue.
pub fn clear_stale_archive_files(directory: &Path, now: SystemTime) -> std::io::Result<usize> {
    let stale = now
        .checked_sub(Duration::from_secs(STALE_ARCHIVE_TIMEOUT))
        .unwrap_or(UNIX_EPOCH);
    let mut removed = 0;
    if !directory.exists() {
        return Ok(0);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.path().is_file() {
            continue;
        }
        if entry.metadata()?.modified()? < stale && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}
