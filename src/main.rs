#![allow(unused,dead_code)]

#[macro_use]
extern crate structopt;
extern crate fuse;
extern crate nbd;

use structopt::StructOpt;
use std::path::PathBuf;
use fuse::Filesystem;

struct NbdFs {

}

impl Filesystem for NbdFs {

}

#[derive(StructOpt, Debug)]
#[structopt(
    after_help = "
Example:
    touch nbd.dat
    fusenbd nbd.dat 127.0.0.1:10809
",
)]
struct Opt {
    /// Regular file to use as mountpoint
    #[structopt(parse(from_os_str))]
    file: PathBuf,
    /// Host:port to make NBD connection
    hostport: String,
    /// Named export to use.
    #[structopt(default_value="")]
    export: String,

    /// Mount read-only
    #[structopt(short="r",long="read-only")]
    ro : bool,
}


fn main() {
    let mut cmd = Opt::from_args();
}
