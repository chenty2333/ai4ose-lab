use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// ArceOS Guest Address Space — multi-architecture build & run tool
#[derive(Parser)]
#[command(name = "xtask", about = "Build and run arceos-guestaspace on different architectures")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the kernel for a given architecture
    Build {
        /// Target architecture: riscv64, aarch64, x86_64
        #[arg(long, default_value = "riscv64")]
        arch: String,
    },
    /// Build and run the kernel in QEMU
    Run {
        /// Target architecture: riscv64, aarch64, x86_64
        #[arg(long, default_value = "riscv64")]
        arch: String,
    },
}

#[derive(Clone)]
struct ArchInfo {
    target: &'static str,
    #[allow(dead_code)]
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
        _ => {
            eprintln!(
                "Error: unsupported architecture '{}'. \
                 Supported: riscv64, aarch64, x86_64",
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

/// Install the platform config for the guest payload.
fn install_payload_config(root: &Path, arch: &str) {
    let payload_dir = root.join("payload").join("gkernel");
    let src = payload_dir.join("configs").join(format!("{arch}.toml"));
    let dst = payload_dir.join(".axconfig.toml");
    if !src.exists() {
        eprintln!("Error: payload config file not found: {}", src.display());
        process::exit(1);
    }
    std::fs::copy(&src, &dst).unwrap_or_else(|e| {
        eprintln!("Error: failed to copy payload config: {}", e);
        process::exit(1);
    });
    println!(
        "Installed payload config: {} -> payload/gkernel/.axconfig.toml",
        src.display()
    );
}

/// Build the guest payload (gkernel = readpflash) for the target architecture.
///
/// The payload is a full ArceOS application built with the `axstd` feature.
fn build_payload(root: &Path, info: &ArchInfo, arch: &str) -> PathBuf {
    let payload_dir = root.join("payload").join("gkernel");
    let manifest = root.join("Cargo.toml");

    println!("Building payload (gkernel) for {arch} ...");

    let mut cmd = Command::new("cargo");

    // For riscv64: full ArceOS guest via axstd feature.
    // Also set AX_CONFIG_PATH so the platform crate picks up our custom config
    // (including pflash MMIO range). The crates.io default config lacks pflash.
    if arch == "riscv64" {
        let axconfig_path = payload_dir.join(".axconfig.toml");
        println!(
            "Setting AX_CONFIG_PATH={} for payload build",
            axconfig_path.display()
        );
        cmd.env("AX_CONFIG_PATH", axconfig_path.to_str().unwrap());
    }

    let mut build_args = vec![
        "build".to_string(),
        "--release".into(),
        "--manifest-path".into(),
        manifest.to_str().unwrap().to_string(),
        "--target".into(),
        info.target.to_string(),
        "--bin".into(),
        "gkernel".into(),
    ];

    // All architectures use axstd (full ArceOS guest with multitasking)
    // Always add guest-kernel feature
    build_args.push("--features".into());
    build_args.push("guest-kernel".into());

    let status = cmd
        .args(&build_args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to run cargo build for payload: {}", e);
            process::exit(1);
        });

    if !status.success() {
        eprintln!("Error: payload compilation failed");
        process::exit(status.code().unwrap_or(1));
    }

    let payload_elf = root
        .join("target")
        .join(info.target)
        .join("release")
        .join("gkernel");

    let payload_bin = payload_elf.with_extension("bin");

    // Convert ELF to flat binary
    let mut objcopy_args = vec![
        format!("--binary-architecture={}", info.objcopy_arch),
        payload_elf.to_str().unwrap().to_string(),
        "--strip-all".into(),
        "-O".into(),
        "binary".into(),
        payload_bin.to_str().unwrap().to_string(),
    ];

    // For x86_64, we don't pass --binary-architecture (not needed for ELF→bin)
    if info.objcopy_arch == "x86_64" {
        objcopy_args = vec![
            payload_elf.to_str().unwrap().to_string(),
            "--strip-all".into(),
            "-O".into(),
            "binary".into(),
            payload_bin.to_str().unwrap().to_string(),
        ];
    }

    let status = Command::new("rust-objcopy")
        .args(&objcopy_args)
        .status()
        .expect("failed to execute rust-objcopy for payload");

    if !status.success() {
        eprintln!("Error: rust-objcopy for payload failed");
        process::exit(status.code().unwrap_or(1));
    }

    // Print binary size
    if let Ok(meta) = std::fs::metadata(&payload_bin) {
        println!(
            "Payload built: {} ({} bytes, {} KB)",
            payload_bin.display(),
            meta.len(),
            meta.len() / 1024
        );
    }

    payload_bin
}

/// Create a 64MB FAT32 disk image containing `/sbin/gkernel`.
fn create_fat_disk_image(path: &Path, payload_bin: &Path) {
    const DISK_SIZE: u64 = 64 * 1024 * 1024;

    let payload_data = std::fs::read(payload_bin).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to read payload {}: {}",
            payload_bin.display(),
            e
        );
        process::exit(1);
    });
    println!("Payload binary size: {} bytes", payload_data.len());

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

    let format_opts = fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32);
    fatfs::format_volume(&file, format_opts).unwrap_or_else(|e| {
        eprintln!("Error: failed to format FAT32: {}", e);
        process::exit(1);
    });

    {
        let fs = fatfs::FileSystem::new(&file, fatfs::FsOptions::new()).unwrap_or_else(|e| {
            eprintln!("Error: failed to open FAT filesystem: {}", e);
            process::exit(1);
        });
        let root_dir = fs.root_dir();

        root_dir.create_dir("sbin").unwrap_or_else(|e| {
            eprintln!("Error: failed to create /sbin: {}", e);
            process::exit(1);
        });

        let mut f = root_dir.create_file("sbin/gkernel").unwrap_or_else(|e| {
            eprintln!("Error: failed to create /sbin/gkernel: {}", e);
            process::exit(1);
        });
        f.write_all(&payload_data).unwrap();
        f.flush().unwrap();
    }

    println!(
        "Created FAT32 disk image: {} ({}MB) with /sbin/gkernel",
        path.display(),
        DISK_SIZE / (1024 * 1024)
    );
}

