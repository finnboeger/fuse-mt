use walkdir::WalkDir;
use std::path::Path;

pub enum Entry {
    Dict { name: String, contents: Vec<Box<Entry>> },
    File(String),
}

impl Entry {
    fn add_entry(&mut self, path: &Path) -> Result<(), &str> {
        match self {
            Entry::File(_) => Err("can't add entry to a file"),
            Entry::Dict{ name, contents } => {
                let name = String::from(path.file_name()?.to_str()?);
                if path.is_dir() {
                    contents.push(Box::new(Entry::Dict {
                        name,
                        contents: Vec::new(),
                    }))
                } else {
                    contents.push(Box::new(Entry::File(name)))
                }
            },
        }
        Ok(())
    }

    fn find(&self, path: &Path) -> Result<&Entry, &str> {
        todo!("implement search");
        Ok(self)
    }
}

//TODO: Error handling
pub fn build(path: &str) {
    let mut root = Entry::Dict {
        name: String::from("."),
        contents: Vec::new(),
    };
    for entry in WalkDir::new(path) {
        match entry.unwrap().path().strip_prefix(path) {
            Ok(p) => {
                root.find(p.parent().unwrap()).unwrap().add_entry(p);
            },
            Err(_) => {}
        }
    }
}
