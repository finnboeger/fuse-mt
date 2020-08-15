#[cfg(any(target_os = "macos", target_os = "freebsd"))]
use crate::libc_extras::libc;
use crate::types::{SerializableFileAttr, SerializableFileType, SerializableTimespec};
#[cfg(feature = "mount")]
use fuse::FileType;
#[cfg(feature = "mount")]
use fuse_mt::{FileAttr, Statfs};

pub(crate) fn mode_to_filetype_serializable(mode: libc::mode_t) -> SerializableFileType {
    match mode & libc::S_IFMT {
        libc::S_IFDIR => SerializableFileType::Directory,
        libc::S_IFREG => SerializableFileType::RegularFile,
        libc::S_IFLNK => SerializableFileType::Symlink,
        libc::S_IFBLK => SerializableFileType::BlockDevice,
        libc::S_IFCHR => SerializableFileType::CharDevice,
        libc::S_IFIFO => SerializableFileType::NamedPipe,
        libc::S_IFSOCK => SerializableFileType::Socket,
        _ => {
            panic!("unknown file type");
        }
    }
}

#[cfg(feature = "mount")]
pub(crate) fn mode_to_filetype(mode: libc::mode_t) -> FileType {
    mode_to_filetype_serializable(mode).into()
}

pub(crate) fn stat_to_fuse_serializable(stat: libc::stat64) -> SerializableFileAttr {
    // st_mode encodes both the kind and the permissions
    let kind = mode_to_filetype_serializable(stat.st_mode);
    let perm = (stat.st_mode & 0o7777) as u16;

    SerializableFileAttr {
        size: stat.st_size as u64,
        blocks: stat.st_blocks as u64,
        atime: SerializableTimespec {
            sec: stat.st_atime as i64,
            nsec: stat.st_atime_nsec as i32,
        },
        mtime: SerializableTimespec {
            sec: stat.st_mtime as i64,
            nsec: stat.st_mtime_nsec as i32,
        },
        ctime: SerializableTimespec {
            sec: stat.st_ctime as i64,
            nsec: stat.st_ctime_nsec as i32,
        },
        crtime: SerializableTimespec { sec: 0, nsec: 0 },
        kind,
        perm,
        nlink: stat.st_nlink as u32,
        uid: stat.st_uid,
        gid: stat.st_gid,
        rdev: stat.st_rdev as u32,
        flags: 0,
    }
}

#[cfg(feature = "mount")]
pub(crate) fn stat_to_fuse(stat: libc::stat64) -> FileAttr {
    stat_to_fuse_serializable(stat).into()
}

#[cfg(all(any(target_os = "macos", target_os = "freebsd"), feature = "mount"))]
pub(crate) fn statfs_to_fuse(statfs: libc::statfs) -> Statfs {
    Statfs {
        blocks: statfs.f_blocks,
        bfree: statfs.f_bfree,
        bavail: statfs.f_bavail,
        files: statfs.f_files,
        ffree: statfs.f_ffree,
        bsize: statfs.f_bsize as u32,
        namelen: 0, // TODO
        frsize: 0,  // TODO
    }
}

#[cfg(all(target_os = "linux", feature = "mount"))]
pub(crate) fn statfs_to_fuse(statfs: libc::statfs) -> Statfs {
    Statfs {
        blocks: statfs.f_blocks as u64,
        bfree: statfs.f_bfree as u64,
        bavail: statfs.f_bavail as u64,
        files: statfs.f_files as u64,
        ffree: statfs.f_ffree as u64,
        bsize: statfs.f_bsize as u32,
        namelen: statfs.f_namelen as u32,
        frsize: statfs.f_frsize as u32,
    }
}
