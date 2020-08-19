use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Cursor, Error as IoError};
use std::path::PathBuf;
use std::thread::spawn;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{channel, Receiver},
};

static FH_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct FileHandles {
    open: HashMap<u64, Descriptor>,
}

impl FileHandles {
    pub fn new() -> Self {
        Self {
            open: HashMap::new(),
        }
    }

    fn find_first_available(&self) -> u64 {
        let mut key: u64 = FH_COUNTER.fetch_add(1, Ordering::SeqCst);

        while self.open.contains_key(&key) {
            key = FH_COUNTER.fetch_add(1, Ordering::SeqCst);
        }
        key
    }

    pub fn register_handle(&mut self, descriptor: Descriptor) -> u64 {
        let key = self.find_first_available();
        self.open.insert(key, descriptor);
        key
    }

    pub fn free_handle(&mut self, handle: u64) -> Result<Descriptor> {
        match self.open.remove(&handle) {
            None => Err(anyhow!("Handle not found")),
            Some(d) => Ok(d),
        }
    }

    pub fn find(&mut self, handle: u64) -> Result<&mut Descriptor> {
        match self.open.get_mut(&handle) {
            None => Err(anyhow!("Handle not found")),
            Some(d) => match d.resolve() {
                Ok(d) => Ok(d),
                Err(err) => Err(err).context("Handle failed to open"),
            }
        }
    }
}

// TODO: figure out how read operates on this level and design a structure that works to read the cached .txt files
pub enum Descriptor {
    Path(PathBuf),
    Handle(u64),
    Lazy(Receiver<Result<u64, i32>>),
    // Placeholder, so we can still release them properly later
    Error(i32),
    File {
        path: OsString,
        cursor: Cursor<Vec<u8>>,
    },
}

impl Descriptor {
    pub fn new<I: Into<PathBuf>>(path: I) -> Self {
        Self::Path(path.into())
    }

    pub fn lazy<I: Into<PathBuf>>(path: I, flags: u32) -> Self {
        let (tx, rx) = channel();
        let owned = path.into();
        spawn(move || {
            use crate::libc_wrappers;

            let path = owned.clone();
            tx.send(match libc_wrappers::open(owned.into_os_string(), flags as libc::c_int) {
                Ok(fh) => Ok(
                    fh,
                ),
                Err(e) => {
                    let err = IoError::from_raw_os_error(e);
                    error!("open({:?}): {}", path.display(), err);
                    Err(e)
                }
            }).unwrap();
        });
        Descriptor::Lazy(rx)
    }

    pub fn resolve(&mut self) -> Result<&mut Self, IoError> {
        match self {
            &mut Descriptor::Lazy(ref mut rx) => {
                match rx.recv().expect("Lazy open thread locked up") {
                    Ok(handle) => {
                        *self = Descriptor::Handle(handle);
                        Ok(self)
                    },
                    Err(x) => {
                        *self = Descriptor::Error(x);
                        Err(IoError::from_raw_os_error(x))
                    },
                }
            },
            &mut Descriptor::Error(x) => Err(IoError::from_raw_os_error(x)),
            x => Ok(x)
        }
    }
}