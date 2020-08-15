// PassthroughFS :: A filesystem that passes all calls through to another underlying filesystem.
//
// Implemented using fuse_mt::FilesystemMT.
//
// Copyright (c) 2016-2020 by William R. Fraser
//
use anyhow::{Context, Result};

use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::path::{Path, PathBuf};

use crate::libc_extras::libc;
use crate::libc_wrappers;

use crate::cache::{load_from_zip, Entry};
use crate::file_handles::*;
use crate::stat::*;
use crate::utils::*;
use fuse_mt::*;
use std::sync::Mutex;
use time::*;
use zip::ZipArchive;

pub struct PassthroughFS {
    source: OsString,
    struct_cache: Entry,
    files_cache: Mutex<ZipArchive<File>>,
    file_handles: Mutex<FileHandles>,
}

impl PassthroughFS {
    #[allow(unused_variables)]
    pub fn new<P: AsRef<Path>>(source: OsString, target: OsString, cache_path: P, coverdb: Option<PathBuf>) -> Result<Self> {
        let cache_path = cache_path.as_ref();
        let file = File::open(cache_path).with_context(|| format!("Failed to open cache zip at '{}'", cache_path.display()))?;
        let mut zip = zip::ZipArchive::new(file).context("Failed to parse cache file as zip")?;
        let struct_cache = load_from_zip(&mut zip).context("Unable to load cache")?;
        
        #[cfg(feature = "cover")]
        if let Some(dest) = coverdb {
            // don't fail if the cache was created without a coverdb
            if let Ok(mut coverdb) = zip.by_name("cover.db") {
                let mut src = tempfile::NamedTempFile::new().context("Failed to create temporary file for the src coverdb")?;
                io::copy(&mut coverdb, &mut src).context("Failed to extract cache coverdb")?;
                src.flush()?;
                crate::coverdb::import(&src, &dest, &target).context("Failed to import coverdb")?;
            }
        }
        
        Ok(Self {
            source,
            struct_cache,
            files_cache: Mutex::new(zip),
            file_handles: Mutex::new(FileHandles::new()),
        })
    }

    fn real_path(&self, partial: &Path) -> OsString {
        PathBuf::from(&self.source)
            .join(path_to_rel(partial))
            .into_os_string()
    }

    fn stat_real(&self, path: &Path) -> io::Result<FileAttr> {
        match self.struct_cache.find(path) {
            Ok(Entry::Dict {
                   name: _,
                   contents: _,
                   stat,
               }) => Ok((*stat).into()),
            Ok(Entry::File { name: _, stat }) => Ok((*stat).into()),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "entry not found in cache",
            )),
        }
    }
}

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };

// TODO: for all operations that change the file structure (e.g. delete, create, rename, chmod, ..)
//       and for write operations on cached files return ENOSYS?
impl FilesystemMT for PassthroughFS {
    fn init(&self, _req: RequestInfo) -> ResultEmpty {
        debug!("init");
        Ok(())
    }

    fn destroy(&self, _req: RequestInfo) {
        debug!("destroy");
    }

    fn getattr(&self, _req: RequestInfo, path: &Path, fh: Option<u64>) -> ResultEntry {
        debug!("getattr: {:?}", path);

        if let Some(fh) = fh {
            match self.file_handles.lock().unwrap().find(fh) {
                Ok(d) => match d {
                    Descriptor::Path(_) => match self.stat_real(path) {
                        Ok(attr) => Ok((TTL, attr)),
                        Err(_) => Err(libc::ENOENT),
                    },
                    Descriptor::Handle(h) => match libc_wrappers::fstat(*h) {
                        Ok(stat) => Ok((TTL, stat_to_fuse(stat))),
                        Err(e) => Err(e),
                    },
                    Descriptor::File { path: _, cursor: _ } => match self.stat_real(path) {
                        Ok(attr) => Ok((TTL, attr)),
                        Err(_) => Err(libc::ENOENT),
                    },
                },
                Err(_) => Err(libc::ENOENT),
            }
        } else {
            match self.stat_real(path) {
                Ok(attr) => Ok((TTL, attr)),
                Err(_) => Err(libc::ENOENT),
            }
        }
    }

