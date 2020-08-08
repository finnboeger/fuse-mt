use fuse_mt::{ FileAttr, Statfs };
use fuse::FileType;
use time::Timespec;

pub(crate) fn mode_to_filetype(mode: libc::mode_t) -> FileType {
    match mode & libc::S_IFMT {
        libc::S_IFDIR => FileType::Directory,
        libc::S_IFREG => FileType::RegularFile,
        libc::S_IFLNK => FileType::Symlink,
        libc::S_IFBLK => FileType::BlockDevice,
        libc::S_IFCHR => FileType::CharDevice,
        libc::S_IFIFO => FileType::NamedPipe,
        libc::S_IFSOCK => FileType::Socket,
        _ => {
            panic!("unknown file type");
        }
    }
}

pub(crate) fn stat_to_fuse(stat: libc::stat64) -> FileAttr {
    // st_mode encodes both the kind and the permissions
    let kind = mode_to_filetype(stat.st_mode);
    let perm = (stat.st_mode & 0o7777) as u16;

    FileAttr {
        size: stat.st_size as u64,
        blocks: stat.st_blocks as u64,
        atime: Timespec {
            sec: stat.st_atime as i64,
            nsec: stat.st_atime_nsec as i32,
        },
        mtime: Timespec {
            sec: stat.st_mtime as i64,
            nsec: stat.st_mtime_nsec as i32,
        },
        ctime: Timespec {
            sec: stat.st_ctime as i64,
            nsec: stat.st_ctime_nsec as i32,
        },
        crtime: Timespec { sec: 0, nsec: 0 },
        kind,
        perm,
        nlink: stat.st_nlink as u32,
        uid: stat.st_uid,
        gid: stat.st_gid,
        rdev: stat.st_rdev as u32,
        flags: 0,
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "linux")]
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
