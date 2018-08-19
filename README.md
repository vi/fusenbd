fusenbd
---

A FUSE mounter for network block device.

Example
---

```
$ fusenbd data '127.0.0.1:10809' sda1 -r -- -o auto_unmount,default_permissions,allow_other,ro&
[1] 14013

$ mkdir -p m

$ ntfs-3g -o ro ./data m

$ ls m
Boot  bootmgr  BOOTSECT.BAK  System Volume Information

$ fusermount -u m

$ fusermount -u data
[1]+  Done         fusenbd 
```

Usage
---

```
fusenbd 0.1.0
Vitaly _Vi Shukela <vi0oss@gmail.com>
FUSE-based network block device client that exposes NBD export as a plain file

USAGE:
    fusenbd [FLAGS] <file> <hostport> [ARGS]

FLAGS:
    -h, --help         Prints help information
    -r, --read-only    Mount read-only
    -V, --version      Prints version information

ARGS:
    <file>        Regular file to use as mountpoint
    <hostport>    Host:port to make NBD connection
    <export>      Named export to use. [default: ]
    <opts>...     The rest of FUSE options. Specify export as "" to use default and skip to FUSE options.


Example:
    fusenbd nbd.dat 127.0.0.1:10809
    
    fusenbd -r sda1 127.0.0.1:10809 sda1 -- -o allow_empty,ro,fsname=qwerty,auto_unmount
```


Building and installing
---

For Linux x86_64, you can try a [pre-built version](https://github.com/vi/fusenbd/releases).
Otherwise,

1. Setup [rust toolchain](rustup.rs)
2. Install FUSE development headers (`apt-get install libfuse-dev`)
3. `cargo install fusenbd`.
4. Either use `fusenbd` right away or find it somewhere and copy to `$PATH`.

