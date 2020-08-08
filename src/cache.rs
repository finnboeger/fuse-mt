use std::cmp::Ordering;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub enum Entry {
    Dict { name: String, contents: Vec<Entry> },
    File(String),
}

impl Entry {
    pub fn new(path: &Path) -> Self {
        // TODO: Error handling if either path has no file_name or can't be converted to string.
        //       Maybe change Entry to use OsStr?
        let name = String::from(
            path.file_name()
                .ok_or("no file name")
                .unwrap()
                .to_str()
                .ok_or("invalid file name")
                .unwrap(),
        );
        if path.is_dir() {
            Entry::Dict {
                name,
                contents: Vec::new(),
            }
        } else {
            Entry::File(name)
        }
    }

    fn add_entry(&mut self, path: &Path) -> Result<(), &str> {
        //println!("adding {}, is dir: {}", path.display(), path.is_dir());
        match self {
            Entry::File(_) => Err("can't add entry to a file"),
            Entry::Dict { name, contents } => {
                contents.push(Entry::new(path));
                Ok(())
            }
        }
    }

    fn find(&mut self, path: &Path) -> Result<&mut Entry, &str> {
        if path == Path::new("") {
            return Ok(self);
        }
        //println!("path: {}", path.display());
        let mut ancestors: Vec<&Path> = path.ancestors().collect();
        //println!("Ancestors: {:?}", ancestors);
        // Drop last two ancestors which are the root element ('') and '.'
        ancestors.pop();
        ancestors.pop();
        ancestors.reverse();

        let mut item = Ok(self);
        for ancestor in ancestors {
            //println!("Searching for {}, is_dir: {}", ancestor.display(), ancestor.is_dir());
            match item? {
                Entry::File(_) => item = Err("can't search in a file"),
                Entry::Dict { name, contents } => {
                    // We're assuming that all Entries are sorted, therefore we can execute a binary search.
                    item = match contents.binary_search_by(|other: &Entry| -> Ordering {
                        let a = ancestor.file_name().unwrap().to_str().unwrap();
                        let b = match other {
                            Entry::File(s) => s,
                            Entry::Dict{ name, contents } => name,
                        };
                        // TODO: solve File not Found error when it obviously exists
                        //println!("contents: {:#?}", contents);
                        //println!("a: {}, b: {}, cmp: {:?}", a, b, a.cmp(b));
                        b.cmp(&String::from(a))
                    }) {
                        Ok(i) => Ok(&mut contents[i]),
                        Err(_) => Err("File not found"),
                    };
                }
            }
        }
        item
    }
}

//TODO: Error handling
pub fn build(path: &str) {

    // TODO: assert path is a directory
    let working_dir = std::env::current_dir().unwrap();

    // Create root
    let mut root = Entry::Dict {
        name: String::from("."),
        contents: Vec::new(),
    };

    std::env::set_current_dir(path).unwrap();
    let entries = WalkDir::new(".")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .min_depth(1);

    for entry in entries {
        let e = entry.unwrap();
        let p = e.path();
        //println!("full_path: {}", p.display());

        // For a file to be added, the parent has to have been added first so unwrapping should be safe.
        let parent = match p.parent() {
            None => &mut root,
            Some(x) => root.find(x).unwrap(),
        };
        &parent.add_entry(p).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&root).unwrap());

    // Restore original working directory
    std::env::set_current_dir(working_dir).unwrap();
}
