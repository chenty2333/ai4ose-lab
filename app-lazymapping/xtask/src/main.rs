use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// ArceOS lazymapping multi-architecture build & run tool
#[derive(Parser)]
#[command(name = "xtask", about = "Build and run arceos-lazymapping on different architectures")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the kernel for a given architecture
    Build {
        #[arg(long, default_value = "riscv64")]
        arch: String,
    },
    /// Build and run the kernel in QEMU
    Run {
        #[arg(long, default_value = "riscv64")]
        arch: String,
    },
}

#[allow(dead_code)]
struct ArchInfo {
    target: &'static str,
    platform: &'static str,
    objcopy_arch: &'static str,
}

fn arch_info(arch: &str) -> ArchInfo {
    match arch {
        "riscv64" => ArchInfo {
            target: "riscv64gc-unknown-none-elf",
            platform: "riscv64-qemu-virt",
            objcopy_arch: "riscv64",
        },
        "aarch64" => ArchInfo {
            target: "aarch64-unknown-none-softfloat",
            platform: "aarch64-qemu-virt",
            objcopy_arch: "aarch64",
        },
        "x86_64" => ArchInfo {
            target: "x86_64-unknown-none",
            platform: "x86-pc",
            objcopy_arch: "x86_64",
        },
        "loongarch64" => ArchInfo {
            target: "loongarch64-unknown-none",
            platform: "loongarch64-qemu-virt",
            objcopy_arch: "loongarch64",
        },
        _ => {
            eprintln!(
                "Error: unsupported architecture '{}'. \
                 Supported: riscv64, aarch64, x86_64, loongarch64",
                arch
            );
            process::exit(1);
        }
    }
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn install_config(root: &Path, arch: &str) {
    let src = root.join("configs").join(format!("{arch}.toml"));
    let dst = root.join(".axconfig.toml");
    if !src.exists() {
        eprintln!("Error: config file not found: {}", src.display());
        process::exit(1);
    }
    std::fs::copy(&src, &dst).unwrap_or_else(|e| {
        eprintln!("Error: failed to copy config: {}", e);
        process::exit(1);
    });
    println!("Installed config: {} -> .axconfig.toml", src.display());
}

/// Build the user-space payload binary for the target architecture.
/// Equivalent to `make payload` in the original workflow.
fn build_payload(root: &Path, info: &ArchInfo) -> PathBuf {
    println!("Building payload for {} ...", info.target);
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            info.target,
            "--bin",
            "origin",
            "--features",
            "payload",
            "--manifest-path",
            root.join("Cargo.toml").to_str().unwrap(),
        ])
        .status()
        .expect("failed to execute cargo build for payload");
    if !status.success() {
        eprintln!("Error: payload build failed");
        process::exit(status.code().unwrap_or(1));
    }

    // Objcopy to flat binary
    let elf = root
        .join("target")
        .join(info.target)
        .join("release")
        .join("origin");
    let bin = elf.with_extension("bin");

    let status = Command::new("rust-objcopy")
        .args([
            &format!("--binary-architecture={}", info.objcopy_arch),
            elf.to_str().unwrap(),
            "--strip-all",
            "-O",
            "binary",
            bin.to_str().unwrap(),
        ])
        .status()
        .expect("failed to execute rust-objcopy for payload");
    if !status.success() {
        eprintln!("Error: payload objcopy failed");
        process::exit(status.code().unwrap_or(1));
    }

    println!("Payload built: {}", bin.display());
    bin
}

/// Create a 64MB FAT32 disk image containing `/sbin/origin`.
/// Equivalent to `./update_disk.sh ./payload/origin/origin`.
fn create_fat_disk_image(path: &Path, payload_bin: &Path) {
    const DISK_SIZE: u64 = 64 * 1024 * 1024;

    // Read the payload binary
    let payload_data = std::fs::read(payload_bin).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to read payload {}: {}",
            payload_bin.display(),
            e
        );
        process::exit(1);
    });
    println!(
        "Payload binary size: {} bytes",
        payload_data.len()
    );

    // Create or truncate the image file
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create disk image: {}", e);
            process::exit(1);
        });
    file.set_len(DISK_SIZE).unwrap();

    // Format as FAT32
    let format_opts = fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32);
    fatfs::format_volume(&file, format_opts).unwrap_or_else(|e| {
        eprintln!("Error: failed to format FAT32: {}", e);
        process::exit(1);
    });

    // Populate filesystem
    {
        let fs = fatfs::FileSystem::new(&file, fatfs::FsOptions::new()).unwrap_or_else(|e| {
            eprintln!("Error: failed to open FAT filesystem: {}", e);
            process::exit(1);
        });
        let root_dir = fs.root_dir();

        // Create /sbin directory
        root_dir.create_dir("sbin").unwrap_or_else(|e| {
            eprintln!("Error: failed to create /sbin: {}", e);
            process::exit(1);
        });

        // Write payload as /sbin/origin
        let mut f = root_dir.create_file("sbin/origin").unwrap_or_else(|e| {
            eprintln!("Error: failed to create /sbin/origin: {}", e);
            process::exit(1);
        });
        f.write_all(&payload_data).unwrap();
        f.flush().unwrap();
    }

    println!(
        "Created FAT32 disk image: {} ({}MB) with /sbin/origin",
        path.display(),
        DISK_SIZE / (1024 * 1024)
    );
}

