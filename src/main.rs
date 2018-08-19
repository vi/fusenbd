#![allow(unused, dead_code)]

#[macro_use]
extern crate structopt;
extern crate fuse;
extern crate nbd;
extern crate readwriteseekfs;
extern crate bufstream;

use fuse::Filesystem;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use structopt::StructOpt;

use std::fs::File;
use std::io::{Error, ErrorKind, Result};

use std::net::TcpStream;

use nbd::client::{handshake, NbdClient};
use readwriteseekfs::{ReadSeekFs,ReadWriteSeekFs};

#[derive(StructOpt, Debug)]
#[structopt(
    after_help = "
Example:
    fusenbd nbd.dat 127.0.0.1:10809
    
    fusenbd -r sda1 127.0.0.1:10809 sda1 -- -o allow_empty,ro,fsname=qwerty,auto_unmount
",
)]
struct Opt {
    /// Regular file to use as mountpoint
    #[structopt(parse(from_os_str))]
    file: PathBuf,
    /// Host:port to make NBD connection
    hostport: String,
    /// Named export to use.
    #[structopt(default_value = "")]
    export: String,

    /// Mount read-only
    #[structopt(short = "r", long = "read-only")]
    ro: bool,

    /// The rest of FUSE options. Specify export as "" to use default and skip to FUSE options.
    #[structopt(parse(from_os_str))]
    opts: Vec<OsString>,
}

fn run() -> Result<()> {
    let mut cmd = Opt::from_args();

    match cmd.file.metadata() {
        Ok(ref m) if m.is_dir() => eprintln!("Warning: {:?} is a directory, not a file", cmd.file),
        Ok(ref m) if m.is_file() => (),
        Ok(_) => eprintln!("Warning: can't determine type of {:?}", cmd.file),
        Err(ref e) if e.kind() == ErrorKind::NotFound => {
            drop(File::create(cmd.file.clone()));
        }
        Err(e) => Err(e)?,
    }

    let mut tcp = TcpStream::connect(cmd.hostport)?;
    let mut tcp = bufstream::BufStream::new(tcp);
    let export = handshake(&mut tcp, cmd.export.as_bytes())?;
    let mut client = NbdClient::new(&mut tcp, &export);

    let opts: Vec<&OsStr> = cmd.opts.iter().map(AsRef::as_ref).collect();
    
    if cmd.ro {
        let fs = readwriteseekfs::ReadSeekFs::new(client, 1024)?;
        fuse::mount(fs, &cmd.file.as_path(), opts.as_slice())
    } else {
        let fs = readwriteseekfs::ReadWriteSeekFs::new(client, 1024)?;
        fuse::mount(fs, &cmd.file.as_path(), opts.as_slice())
    }
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("fusenbd: {}", e);
        ::std::process::exit(1);
    }
}