    #[allow(unused_variables)]
    fn chmod(&self, _req: RequestInfo, path: &Path, fh: Option<u64>, mode: u32) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn chown(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: Option<u64>,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    fn truncate(&self, _req: RequestInfo, path: &Path, fh: Option<u64>, size: u64) -> ResultEmpty {
        debug!("truncate: {:?} to {:#x}", path, size);

        let result = if let Some(fd) = fh {
            match self.file_handles.lock().unwrap().find(fd) {
                Ok(Descriptor::Handle(h)) => unsafe {
                    libc::ftruncate64(*h as libc::c_int, size as i64)
                },
                // TODO: maybe EROFS? How will other files be handled if we return that?
                Ok(Descriptor::Path(_)) => return Err(libc::EACCES),
                Err(_) => return Err(libc::ENOENT),
                Ok(Descriptor::File { path: _, cursor: _ }) => return Err(libc::EACCES),
            }
        } else {
            let mut zip = self.files_cache.lock().unwrap();
            let result = match path_to_rel(path).to_str().map(|x| zip.by_name(x)).transpose() {
                Err(_) | Ok(None) => {
                    let real = self.real_path(path);
                    unsafe {
                        let path_c = CString::from_vec_unchecked(real.into_vec());
                        libc::truncate64(path_c.as_ptr(), size as i64)
                    }
                }
                Ok(_) => return Err(libc::EACCES),
            };
            result
        };

        if -1 == result {
            let e = io::Error::last_os_error();
            error!("truncate({:?}, {}): {}", path, size, e);
            Err(e.raw_os_error().unwrap())
        } else {
            Ok(())
        }
    }

    #[allow(unused_variables)]
    fn utimens(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: Option<u64>,
        atime: Option<Timespec>,
        mtime: Option<Timespec>,
    ) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    fn readlink(&self, _req: RequestInfo, path: &Path) -> ResultData {
        debug!("readlink: {:?}", path);

        let real = self.real_path(path);
        match ::std::fs::read_link(real) {
            Ok(target) => Ok(target.into_os_string().into_vec()),
            Err(e) => Err(e.raw_os_error().unwrap()),
        }
    }

    #[allow(unused_variables)]
    fn mknod(
        &self,
        _req: RequestInfo,
        parent_path: &Path,
        name: &OsStr,
        mode: u32,
        rdev: u32,
    ) -> ResultEntry {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn mkdir(&self, _req: RequestInfo, parent_path: &Path, name: &OsStr, mode: u32) -> ResultEntry {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn unlink(&self, _req: RequestInfo, parent_path: &Path, name: &OsStr) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn rmdir(&self, _req: RequestInfo, parent_path: &Path, name: &OsStr) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn symlink(
        &self,
        _req: RequestInfo,
        parent_path: &Path,
        name: &OsStr,
        target: &Path,
    ) -> ResultEntry {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn rename(
        &self,
        _req: RequestInfo,
        parent_path: &Path,
        name: &OsStr,
        newparent_path: &Path,
        newname: &OsStr,
    ) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn link(
        &self,
        _req: RequestInfo,
        path: &Path,
        newparent: &Path,
        newname: &OsStr,
    ) -> ResultEntry {
        Err(libc::ENOSYS)
    }

    fn open(&self, _req: RequestInfo, path: &Path, flags: u32) -> ResultOpen {
        debug!("open: {:?} flags={:#x}", path, flags);
        let mut zip = self.files_cache.lock().unwrap();
        let result = match path_to_rel(path).to_str().map(|x| zip.by_name(x)).transpose() {
            Err(_) | Ok(None) => {
                let real = self.real_path(path);
                match libc_wrappers::open(real, flags as libc::c_int) {
                    Ok(fh) => Ok((
                        self.file_handles
                            .lock()
                            .unwrap()
                            .register_handle(Descriptor::Handle(fh)),
                        flags,
                    )),
                    Err(e) => {
                        error!("open({:?}): {}", path, io::Error::from_raw_os_error(e));
                        Err(e)
                    }
                }
            },
            Ok(Some(mut file)) => {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).expect("Zip cache was forcefully closed?");
                Ok((
                    self.file_handles
                        .lock()
                        .unwrap()
                        .register_handle(Descriptor::File {
                            path: path.to_path_buf().into_os_string(),
                            cursor: Cursor::new(buf),
                        }),
                    flags,
                ))
            },
        };
        result
    }

    fn read(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: u64,
        offset: u64,
        size: u32,
        callback: impl FnOnce(ResultSlice<'_>) -> CallbackResult,
    ) -> CallbackResult {
        debug!("read: {:?} {:#x} @ {:#x}", path, size, offset);

        // TODO: remove code duplication
        match self.file_handles.lock().unwrap().find_mut(fh) {
            Ok(d) => match d {
                Descriptor::Path(_) => return callback(Err(libc::EISDIR)),
                Descriptor::Handle(handle) => {
                    let mut file = unsafe { UnmanagedFile::new(*handle) };
                    let mut data = Vec::<u8>::with_capacity(size as usize);
                    unsafe { data.set_len(size as usize) };

                    if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                        error!("seek({:?}, {}): {}", path, offset, e);
                        return callback(Err(e.raw_os_error().unwrap()));
                    }
                    match file.read(&mut data) {
                        Ok(n) => {
                            data.truncate(n);
                        }
                        Err(e) => {
                            error!("read {:?}, {:#x} @ {:#x}: {}", path, size, offset, e);
                            return callback(Err(e.raw_os_error().unwrap()));
                        }
                    }

                    callback(Ok(&data))
                }
                Descriptor::File { path: _, cursor } => {
                    let mut data = Vec::<u8>::with_capacity(size as usize);
                    unsafe { data.set_len(size as usize) };

                    if let Err(e) = cursor.seek(SeekFrom::Start(offset)) {
                        error!("seek({:?}, {}): {}", path, offset, e);
                        return callback(Err(e.raw_os_error().unwrap()));
                    }
                    match cursor.read(&mut data) {
                        Ok(n) => {
                            data.truncate(n);
                        }
                        Err(e) => {
                            error!("read {:?}, {:#x} @ {:#x}: {}", path, size, offset, e);
                            return callback(Err(e.raw_os_error().unwrap()));
                        }
                    }

                    callback(Ok(&data))
                }
            },
            Err(_) => callback(Err(libc::EBADF)),
        }
    }

    fn write(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: u64,
        offset: u64,
        data: Vec<u8>,
        _flags: u32,
    ) -> ResultWrite {
        let handle = match self.file_handles.lock().unwrap().find(fh) {
            Ok(Descriptor::Handle(h)) => *h,
            _ => return Err(libc::EACCES),
        };
        debug!("write: {:?} {:#x} @ {:#x}", path, data.len(), offset);
        let mut file = unsafe { UnmanagedFile::new(handle) };

        if let Err(e) = file.seek(SeekFrom::Start(offset)) {
            error!("seek({:?}, {}): {}", path, offset, e);
            return Err(e.raw_os_error().unwrap());
        }
        let nwritten: u32 = match file.write(&data) {
            Ok(n) => n as u32,
            Err(e) => {
                error!("write {:?}, {:#x} @ {:#x}: {}", path, data.len(), offset, e);
                return Err(e.raw_os_error().unwrap());
            }
        };

        Ok(nwritten)
    }

    fn flush(&self, _req: RequestInfo, path: &Path, fh: u64, _lock_owner: u64) -> ResultEmpty {
        debug!("flush: {:?}", path);

        let handle = match self.file_handles.lock().unwrap().find(fh) {
            Ok(Descriptor::Handle(h)) => *h,
            _ => return Ok(()),
        };

        let mut file = unsafe { UnmanagedFile::new(handle) };

        if let Err(e) = file.flush() {
            error!("flush({:?}): {}", path, e);
            return Err(e.raw_os_error().unwrap());
        }

        Ok(())
    }

    // TODO: should fail if called on a dir
    fn release(
        &self,
        _req: RequestInfo,
        path: &Path,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> ResultEmpty {
        debug!("release: {:?}", path);
        match self.file_handles.lock().unwrap().free_handle(fh) {
            Ok(Descriptor::File { path: _, cursor: _ }) => Ok(()),
            Ok(Descriptor::Handle(handle)) => libc_wrappers::close(handle),
            Ok(Descriptor::Path(_)) => Ok(()),
            Err(_) => Err(libc::EBADF),
        }
    }

    fn fsync(&self, _req: RequestInfo, path: &Path, fh: u64, datasync: bool) -> ResultEmpty {
        debug!("fsync: {:?}, data={:?}", path, datasync);

        let handle = match self.file_handles.lock().unwrap().find(fh) {
            Ok(Descriptor::Handle(h)) => *h,
            _ => return Err(libc::EACCES),
        };

        let file = unsafe { UnmanagedFile::new(handle) };

        if let Err(e) = if datasync {
            file.sync_data()
        } else {
            file.sync_all()
        } {
            error!("fsync({:?}, {:?}): {}", path, datasync, e);
            return Err(e.raw_os_error().unwrap());
        }

        Ok(())
    }

    fn opendir(&self, _req: RequestInfo, path: &Path, _flags: u32) -> ResultOpen {
        debug!("opendir: {:?} (flags = {:#o})", path, _flags);
        match self.struct_cache.find(path) {
            Ok(_) => Ok((
                self.file_handles
                    .lock()
                    .unwrap()
                    .register_handle(Descriptor::new(path)),
                0,
            )),
            Err(e) => {
                error!("opendir({:?}): {}", path, e);
                Err(libc::ENOENT)
            }
        }
    }

    fn readdir(&self, _req: RequestInfo, path: &Path, fh: u64) -> ResultReaddir {
        debug!("readdir: {:?}", path);
        let mut entries: Vec<DirectoryEntry> = vec![];

        if fh == 0 {
            error!("readdir: missing fh");
            return Err(libc::EINVAL);
        }

        match self.file_handles.lock().unwrap().find(fh).unwrap() {
            Descriptor::Path(s) => {
                assert_eq!(path, Path::new(&s));
                match self.struct_cache.find(path) {
                    Ok(e) => match e {
                        Entry::Dict {
                            name: _,
                            contents,
                            stat: _,
                        } => {
                            for entry in contents {
                                match entry {
                                    Entry::Dict {
                                        name,
                                        contents: _,
                                        stat,
                                    } => entries.push(DirectoryEntry {
                                        name: OsString::from(name),
                                        kind: stat.kind.into(),
                                    }),
                                    Entry::File { name, stat } => entries.push(DirectoryEntry {
                                        name: OsString::from(name),
                                        kind: stat.kind.into(),
                                    }),
                                }
                            }
                            Ok(entries)
                        }
                        Entry::File { name: _, stat: _ } => Err(libc::ENOTDIR),
                    },
                    Err(_) => Err(libc::ENOENT),
                }
            }
            Descriptor::Handle(handle) => {
                loop {
                    match libc_wrappers::readdir(*handle) {
                        Ok(Some(entry)) => {
                            let name_c = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) };
                            let name = OsStr::from_bytes(name_c.to_bytes()).to_owned();

                            let filetype = match entry.d_type {
                                libc::DT_DIR => FileType::Directory,
                                libc::DT_REG => FileType::RegularFile,
                                libc::DT_LNK => FileType::Symlink,
                                libc::DT_BLK => FileType::BlockDevice,
                                libc::DT_CHR => FileType::CharDevice,
                                libc::DT_FIFO => FileType::NamedPipe,
                                libc::DT_SOCK => {
                                    warn!("FUSE doesn't support Socket file type; translating to NamedPipe instead.");
                                    FileType::NamedPipe
                                }
                                _ => {
                                    let entry_path = PathBuf::from(path).join(&name);
                                    let real_path = self.real_path(&entry_path);
                                    match libc_wrappers::lstat(real_path) {
                                        Ok(stat64) => mode_to_filetype(stat64.st_mode),
                                        Err(errno) => {
                                            let ioerr = io::Error::from_raw_os_error(errno);
                                            panic!("lstat failed after readdir_r gave no file type for {:?}: {}",
                                                   entry_path, ioerr);
                                        }
                                    }
                                }
                            };

                            entries.push(DirectoryEntry {
                                name,
                                kind: filetype,
                            })
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(e) => {
                            error!("readdir: {:?}: {}", path, e);
                            return Err(e);
                        }
                    }
                }

                Ok(entries)
            }
            Descriptor::File { path: _, cursor: _ } => Err(libc::ENOTDIR),
        }
    }

    // TODO: should fail if called on a non-dir
    fn releasedir(&self, _req: RequestInfo, path: &Path, fh: u64, _flags: u32) -> ResultEmpty {
        debug!("releasedir: {:?}", path);
        match self.file_handles.lock().unwrap().free_handle(fh) {
            Ok(Descriptor::Path(_)) => Ok(()),
            Ok(Descriptor::Handle(handle)) => libc_wrappers::closedir(handle),
            Ok(Descriptor::File { path: _, cursor: _ }) => Ok(()),
            Err(_) => Err(libc::EBADF),
        }
    }

    fn fsyncdir(&self, _req: RequestInfo, path: &Path, fh: u64, datasync: bool) -> ResultEmpty {
        debug!("fsyncdir: {:?} (datasync = {:?})", path, datasync);

        let handle = match self.file_handles.lock().unwrap().find(fh) {
            Ok(Descriptor::Handle(h)) => *h,
            _ => return Err(libc::EACCES),
        };

        // TODO: what does datasync mean with regards to a directory handle?
        let result = unsafe { libc::fsync(handle as libc::c_int) };
        if -1 == result {
            let e = io::Error::last_os_error();
            error!("fsyncdir({:?}): {}", path, e);
            Err(e.raw_os_error().unwrap())
        } else {
            Ok(())
        }
    }

    fn statfs(&self, _req: RequestInfo, path: &Path) -> ResultStatfs {
        debug!("statfs: {:?}", path);

        let real = self.real_path(path);
        let mut buf: libc::statfs = unsafe { ::std::mem::zeroed() };
        let result = unsafe {
            let path_c = CString::from_vec_unchecked(real.into_vec());
            libc::statfs(path_c.as_ptr(), &mut buf)
        };

        if -1 == result {
            let e = io::Error::last_os_error();
            error!("statfs({:?}): {}", path, e);
            Err(e.raw_os_error().unwrap())
        } else {
            Ok(statfs_to_fuse(buf))
        }
    }

    #[allow(unused_variables)]
    fn setxattr(
        &self,
        _req: RequestInfo,
        path: &Path,
        name: &OsStr,
        value: &[u8],
        flags: u32,
        position: u32,
    ) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    fn getxattr(&self, _req: RequestInfo, path: &Path, name: &OsStr, size: u32) -> ResultXattr {
        debug!("getxattr: {:?} {:?} {}", path, name, size);

        let real = self.real_path(path);

        if size > 0 {
            let mut data = Vec::<u8>::with_capacity(size as usize);
            unsafe { data.set_len(size as usize) };
            let nread = libc_wrappers::lgetxattr(real, name.to_owned(), data.as_mut_slice())?;
            data.truncate(nread);
            Ok(Xattr::Data(data))
        } else {
            let nbytes = libc_wrappers::lgetxattr(real, name.to_owned(), &mut [])?;
            Ok(Xattr::Size(nbytes as u32))
        }
    }

    fn listxattr(&self, _req: RequestInfo, path: &Path, size: u32) -> ResultXattr {
        debug!("listxattr: {:?}", path);

        let real = self.real_path(path);

        if size > 0 {
            let mut data = Vec::<u8>::with_capacity(size as usize);
            unsafe { data.set_len(size as usize) };
            let nread = libc_wrappers::llistxattr(real, data.as_mut_slice())?;
            data.truncate(nread);
            Ok(Xattr::Data(data))
        } else {
            let nbytes = libc_wrappers::llistxattr(real, &mut [])?;
            Ok(Xattr::Size(nbytes as u32))
        }
    }

    #[allow(unused_variables)]
    fn removexattr(&self, _req: RequestInfo, path: &Path, name: &OsStr) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[allow(unused_variables)]
    fn create(
        &self,
        _req: RequestInfo,
        parent: &Path,
        name: &OsStr,
        mode: u32,
        flags: u32,
    ) -> ResultCreate {
        Err(libc::ENOSYS)
    }

    #[cfg(target_os = "macos")]
    fn setvolname(&self, _req: RequestInfo, name: &OsStr) -> ResultEmpty {
        Err(libc::ENOSYS)
    }

    #[cfg(target_os = "macos")]
    fn getxtimes(&self, _req: RequestInfo, path: &Path) -> ResultXTimes {
        debug!("getxtimes: {:?}", path);
        let xtimes = XTimes {
            bkuptime: Timespec { sec: 0, nsec: 0 },
            crtime: Timespec { sec: 0, nsec: 0 },
        };
        Ok(xtimes)
    }
}

/// A file that is not closed upon leaving scope.
struct UnmanagedFile {
    inner: Option<File>,
}

impl UnmanagedFile {
    unsafe fn new(fd: u64) -> UnmanagedFile {
        UnmanagedFile {
            inner: Some(File::from_raw_fd(fd as i32)),
        }
    }
    fn sync_all(&self) -> io::Result<()> {
        self.inner.as_ref().unwrap().sync_all()
    }
    fn sync_data(&self) -> io::Result<()> {
        self.inner.as_ref().unwrap().sync_data()
    }
}

impl Drop for UnmanagedFile {
    fn drop(&mut self) {
        // Release control of the file descriptor so it is not closed.
        let file = self.inner.take().unwrap();
        file.into_raw_fd();
    }
}

impl Read for UnmanagedFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.as_ref().unwrap().read(buf)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.as_ref().unwrap().read_to_end(buf)
    }
}

impl Write for UnmanagedFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.as_ref().unwrap().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_ref().unwrap().flush()
    }
}

impl Seek for UnmanagedFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.as_ref().unwrap().seek(pos)
    }
}