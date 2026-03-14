#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

#[cfg(feature = "axstd")]
extern crate axfs;
#[cfg(feature = "axstd")]
extern crate axio;
#[cfg(feature = "axstd")]
extern crate axfs_ng_vfs;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use axfs::ROOT_FS_CONTEXT;
        use axfs::File;
        use axfs_ng_vfs::NodePermission;
        use axio::Write;

        let full_dir_mode = NodePermission::OWNER_READ
            | NodePermission::OWNER_WRITE
            | NodePermission::OWNER_EXEC
            | NodePermission::GROUP_READ
            | NodePermission::GROUP_WRITE
            | NodePermission::GROUP_EXEC
            | NodePermission::OTHER_READ
            | NodePermission::OTHER_WRITE
            | NodePermission::OTHER_EXEC;

        let ctx = ROOT_FS_CONTEXT.get().expect("Root FS not initialized");

        println!("Create '/tmp' ...");
        ctx.create_dir("/tmp", full_dir_mode)
            .unwrap_or_else(|e| panic!("Cannot create /tmp: {:?}", e));

        println!("Create '/tmp/dira' ...");
        ctx.create_dir("/tmp/dira", full_dir_mode)
            .unwrap_or_else(|e| panic!("Cannot create /tmp/dira: {:?}", e));

        println!("Rename '/tmp/dira' to '/tmp/dirb' ...");
        ctx.rename("/tmp/dira", "/tmp/dirb")
            .unwrap_or_else(|e| panic!("Cannot rename dira -> dirb: {:?}", e));

        println!("Create '/tmp/a.txt' and write [hello] ...");
        let file = File::create(ctx, "/tmp/a.txt")
            .unwrap_or_else(|e| panic!("Cannot create /tmp/a.txt: {:?}", e));
        (&file)
            .write_all(b"hello")
            .unwrap_or_else(|e| panic!("Cannot write /tmp/a.txt: {:?}", e));

        println!("Rename '/tmp/a.txt' to '/tmp/b.txt' ...");
        ctx.rename("/tmp/a.txt", "/tmp/b.txt")
            .unwrap_or_else(|e| panic!("Cannot rename a.txt -> b.txt: {:?}", e));

        println!("Move '/tmp/b.txt' to '/tmp/dirb/b.txt' ...");
        move_file(ctx, "/tmp/b.txt", "/tmp/dirb/b.txt")
            .unwrap_or_else(|e| panic!("Cannot move b.txt -> dirb/b.txt: {:?}", e));

        println!("Read '/tmp/dirb/b.txt' ...");
        let file = File::open(ctx, "/tmp/dirb/b.txt")
            .unwrap_or_else(|e| panic!("Cannot open moved file: {:?}", e));
        let mut buf = [0u8; 16];
        let n = (&file)
            .read(&mut buf[..])
            .unwrap_or_else(|e| panic!("Cannot read moved file: {:?}", e));
        println!(
            "Read '/tmp/dirb/b.txt' content: [{}]",
            core::str::from_utf8(&buf[..n]).unwrap()
        );

        println!("\n[Ramfs-Rename]: ok!");

        fn move_file(ctx: &axfs::FsContext, src: &str, dst: &str) -> Result<(), axio::Error> {
            let src_file = File::open(ctx, src).map_err(|_| axio::Error::NotFound)?;
            let dst_file = File::create(ctx, dst).map_err(|_| axio::Error::AlreadyExists)?;
            let mut buf = [0u8; 256];
            loop {
                let n = (&src_file).read(&mut buf[..])?;
                if n == 0 {
                    break;
                }
                (&dst_file).write_all(&buf[..n])?;
            }
            ctx.remove_file(src).map_err(|_| axio::Error::InvalidInput)?;
            Ok(())
        }
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for filesystem access.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
