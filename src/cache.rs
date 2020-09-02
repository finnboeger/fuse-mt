#[cfg(feature = "cover")]
use crate::coverdb::CoverDB;
use crate::stat::stat_to_fuse_serializable;
use crate::types::{SerializableFileAttr, ArcBuf};
use crate::utils::*;
use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ffi::{OsString, OsStr};
use std::fs::File;
use std::io::copy;
use std::path::Path;
use walkdir::WalkDir;
#[cfg(feature = "mount")]
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize)]
pub enum Entry {
    Dict {
        name: OsString,
        contents: Vec<Entry>,
        stat: SerializableFileAttr,
    },
    File {
        name: OsString,
        stat: SerializableFileAttr,
        #[serde(skip)]
        #[serde(default = "new_none")] 
        contents: Option<ArcBuf>,
    },
}
fn new_none<T>() -> Option<T> {
    None
}

impl Entry {
    fn new(path: &Path) -> Self {
        // path needs to have a filename, otherwise we got a root, which is useless.
        // This function is private and the api would be annoying otherwise,
        // so we just require this.
        let name = path
            .file_name()
            .expect("Entry::new got a root")
            .to_os_string();
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
            if path.extension().map_or(false, |x| x == "txt") {
                // remove write permission as files will be read from cache and readonly.
                stat.perm = stat.perm & 0o5555;
            }
            Entry::File { name, stat, contents: None }
        }
    }

    fn add_entry(&mut self, path: &Path) -> Result<()> {
        match self {
            Entry::File { .. } => Err(anyhow!("Can't add entry to a file")),
            Entry::Dict {
                name: _,
                contents,
                stat: _,
            } => {
                contents.push(Entry::new(path));
                Ok(())
            }
        }
    }

    #[cfg(feature = "mount")]
    pub fn find(&self, path: &Path) -> Result<&Entry> {
        let path = path_to_rel(path);
        if path == Path::new("") {
            return Ok(self);
        }

        let mut item = self;
        for ancestor in path
            .ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip(1)
        {
            match item {
                Entry::File { .. } => return Err(anyhow!("Can't search in a file")),
                Entry::Dict {
                    name: _,
                    contents,
                    stat: _,
                } => {
                    // We're assuming that all Entries are sorted, therefore we can execute a binary search.
                    item = match contents.binary_search_by(|other: &Entry| -> Ordering {
                        let a = ancestor
                            .file_name()
                            .expect("Entry::find requires relative path");
                        let b = match other {
                            Entry::File { name, .. } => name,
                            Entry::Dict {
                                name,
                                contents: _,
                                stat: _,
                            } => name,
                        };
                        // TODO: solve File not Found error when it obviously exists
                        (**b).cmp(a)
                    }) {
                        Ok(i) => &contents[i],
                        Err(_) => return Err(anyhow!("File not found")),
                    };
                }
            }
        }
        Ok(item)
    }

    fn find_mut(&mut self, path: &Path) -> Result<&mut Entry> {
        let path = path_to_rel(path);
        if path == Path::new("") {
            return Ok(self);
        }

        let mut item = self;
        for ancestor in path
            .ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip(1)
        {
            match item {
                Entry::File { .. } => return Err(anyhow!("Can't search in a file")),
                Entry::Dict {
                    name: _,
                    contents,
                    stat: _,
                } => {
                    // We're assuming that all Entries are sorted, therefore we can execute a binary search.
                    item = match contents.binary_search_by(|other: &Entry| -> Ordering {
                        let a = ancestor
                            .file_name()
                            .expect("Entry::find_mut requires relative path");
                        let b = match other {
                            Entry::File { name, .. } => name,
                            Entry::Dict {
                                name,
                                contents: _,
                                stat: _,
                            } => name,
                        };
                        // TODO: solve File not Found error when it obviously exists
                        (**b).cmp(a)
                    }) {
                        Ok(i) => &mut contents[i],
                        Err(_) => return Err(anyhow!("File not found")),
                    };
                }
            }
        }
        Ok(item)
    }

    pub fn name(&self) -> &OsStr {
        match self {
            Entry::File { name, .. } => &name,
            Entry::Dict { name, .. } => &name,
        }
    }

    pub fn iter_files<F>(&mut self, mut f: F)
        where F: FnMut(&mut Entry, &Path)
    {
        fn iter_files_internal<F>(entry: &mut Entry, path: &Path, f: &mut F) 
            where F: FnMut(&mut Entry, &Path)
        {
            match entry {
                x @ Entry::File { .. } => {
                    f(x, &path.join(x.name()))
                },
                Entry::Dict { name, contents, .. } => {
                    let current_dir = path.join(name);
                    for entry in contents { iter_files_internal(entry, &current_dir, f) }
                }
            }
        }
        iter_files_internal(self, &Path::new(""), &mut f)
    }
}



fn add_txt_to_cache(
    p: &Path,
    zip: &mut zip::ZipWriter<File>,
    options: &zip::write::FileOptions,
) -> Result<()> {
    zip.start_file_from_path(p, *options)
        .context("Failed to start zip file")?;
    let mut file = File::open(p)?;
    copy(&mut file, zip).context("Failed to copy into cache")?;
    Ok(())
}

