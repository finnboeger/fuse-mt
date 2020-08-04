// Main Entry Point :: A fuse_mt test program.
//
// Copyright (c) 2016-2020 by William R. Fraser
//

#![deny(rust_2018_idioms)]

use std::env;
use std::ffi::{OsStr, OsString};
use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

#[macro_use]
extern crate log;

mod libc_extras;
mod libc_wrappers;
mod passthrough;

fn main() {
    Builder::new()
        .format(|buf, record| {
            writeln!(buf,
                     "{} [{}]: {}: {}",
                     Local::now().format("%Y-%m-%dT%H:%M:%S"),
                     record.level(),
                     record.target(),
                     record.args()
            )})
        .filter(Some("fuse_mt"), LevelFilter::Warn)
        .filter(Some("fuse"), LevelFilter::Warn)
        .filter(None, LevelFilter::Warn)
        .init();

    let args: Vec<OsString> = env::args_os().collect();

    if args.len() != 3 {
        println!("usage: {} <target> <mountpoint>", &env::args().next().unwrap());
        ::std::process::exit(-1);
    }

    let filesystem = passthrough::PassthroughFS {
        target: args[1].clone(),
    };

    let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];

    fuse_mt::mount(fuse_mt::FuseMT::new(filesystem, 1), &args[2], &fuse_args).unwrap();
}