/// Build the kernel.
fn do_build(root: &Path, info: &ArchInfo) {
    let manifest = root.join("Cargo.toml");
    let ax_config = root.join(".axconfig.toml");
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            info.target,
            "--features",
            "axstd",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
        // Ensure dependencies read the intended config regardless of subprocess cwd.
        .env("AX_CONFIG_PATH", ax_config.to_str().unwrap())
        .status()
        .expect("failed to execute cargo build");
    if !status.success() {
        eprintln!("Error: cargo build failed");
        process::exit(status.code().unwrap_or(1));
    }
}

/// Convert ELF to raw binary.
fn do_objcopy(elf: &Path, bin: &Path, objcopy_arch: &str) {
    let status = Command::new("rust-objcopy")
        .args([
            &format!("--binary-architecture={objcopy_arch}"),
            elf.to_str().unwrap(),
            "--strip-all",
            "-O",
            "binary",
            bin.to_str().unwrap(),
        ])
        .status()
        .expect("failed to execute rust-objcopy");
    if !status.success() {
        eprintln!("Error: rust-objcopy failed");
        process::exit(status.code().unwrap_or(1));
    }
}

/// Run QEMU with VirtIO block device.
fn do_run_qemu(arch: &str, elf: &Path, bin: &Path, disk: &Path) {
    let mem = "128M";
    let smp = "1";
    let qemu = format!("qemu-system-{arch}");

    let mut args: Vec<String> = vec![
        "-m".into(),
        mem.into(),
        "-smp".into(),
        smp.into(),
        "-nographic".into(),
    ];

    match arch {
        "riscv64" => {
            args.extend([
                "-machine".into(),
                "virt".into(),
                "-bios".into(),
                "default".into(),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
            ]);
        }
        "aarch64" => {
            args.extend([
                "-cpu".into(),
                "cortex-a72".into(),
                "-machine".into(),
                "virt".into(),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
            ]);
        }
        "x86_64" => {
            args.extend([
                "-machine".into(),
                "q35".into(),
                "-kernel".into(),
                elf.to_str().unwrap().into(),
            ]);
        }
        "loongarch64" => {
            args.extend([
                "-machine".into(),
                "virt".into(),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
            ]);
        }
        _ => unreachable!(),
    }

    // VirtIO block device
    args.extend([
        "-drive".into(),
        format!("file={},format=raw,if=none,id=disk0", disk.display()),
        "-device".into(),
        "virtio-blk-pci,drive=disk0".into(),
    ]);

    println!("Running: {} {}", qemu, args.join(" "));
    let status = Command::new(&qemu)
        .args(&args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to run {}: {}", qemu, e);
            process::exit(1);
        });
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn main() {
    let cli = Cli::parse();
    let root = project_root();

    match cli.command {
        Cmd::Build { ref arch } => {
            let info = arch_info(arch);
            install_config(&root, arch);
            let _payload = build_payload(&root, &info);
            do_build(&root, &info);
            println!("Build complete for {arch} ({})", info.target);
        }
        Cmd::Run { ref arch } => {
            let info = arch_info(arch);
            install_config(&root, arch);

            // 1. Build payload (equivalent to `make payload`)
            let payload_bin = build_payload(&root, &info);

            // 2. Create disk image with payload (equivalent to `./update_disk.sh`)
            let disk = root.join("target").join(format!("disk-{arch}.img"));
            create_fat_disk_image(&disk, &payload_bin);

            // 3. Build kernel (equivalent to `make run A=tour/m_2_0 BLK=y`)
            do_build(&root, &info);

            let elf = root
                .join("target")
                .join(info.target)
                .join("release")
                .join("arceos-lazymapping");
            let bin = elf.with_extension("bin");

            if arch != "x86_64" {
                do_objcopy(&elf, &bin, info.objcopy_arch);
            }

            do_run_qemu(arch, &elf, &bin, &disk);
        }
    }
}
