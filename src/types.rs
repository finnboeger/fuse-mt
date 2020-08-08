use fuse_mt::FileAttr;
use fuse::FileType;
use time::Timespec;
use serde::{ Deserialize, Serialize };

use std::convert::{ From, Into };

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
        unimplemented!()
    }
}

impl Into<FileAttr> for SerializableFileAttr {
    fn into(self) -> FileAttr {
        unimplemented!()
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
        unimplemented!()
    }
}

impl Into<FileType> for SerializableFileType {
    fn into(self) -> FileType {
        unimplemented!()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub struct SerializableTimespec { pub sec: i64, pub nsec: i32 }

impl From<Timespec> for SerializableTimespec {
    fn from(timespec: Timespec) -> Self {
        unimplemented!()
    }
}

impl Into<Timespec> for SerializableTimespec {
    fn into(self) -> Timespec {
        unimplemented!()
    }
}