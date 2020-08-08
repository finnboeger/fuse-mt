use fuse::FileType;
use fuse_mt::FileAttr;
use serde::{Deserialize, Serialize};
use time::Timespec;

use std::convert::{From, Into};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SerializableFileAttr {
    /// Size in bytes
    pub size: u64,
    /// Size in blocks
    pub blocks: u64,
    /// Time of last access
    pub atime: SerializableTimespec,
    /// Time of last modification
    pub mtime: SerializableTimespec,
    /// Time of last metadata change
    pub ctime: SerializableTimespec,
    /// Time of creation (macOS only)
    pub crtime: SerializableTimespec,
    /// Kind of file (directory, file, pipe, etc.)
    pub kind: SerializableFileType,
    /// Permissions
    pub perm: u16,
    /// Number of hard links
    pub nlink: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Device ID (if special file)
    pub rdev: u32,
    /// Flags (macOS only; see chflags(2))
    pub flags: u32,
}

impl From<FileAttr> for SerializableFileAttr {
    fn from(attr: FileAttr) -> Self {
        Self {
            size: attr.size,
            blocks: attr.blocks,
            atime: SerializableTimespec::from(attr.atime),
            mtime: SerializableTimespec::from(attr.mtime),
            ctime: SerializableTimespec::from(attr.ctime),
            crtime: SerializableTimespec::from(attr.crtime),
            kind: SerializableFileType::from(attr.kind),
            perm: attr.perm,
            nlink: attr.nlink,
            uid: attr.uid,
            gid: attr.gid,
            rdev: attr.rdev,
            flags: attr.flags,
        }
    }
}

impl Into<FileAttr> for SerializableFileAttr {
    fn into(self) -> FileAttr {
        FileAttr {
            size: self.size,
            blocks: self.blocks,
            atime: self.atime.into(),
            mtime: self.mtime.into(),
            ctime: self.ctime.into(),
            crtime: self.crtime.into(),
            kind: self.kind.into(),
            perm: self.perm,
            nlink: self.nlink,
            uid: self.uid,
            gid: self.gid,
            rdev: self.rdev,
            flags: self.flags,
        }
    }
}

/// File types
#[derive(Clone, Copy, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub enum SerializableFileType {
    /// Named pipe (S_IFIFO)
    NamedPipe,
    /// Character device (S_IFCHR)
    CharDevice,
    /// Block device (S_IFBLK)
    BlockDevice,
    /// Directory (S_IFDIR)
    Directory,
    /// Regular file (S_IFREG)
    RegularFile,
    /// Symbolic link (S_IFLNK)
    Symlink,
    /// Unix domain socket (S_IFSOCK)
    Socket,
}

impl From<FileType> for SerializableFileType {
    fn from(file_type: FileType) -> Self {
        match file_type {
            FileType::NamedPipe => Self::NamedPipe,
            FileType::CharDevice => Self::CharDevice,
            FileType::BlockDevice => Self::BlockDevice,
            FileType::Directory => Self::Directory,
            FileType::RegularFile => Self::RegularFile,
            FileType::Symlink => Self::Symlink,
            FileType::Socket => Self::Socket,
        }
    }
}

impl Into<FileType> for SerializableFileType {
    fn into(self) -> FileType {
        match self {
            Self::NamedPipe => FileType::NamedPipe,
            Self::CharDevice => FileType::CharDevice,
            Self::BlockDevice => FileType::BlockDevice,
            Self::Directory => FileType::Directory,
            Self::RegularFile => FileType::RegularFile,
            Self::Symlink => FileType::Symlink,
            Self::Socket => FileType::Socket,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub struct SerializableTimespec {
    pub sec: i64,
    pub nsec: i32,
}

impl From<Timespec> for SerializableTimespec {
    fn from(timespec: Timespec) -> Self {
        Self {
            sec: timespec.sec,
            nsec: timespec.nsec,
        }
    }
}

impl Into<Timespec> for SerializableTimespec {
    fn into(self) -> Timespec {
        Timespec {
            sec: self.sec,
            nsec: self.nsec,
        }
    }
}
