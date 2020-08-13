use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::io::Cursor;
use std::path::Path;
use std::ffi::OsString;

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
        // 0 = stdin, 1 = stdout, 2 = stderr
        let mut key: u64 = 3;

        while self.open.contains_key(&key) {
            key += 1;
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

    pub fn find(&self, handle: u64) -> Result<&Descriptor> {
        match self.open.get(&handle) {
            None => Err(anyhow!("Handle not found")),
            Some(d) => Ok(d),
        }
    }

    pub fn find_mut(&mut self, handle: u64) -> Result<&mut Descriptor, &str> {
        match self.open.get_mut(&handle) {
            None => Err("Handle not found"),
            Some(d) => Ok(d),
        }
    }
}

// TODO: figure out how read operates on this level and design a structure that works to read the cached .txt files
pub enum Descriptor {
    Path(PathBuf),
    Handle(u64),
    File {
        path: OsString,
        cursor: Cursor<Vec<u8>>,
    },
}

impl Descriptor {
    pub fn new<I: Into<PathBuf>>(path: I) -> Self {
        Self::Path(path.into())
    }
}
