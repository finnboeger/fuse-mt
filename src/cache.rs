use crate::stat::stat_to_fuse_serializable;
use crate::types::SerializableFileAttr;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::File;
use std::io::{copy, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize)]
pub enum Entry {
    Dict {
        name: String,
        contents: Vec<Entry>,
        stat: SerializableFileAttr,
    },
    File {
        name: String,
        stat: SerializableFileAttr,
    },
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
                stat: stat_to_fuse_serializable(
                    crate::libc_wrappers::lstat(OsString::from(path)).unwrap(),
                ),
            }
        } else {
            let mut stat = stat_to_fuse_serializable(
                crate::libc_wrappers::lstat(OsString::from(path)).unwrap(),
            );
            if name.ends_with(".txt") {
                // remove write permission as files will be read from cache and readonly.
                stat.perm = stat.perm & 0o5555;
            }
            Entry::File { name, stat }
        }
    }

    fn add_entry(&mut self, path: &Path) -> Result<(), &str> {
        match self {
            Entry::File { name, stat } => Err("can't add entry to a file"),
            Entry::Dict {
                name,
                contents,
                stat,
            } => {
                contents.push(Entry::new(path));
                Ok(())
            }
        }
    }

    pub fn find(&self, path: &Path) -> Result<&Entry, &str> {
        if path == Path::new("") {
            return Ok(self);
        }
        let mut ancestors: Vec<&Path> = path.ancestors().collect();
        // Drop last two ancestors which are the root element ('') and '.'
        ancestors.pop();
        ancestors.pop();
        ancestors.reverse();

        let mut item = Ok(self);
        for ancestor in ancestors {
            match item? {
                Entry::File { name, stat } => item = Err("can't search in a file"),
                Entry::Dict {
                    name,
                    contents,
                    stat,
                } => {
                    // We're assuming that all Entries are sorted, therefore we can execute a binary search.
                    item = match contents.binary_search_by(|other: &Entry| -> Ordering {
                        let a = ancestor.file_name().unwrap().to_str().unwrap();
                        let b = match other {
                            Entry::File { name, stat } => name,
                            Entry::Dict {
                                name,
                                contents,
                                stat,
                            } => name,
                        };
                        // TODO: solve File not Found error when it obviously exists
                        b.cmp(&String::from(a))
                    }) {
                        Ok(i) => Ok(&contents[i]),
                        Err(_) => Err("File not found"),
                    };
                }
            }
        }
        item
    }

    fn find_mut(&mut self, path: &Path) -> Result<&mut Entry, &str> {
        if path == Path::new("") {
            return Ok(self);
        }
        let mut ancestors: Vec<&Path> = path.ancestors().collect();
        // Drop last two ancestors which are the root element ('') and '.'
        ancestors.pop();
        ancestors.pop();
        ancestors.reverse();

        let mut item = Ok(self);
        for ancestor in ancestors {
            match item? {
                Entry::File { name, stat } => item = Err("can't search in a file"),
                Entry::Dict {
                    name,
                    contents,
                    stat,
                } => {
                    // We're assuming that all Entries are sorted, therefore we can execute a binary search.
                    item = match contents.binary_search_by(|other: &Entry| -> Ordering {
                        let a = ancestor.file_name().unwrap().to_str().unwrap();
                        let b = match other {
                            Entry::File { name, stat } => name,
                            Entry::Dict {
                                name,
                                contents,
                                stat,
                            } => name,
                        };
                        // TODO: solve File not Found error when it obviously exists
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
pub fn build(src_path: &str, output_path: &str) {
    // TODO: assert path is a directory
    let working_dir = std::env::current_dir().unwrap();

    let zip_file = File::create(output_path).unwrap();
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = zip::write::FileOptions::default();

    // Create root
    let mut root = Entry::Dict {
        name: String::from("."),
        contents: Vec::new(),
        stat: stat_to_fuse_serializable(
            crate::libc_wrappers::lstat(OsString::from(".")).unwrap(),
        ),
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} [{elapsed_precise}] {msg}"));
    let mut counter = 1;

    std::env::set_current_dir(src_path).unwrap();
    let entries = WalkDir::new(".")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .min_depth(1);

    for entry in entries {
        pb.set_message(&format!("Processed entries: {}", counter));
        counter += 1;

        let e = entry.unwrap();
        let p = e.path();

        // For a file to be added, the parent has to have been added first so unwrapping should be safe.
        let parent = match p.parent() {
            None => &mut root,
            Some(x) => root.find_mut(x).unwrap(),
        };
        &parent.add_entry(p).unwrap();

        // Add to cache if it is a .txt-file
        if p.file_name().unwrap().to_str().unwrap().ends_with(".txt") {
            zip.start_file_from_path(p, options).unwrap();
            let mut file = File::open(p).unwrap();
            copy(&mut file, &mut zip).unwrap();
        }
    }

    pb.finish();

    // Store directory structure
    zip.start_file("files.json", options).unwrap();
    serde_json::to_writer_pretty(&mut zip, &root).unwrap();

    zip.finish().unwrap();

    // Restore original working directory
    std::env::set_current_dir(working_dir).unwrap();
}

pub fn load(path: &str) -> Entry {
    let file = File::open(path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    serde_json::from_reader(zip.by_name("files.json").unwrap()).unwrap()
}

pub fn load_from_zip(zip: &mut ZipArchive<File>) -> Entry {
    serde_json::from_reader(zip.by_name("files.json").unwrap()).unwrap()
}
