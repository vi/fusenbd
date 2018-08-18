#![allow(unused, dead_code)]

#[macro_use]
extern crate structopt;
extern crate fuse;
extern crate nbd;

use fuse::Filesystem;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use structopt::StructOpt;

use std::fs::File;
use std::io::{Error, ErrorKind, Result};

use std::net::TcpStream;

use nbd::client::{handshake, NbdClient};

struct NbdFs {}

impl Filesystem for NbdFs {}

mod readwriteseekfs {
    extern crate fuse;
    extern crate libc;
    extern crate time;
    use self::fuse::{
        FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyEmpty, ReplyEntry, ReplyWrite,
        Request,
    };
    use self::time::Timespec;
    use std::ffi::OsStr;
    use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
    use std::path::Path;

    const CREATE_TIME: Timespec = Timespec {
        sec: 1534631479,
        nsec: 0,
    }; //FIXME

    const TTL: Timespec = Timespec { sec: 9999, nsec: 0 };

    use self::libc::c_int;
    fn errmap(e: Error) -> c_int {
        use self::libc::*;
        use ErrorKind::*;
        // TODO parse Other's Display and derive more error codes
        match e.kind() {
            NotFound => ENOENT,
            PermissionDenied => EACCES,
            ConnectionRefused => ECONNREFUSED,
            ConnectionReset => ECONNREFUSED,
            ConnectionAborted => ECONNABORTED,
            NotConnected => ENOTCONN,
            AddrInUse => EADDRINUSE,
            AddrNotAvailable => EADDRNOTAVAIL,
            BrokenPipe => EPIPE,
            AlreadyExists => EEXIST,
            WouldBlock => EWOULDBLOCK,
            InvalidInput => EINVAL,
            InvalidData => EINVAL,
            TimedOut => ETIMEDOUT,
            WriteZero => EINVAL,
            UnexpectedEof => EINVAL,
            _ => EINVAL,
        }
    }

    trait MyReadEx: Read {
        // Based on https://doc.rust-lang.org/src/std/io/mod.rs.html#620
        fn read_exact2(&mut self, mut buf: &mut [u8]) -> ::std::io::Result<usize> {
            let mut successfully_read = 0;
            while !buf.is_empty() {
                match self.read(buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        successfully_read += n;
                        let tmp = buf;
                        buf = &mut tmp[n..];
                    }
                    Err(ref e) if e.kind() == ::std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(successfully_read)
        }
    }
    impl<T: Read> MyReadEx for T {}

    trait MyWriteEx: Write {
        fn write_all2(&mut self, mut buf: &[u8]) -> Result<usize> {
            let mut successfully_written = 0;
            while !buf.is_empty() {
                match self.write(buf) {
                    Ok(0) => return Ok(successfully_written),
                    Ok(n) => {
                        successfully_written += n;
                        buf = &buf[n..];
                    }
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(successfully_written)
        }
    }
    impl<T: Write> MyWriteEx for T {}

    pub struct ReadWriteSeekFs<F: Read> {
        file: F,
        fa: FileAttr,
    }

    impl<F> ReadWriteSeekFs<F>
    where
        F: Read + Write + Seek,
    {
        pub fn new(mut f: F, bs: usize) -> Result<ReadWriteSeekFs<F>> {
            let len = f.seek(SeekFrom::End(0))?;
            let blocks = ((len - 1) / (bs as u64)) + 1;

            Ok(ReadWriteSeekFs {
                file: f,
                fa: FileAttr {
                    ino: 1,
                    size: len,
                    blocks: blocks,
                    atime: CREATE_TIME,
                    mtime: CREATE_TIME,
                    ctime: CREATE_TIME,
                    crtime: CREATE_TIME,
                    kind: FileType::RegularFile,
                    perm: 0o644,
                    nlink: 1,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                },
            })
        }

        fn seek(&mut self, offset: i64) -> Result<()> {
            if offset < 0 {
                Err(ErrorKind::InvalidInput)?;
            }
            self.file.seek(SeekFrom::Start(offset as u64))?;
            Ok(())
        }
        fn seek_and_read(&mut self, offset: i64, size: usize) -> Result<Vec<u8>> {
            self.seek(offset)?;
            let mut buf = vec![0; size as usize];
            let ret = self.file.read_exact2(&mut buf)?;
            buf.truncate(ret);
            Ok(buf)
        }

        fn seek_and_write(&mut self, offset: i64, data: &[u8]) -> Result<usize> {
            self.seek(offset)?;
            self.file.write_all2(data)
        }
    }

    impl<F> Filesystem for ReadWriteSeekFs<F>
    where
        F: Read + Write + Seek,
    {
        fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
            reply.entry(&TTL, &self.fa, 0);
        }

        fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
            reply.attr(&TTL, &self.fa);
        }

        fn read(
            &mut self,
            _req: &Request,
            ino: u64,
            _fh: u64,
            offset: i64,
            size: u32,
            reply: ReplyData,
        ) {
            match self.seek_and_read(offset, size as usize) {
                Ok(buf) => reply.data(buf.as_slice()),
                Err(e) => reply.error(errmap(e)),
            }
        }

        fn write(
            &mut self,
            _req: &Request,
            _ino: u64,
            _fh: u64,
            offset: i64,
            data: &[u8],
            _flags: u32,
            reply: ReplyWrite,
        ) {
            match self.seek_and_write(offset, data) {
                Ok(len) => reply.written(len as u32),
                Err(e) => reply.error(errmap(e)),
            }
        }

        fn flush(
            &mut self,
            _req: &Request,
            _ino: u64,
            _fh: u64,
            _lock_owner: u64,
            reply: ReplyEmpty,
        ) {
            match self.file.flush() {
                Ok(()) => reply.ok(),
                Err(e) => reply.error(errmap(e)),
            }
        }

        fn setattr(
            &mut self,
            _req: &Request,
            _ino: u64,
            _mode: Option<u32>,
            _uid: Option<u32>,
            _gid: Option<u32>,
            _size: Option<u64>,
            _atime: Option<Timespec>,
            _mtime: Option<Timespec>,
            _fh: Option<u64>,
            _crtime: Option<Timespec>,
            _chgtime: Option<Timespec>,
            _bkuptime: Option<Timespec>,
            _flags: Option<u32>,
            reply: ReplyAttr,
        ) {
            reply.attr(&TTL, &self.fa);
        }
    }

}

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
    let export = handshake(&mut tcp, cmd.export.as_bytes())?;
    let mut client = NbdClient::new(&mut tcp, &export);

    let fs = readwriteseekfs::ReadWriteSeekFs::new(client, 1024)?;

    let opts: Vec<&OsStr> = cmd.opts.iter().map(AsRef::as_ref).collect();
    fuse::mount(fs, &cmd.file.as_path(), opts.as_slice())
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("fusenbd: {}", e);
        ::std::process::exit(1);
    }
}
