use std::collections::HashMap;
use std::path::Path;

pub struct FileHandles {
    open: HashMap<u64, Descriptor>,
}

// TODO: figure out how read operates on this level and design a structure that works to read the cached .txt files
pub enum Descriptor {
    Path(Box<Path>),
    Handle(u64),
}

impl FileHandles {
    pub fn new() -> Self {
        unimplemented!()
    }

    fn find_first_available(&self) -> u64 {
        unimplemented!()
    }

    pub fn register_handle(&self, descriptor: Descriptor) -> u64 {
        unimplemented!()
    }

    pub fn free_handle(&self, handle: u64) -> Result<(), &str> {
        unimplemented!()
    }

    pub fn find(&self, handle: u64) -> Result<Descriptor, &str> {
        unimplemented!()
    }
}
