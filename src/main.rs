// Main Entry Point :: A fuse_mt test program.
//
// Copyright (c) 2016-2020 by William R. Fraser
//

#![deny(rust_2018_idioms)]

use anyhow::{Context, Result};
use chrono::Local;
use clap::{App, AppSettings, Arg, SubCommand};
use env_logger::Builder;
use log::LevelFilter;
#[cfg(feature = "mount")]
use std::ffi::{OsStr, OsString};
use std::io::Write;

#[macro_use]
extern crate log;
#[cfg_attr(feature = "cover", macro_use)]
#[cfg(feature = "cover")]
extern crate diesel;

mod cache;
#[cfg(feature = "cover")]
mod coverdb;
mod file_handles;
mod libc_extras;
mod libc_wrappers;
#[cfg(feature = "mount")]
mod passthrough;
mod stat;
mod types;
mod utils;

fn main() -> Result<()> {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}]: {}: {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter(Some("fuse_mt"), LevelFilter::Warn)
        .filter(Some("fuse"), LevelFilter::Warn)
        .filter(None, LevelFilter::Warn)
        .init();

    let mut app = App::new("Ultrastar-Fs")
        .version("0.1.0")
        .author("Finn Böger <finnboeger@gmail.com>")
        .about("A jump start for ultrastar deluxe when using large song collections and/or slow media")
        .setting(AppSettings::SubcommandRequiredElseHelp);
    
    #[cfg(feature = "mount")]
    {
        #[allow(unused_mut)]
        let mut mount_command = SubCommand::with_name("mount")
                .about("Mirrors a given directory while using the cache to speed up i.a. directory listings")
                .arg(Arg::with_name("cache")
                    .short("c")
                    .long("cache")
                    .takes_value(true)
                    .value_name("FILE")
                    .default_value("cache.zip")
                    .help("Sets a custom cache file."))
                .arg(Arg::with_name("source")
                    .help("Sets the directory that will be mirrored.")
                    .required(true))
                .arg(Arg::with_name("target")
                    .help("Sets the mount point.")
                    .required(true));

        #[cfg(feature = "cover")]
        {
            mount_command = mount_command
                .arg(Arg::with_name("coverdb")
                    .value_name("IMPORT_COVER_DB")
                    .short("i")
                    .long("import-coverdb")
                    .takes_value(true)
                    .required(false)
                    .help("Specify where the coverdb file is to import into"));
        }

        app = app.subcommand(mount_command);
    }
    
    let cache_command = SubCommand::with_name("build")
            .about("Creates the cache to be used")
            .arg(Arg::with_name("root")
                .value_name("ROOT_DIR")
                .required(true)
                .help("set root directory from which the cache will be created."))
            .arg(Arg::with_name("output")
                .short("o")
                .long("output")
                .takes_value(true)
                .value_name("FILE")
                .default_value("cache.zip")
                .help("Specify where the created cache file should be saved."));
    
    #[cfg(feature = "cover")]
    let cache_command = cache_command.arg(Arg::with_name("nocoverdb")
        .value_name("NO_COVER_DB")
        .required(false)
        .short("s")
        .long("skip-coverdb")
        .takes_value(false)
        .help("Skips creation of a relative cover_db file with can be loaded by the mount-command to skip thumbnail generation of ultrastar"));
            
    app = app.subcommand(cache_command);

    let matches = app.get_matches();

    match matches.subcommand() {
        #[cfg(feature = "mount")]
        ("mount", Some(sub_matches)) => {
            // TODO: load and use cache

            #[cfg(not(feature = "cover"))]
            let cover = None;
            #[cfg(feature = "cover")]
            let cover = sub_matches.value_of("coverdb").map(std::path::PathBuf::from);

            let filesystem = passthrough::PassthroughFS::new(
                sub_matches.value_of_os("source").expect("'source' is required").into(),
                sub_matches.value_of_os("target").expect("'target' is required").into(),
                sub_matches.value_of("cache").expect("'cache' has default"),
                cover,
            ).context("Unable to load filesystem")?;

            println!("Filesystem has been created");

            // TODO: add heuristic to detect ultrastardx startup and display progress bar based on that.

            let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];

            let mount_point: OsString = sub_matches.value_of_os("target").expect("'target' is required").into();

            fuse_mt::mount(
                fuse_mt::FuseMT::new(filesystem, 1),
                &mount_point,
                &fuse_args,
            )?
        },
        ("build", Some(sub_matches)) => {
            #[cfg(not(feature = "cover"))]
            let cover = false;
            #[cfg(feature = "cover")]
            let cover = !sub_matches.is_present("nocoverdb");
            cache::build(
                sub_matches.value_of("root").expect("'root' is required"),
                sub_matches.value_of("output").expect("'output' has default value"),
                cover,
            )?;
        }
        _ => {}
    };

    Ok(())
}