/// Create a pflash image with magic "pfld" at offset 0 (for NPF passthrough test).
fn create_pflash_image(root: &Path, arch: &str) -> PathBuf {
    let size: usize = match arch {
        "riscv64" => 32 * 1024 * 1024, // 32MB - QEMU virt pflash1
        "aarch64" => 64 * 1024 * 1024, // 64MB - QEMU virt pflash1
        _ => 4 * 1024 * 1024,
    };

    let pflash_path = root.join("target").join(format!("pflash-{arch}.img"));
    let mut image = vec![0xFFu8; size];

    // Write magic "pfld" at offset 0 (consistent with h_2_0 format)
    image[0..4].copy_from_slice(b"pfld");

    std::fs::write(&pflash_path, &image).unwrap_or_else(|e| {
        eprintln!("Error: failed to write pflash image: {}", e);
        process::exit(1);
    });
    println!(
        "Created pflash image: {} ({} bytes)",
        pflash_path.display(),
        size
    );
    pflash_path
}

/// Build the hypervisor kernel.
fn do_build(root: &Path, info: &ArchInfo) {
    let manifest = root.join("Cargo.toml");
    let axconfig_path = root.join(".axconfig.toml");
    let status = Command::new("cargo")
        .env("AX_CONFIG_PATH", axconfig_path.to_str().unwrap())
        .args([
            "build",
            "--release",
            "--target",
            info.target,
            "--features",
            "hypervisor",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
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
fn do_run_qemu(arch: &str, elf: &Path, bin: &Path, disk: &Path, pflash: Option<&Path>) {
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
            // Attach pflash1 for pflash NPF test
            if let Some(pf) = pflash {
                args.extend([
                    "-drive".into(),
                    format!(
                        "if=pflash,format=raw,unit=1,file={},readonly=on",
                        pf.display()
                    ),
                ]);
            }
        }
        "aarch64" => {
            args.extend([
                "-cpu".into(),
                "max".into(),
                "-machine".into(),
                "virt,virtualization=on".into(),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
            ]);
            // Attach pflash1 for pflash NPF test (mapped at 0x04000000 on virt)
            if let Some(pf) = pflash {
                args.extend([
                    "-drive".into(),
                    format!(
                        "if=pflash,format=raw,unit=1,file={},readonly=on",
                        pf.display()
                    ),
                ]);
            }
        }
        "x86_64" => {
            args.extend([
                "-machine".into(),
                "q35".into(),
                "-cpu".into(),
                "EPYC".into(),
                "-kernel".into(),
                elf.to_str().unwrap().into(),
            ]);
        }
        _ => unreachable!(),
    }

    // VirtIO block device (for disk image containing guest payload)
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
            install_payload_config(&root, arch);
            let _payload = build_payload(&root, &info, arch);
            do_build(&root, &info);
            println!("Build complete for {arch} ({})", info.target);
        }
        Cmd::Run { ref arch } => {
            let info = arch_info(arch);
            install_config(&root, arch);

            // 1. Install payload config and build payload (gkernel/readpflash)
            install_payload_config(&root, arch);
            let payload_bin = build_payload(&root, &info, arch);

            // 2. Create disk image with payload
            let disk = root.join("target").join(format!("disk-{arch}.img"));
            create_fat_disk_image(&disk, &payload_bin);

            // 3. Create pflash image (for riscv64/aarch64 NPF passthrough test)
            let pflash = if arch == "riscv64" || arch == "aarch64" {
                Some(create_pflash_image(&root, arch))
            } else {
                None
            };

            // 4. Build hypervisor kernel
            do_build(&root, &info);

            let elf = root
                .join("target")
                .join(info.target)
                .join("release")
                .join("arceos-guestaspace");
            let bin = elf.with_extension("bin");

            if arch != "x86_64" {
                do_objcopy(&elf, &bin, info.objcopy_arch);
            }

            // 5. Run QEMU
            do_run_qemu(arch, &elf, &bin, &disk, pflash.as_deref());
        }
    }
}
