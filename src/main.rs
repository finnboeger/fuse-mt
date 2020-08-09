// Main Entry Point :: A fuse_mt test program.
//
// Copyright (c) 2016-2020 by William R. Fraser
//

#![deny(rust_2018_idioms)]

use chrono::Local;
use clap::{App, AppSettings, Arg, SubCommand};
use env_logger::Builder;
use log::LevelFilter;
use std::ffi::{OsStr, OsString};
use std::io::Write;

#[macro_use]
extern crate log;

mod cache;
mod file_handles;
mod libc_extras;
mod libc_wrappers;
mod passthrough;
mod stat;
mod types;

fn main() {
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

    let matches = App::new("Ultrastar-Fs")
        .version("0.1.0")
        .author("Finn Böger <finnboeger@gmail.com>")
        .about("A jump start for ultrastar deluxe when using large song collections and/or slow media")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("mount")
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
                .required(true)))
        .subcommand(SubCommand::with_name("build")
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
                .help("Specify where the created cache file should be saved.")))
        .get_matches();

    match matches.subcommand() {
        ("mount", Some(sub_matches)) => {
            // TODO: load and use cache

            let filesystem = passthrough::PassthroughFS::new(
                OsString::from(sub_matches.value_of_os("source").unwrap()),
                sub_matches.value_of("cache").unwrap(),
            );

            let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];

            let mount_point: OsString = OsString::from(sub_matches.value_of_os("target").unwrap());

            fuse_mt::mount(
                fuse_mt::FuseMT::new(filesystem, 1),
                &mount_point,
                &fuse_args,
            )
            .unwrap();
        }
        ("build", Some(sub_matches)) => {
            cache::build(
                sub_matches.value_of("root").unwrap(),
                sub_matches.value_of("output").unwrap(),
            );
        }
        _ => {}
    }
}