fn add_audio_to_cache(
    p: &Path,
    zip: &mut zip::ZipWriter<File>,
    options: &zip::write::FileOptions,
) -> Result<()> {
    use std::io::{Read, Write};

    let extension = p.extension().expect("Extension is tested previously")
        .to_str().expect("Extension was tested against utf8-strings previously");
    zip.start_file_from_path(&p.with_extension(&format!("{}.part", extension)), *options)?;
    let mut file = File::open(p)?;
    let mut counter = 0;
    while counter < 16_384 {
        let mut buf = [0; 128];
        let mut size = file.read(&mut buf)?;
        if size == 0 {
            break;
        }
        if counter + size > 16_384 {
            size = 16_384 - counter;
        };
        zip.write_all(&buf[0..size])?;
        counter += size;
    }
    Ok(())
}

#[cfg(feature = "cover")]
fn add_to_coverdb(p: &Path, cover_db: &mut CoverDB) -> Result<()> {
    // ultrastar-txt's errors are not Sync, which anyhow needs
    let txt = ultrastar_txt::parse_txt_song(p)
        .map_err(|err| anyhow!("Unable to parse song file: {}", err))?;
    if let Some(cover_path) = txt.header.cover_path {
        cover_db
            .add(&cover_path)
            .with_context(|| format!("Failed to load cover '{}' into db", cover_path.display()))?;
    }
    Ok(())
}

#[allow(unused_variables)]
pub fn build<P1: AsRef<Path>, P2: AsRef<Path>>(
    src_path: P1,
    output_path: P2,
    generate_coverdb: bool,
    cache_audio: bool,
) -> Result<()> {
    let src_path = src_path.as_ref();
    let output_path = output_path.as_ref();
    assert!(src_path.is_dir());
    let working_dir = std::env::current_dir();

    let zip_file = File::create(output_path).context("Unable to create cache.zip")?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = zip::write::FileOptions::default();

    // Create root
    let mut root = Entry::Dict {
        name: OsString::from("."),
        contents: Vec::new(),
        stat: stat_to_fuse_serializable(
            crate::libc_wrappers::lstat(OsString::from(src_path))
                .map_err(|errno| std::io::Error::from_raw_os_error(errno))
                .with_context(|| format!("Unable to read stats of '{}'", src_path.display()))?,
        ),
    };

    // Create Cache DB
    #[cfg(feature = "cover")]
    let mut cover_db = CoverDB::new(src_path).context("Unable to initialize cover.db")?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner().template("{spinner:.green} [{elapsed_precise}] {msg}"),
    );
    let mut counter = 1;

    std::env::set_current_dir(src_path)
        .with_context(|| format!("Unable to change current_dir to '{}'", src_path.display()))?;
    let entries = WalkDir::new(".")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .min_depth(1);

    for entry in entries {
        pb.set_message(&format!("Processed entries: {}", counter));
        counter += 1;

        let e = match entry {
            Ok(e) => e,
            Err(err) => {
                warn!("Unable to process: '{}'", err);
                continue;
            }
        };
        let p = e.path();

        // For a file to be added, the parent has to have been added first so unwrapping should be safe.
        let parent = match p.parent() {
            None => &mut root,
            Some(x) => root.find_mut(x)?,
        };
        parent.add_entry(p)?;

        if p.extension().map_or(false, |x| x == "txt") {
            // Add to cache if it is a .txt-file
            if let Err(err) = add_txt_to_cache(p, &mut zip, &options) {
                pb.println(format!("[WARN] Unable to cache '{}': {}", p.display(), err));
                continue;
            }

            // Generate cover db entry, if this is a .txt-file
            #[cfg(feature = "cover")]
            if generate_coverdb {
                if let Err(err) = add_to_coverdb(p, &mut cover_db) {
                    pb.println(format!(
                        "[WARN] Unable to add to cover database '{}': {}",
                        p.display(),
                        err
                    ));
                    continue;
                }
            }
        }

        if cache_audio && p.extension().map_or(false,
            |x| x == "mp3" || x == "m4a" || x == "ogg" || x == "wav" || x == "wma" || x == "flac"
        ) {
            if let Err(err) = add_audio_to_cache(p, &mut zip, &options) {
                pb.println(format!(
                    "[WARN] Unable to add music header for '{}': {}",
                    p.display(),
                    err
                ));
                continue;
            }
        }
    }

    pb.finish();

    // Store directory structure
    zip.start_file("files.json", options)
        .context("Failed to create 'files.json' in cache.zip")?;
    serde_json::to_writer_pretty(&mut zip, &root)
        .context("Failed to write 'files.json' in cache.zip")?;

    // Store coverdb
    #[cfg(feature = "cover")]
    {
        zip.start_file("cover.db", options)
            .context("Failed to add cover.db to cache.zip")?;
        cover_db
            .write(&mut zip)
            .context("Failed to write cover.db to cache.zip")?;
    }

    zip.finish().context("Failed to finish up cache.zip")?;

    // Restore original working directory (if any)
    if let Ok(working_dir) = working_dir {
        // ignore failure
        let _ = std::env::set_current_dir(working_dir);
    }

    Ok(())
}

#[cfg(feature = "mount")]
pub fn load_from_zip(zip: &mut ZipArchive<File>) -> Result<Entry> {
    serde_json::from_reader(
        zip.by_name("files.json")
            .context("Cache contains no files.json / is malformed")?,
    )
    .context("files.json is no valid json")
    .into()
}
