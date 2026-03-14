use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// ArceOS readblk multi-architecture build & run tool
#[derive(Parser)]
#[command(name = "xtask", about = "Build and run arceos-readblk on different architectures")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the kernel for a given architecture
    Build {
        /// Target architecture: riscv64, aarch64, x86_64, loongarch64
        #[arg(long, default_value = "riscv64")]
        arch: String,
    },
    /// Build and run the kernel in QEMU
    Run {
        /// Target architecture: riscv64, aarch64, x86_64, loongarch64
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

/// Locate the project root.
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Copy the architecture-specific axconfig to .axconfig.toml
fn install_config(root: &Path, arch: &str) {
    let src = root.join("configs").join(format!("{arch}.toml"));
    let dst = root.join(".axconfig.toml");
    if !src.exists() {
        eprintln!("Error: config file not found: {}", src.display());
        process::exit(1);
    }
    std::fs::copy(&src, &dst).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to copy {} -> {}: {}",
            src.display(),
            dst.display(),
            e
        );
        process::exit(1);
    });
    println!("Installed config: {} -> .axconfig.toml", src.display());
}

/// Create a 64MB disk image with a FAT-like boot sector header.
///
/// The first 512-byte block contains:
/// - Bytes 0..3: JMP SHORT 0x3C; NOP (x86 boot jump)
/// - Bytes 3..11: OEM ID "mkfs.fat" (8 bytes, valid UTF-8)
///
/// This allows the application to read bytes 3..11 and parse them
/// as a UTF-8 string to verify block device I/O.
fn create_disk_image(path: &Path) {
    const DISK_SIZE: usize = 0x400_0000; // 64MB

    let mut boot_sector = vec![0u8; 512];

    // FAT boot sector: 3-byte jump instruction
    boot_sector[0] = 0xEB; // JMP SHORT
    boot_sector[1] = 0x3C; // offset
    boot_sector[2] = 0x90; // NOP

    // OEM ID at bytes 3..11
    let oem = b"mkfs.fat";
    boot_sector[3..11].copy_from_slice(oem);

    // Bytes per sector (512 = 0x0200, little-endian)
    boot_sector[11] = 0x00;
    boot_sector[12] = 0x02;

    let mut f = File::create(path).unwrap_or_else(|e| {
        eprintln!("Error: failed to create disk image {}: {}", path.display(), e);
        process::exit(1);
    });
    f.write_all(&boot_sector).unwrap();
    // Extend to full 64MB (sparse file)
    f.set_len(DISK_SIZE as u64).unwrap();

    println!(
        "Created disk image: {} ({}MB)",
        path.display(),
        DISK_SIZE / (1024 * 1024)
    );
}

/// Run cargo build for the target architecture.
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

/// Convert ELF to raw binary using rust-objcopy.
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
        .expect("failed to execute rust-objcopy (install with: cargo install cargo-binutils)");
    if !status.success() {
        eprintln!("Error: rust-objcopy failed");
        process::exit(status.code().unwrap_or(1));
    }
}

/// Run the kernel image in QEMU with a VirtIO block device.
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

    // Attach the disk image as a VirtIO PCI block device.
    args.extend([
        "-drive".into(),
        format!(
            "file={},format=raw,if=none,id=disk0",
            disk.display()
        ),
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
            do_build(&root, &info);
            println!("Build complete for {arch} ({})", info.target);
        }
        Cmd::Run { ref arch } => {
            let info = arch_info(arch);
            install_config(&root, arch);
            do_build(&root, &info);

            let elf = root
                .join("target")
                .join(info.target)
                .join("release")
                .join("arceos-readblk");
            let bin = elf.with_extension("bin");

            // Create disk image
            let disk = root.join("target").join("disk.img");
            create_disk_image(&disk);

            // objcopy for non-x86_64 architectures
            if arch != "x86_64" {
                do_objcopy(&elf, &bin, info.objcopy_arch);
            }

            do_run_qemu(arch, &elf, &bin, &disk);
        }
    }
}
