use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// ArceOS guest-mode multi-architecture build & run tool
#[derive(Parser)]
#[command(name = "xtask", about = "Build and run arceos-guestmode on different architectures")]
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
        #[arg(long, default_value_t = false)]
        riscv64: bool,
        #[arg(long, default_value_t = false)]
        aarch64: bool,
        #[arg(long = "x86_64", visible_alias = "x86-64", default_value_t = false)]
        x86_64: bool,
        #[arg(long, default_value_t = false)]
        loongarch64: bool,
    },
    /// Build and run the kernel in QEMU
    Run {
        #[arg(long, default_value = "riscv64")]
        arch: String,
        #[arg(long, default_value_t = false)]
        riscv64: bool,
        #[arg(long, default_value_t = false)]
        aarch64: bool,
        #[arg(long = "x86_64", visible_alias = "x86-64", default_value_t = false)]
        x86_64: bool,
        #[arg(long, default_value_t = false)]
        loongarch64: bool,
    },
}

#[derive(Clone)]
struct ArchInfo {
    target: &'static str,
    objcopy_arch: &'static str,
}

fn arch_info(arch: &str) -> ArchInfo {
    match arch {
        "riscv64" | "reiscv64" => ArchInfo {
            target: "riscv64gc-unknown-none-elf",
            objcopy_arch: "riscv64",
        },
        "aarch64" => ArchInfo {
            target: "aarch64-unknown-none-softfloat",
            objcopy_arch: "aarch64",
        },
        "x86_64" => ArchInfo {
            target: "x86_64-unknown-none",
            objcopy_arch: "x86_64",
        },
        "loongarch64" => ArchInfo {
            target: "loongarch64-unknown-none",
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

fn resolve_arch(
    arch: &str,
    riscv64: bool,
    aarch64: bool,
    x86_64: bool,
    loongarch64: bool,
) -> String {
    let mut selected = Vec::new();
    if riscv64 {
        selected.push("riscv64");
    }
    if aarch64 {
        selected.push("aarch64");
    }
    if x86_64 {
        selected.push("x86_64");
    }
    if loongarch64 {
        selected.push("loongarch64");
    }

    if selected.len() > 1 {
        eprintln!(
            "Error: architecture flags are mutually exclusive, got: {}",
            selected.join(", ")
        );
        process::exit(2);
    }
    if let Some(one) = selected.first() {
        return (*one).to_string();
    }
    if arch == "reiscv64" {
        return "riscv64".to_string();
    }
    arch.to_string()
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

/// Build the guest payload (skernel) for the target architecture.
fn build_payload(root: &Path, info: &ArchInfo) -> PathBuf {
    let manifest = root.join("Cargo.toml");

    println!("Building payload (skernel) ...");

    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--manifest-path",
            manifest.to_str().unwrap(),
            "--bin",
            "skernel",
            "--features",
            "guest-kernel",
            "--target",
            info.target,
        ])
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
        .join("skernel");

    // We need a flat binary for the guest loading?
    // h_1_0 uses `load_vm_image`. `loader::load_vm_image` in h_1_0 likely parses ELF or loads binary.
    // The h_1_0 source calls `load_vm_image("/sbin/skernel")`.
    // The `make payload` in h_1_0 does `rust-objcopy ... -O binary`.
    // So `skernel` on disk image should be a binary file.
    
    let payload_bin = payload_elf.with_extension("bin");
    
    let status = Command::new("rust-objcopy")
        .args([
            &format!("--binary-architecture={}", info.objcopy_arch),
            "--only-section=.text",
            payload_elf.to_str().unwrap(),
            "--strip-all",
            "-O",
            "binary",
            payload_bin.to_str().unwrap(),
        ])
        .status()
        .expect("failed to execute rust-objcopy for payload");

    if !status.success() {
        eprintln!("Error: rust-objcopy for payload failed");
        process::exit(status.code().unwrap_or(1));
    }

    println!("Payload built: {}", payload_bin.display());
    payload_bin
}

/// Create a 64MB FAT32 disk image containing `/sbin/skernel`.
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
    println!("Payload binary size: {} bytes", payload_data.len());

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

        // Write payload as /sbin/skernel
        let mut f = root_dir.create_file("sbin/skernel").unwrap_or_else(|e| {
            eprintln!("Error: failed to create /sbin/skernel: {}", e);
            process::exit(1);
        });
        f.write_all(&payload_data).unwrap();
        f.flush().unwrap();
    }

    println!(
        "Created FAT32 disk image: {} ({}MB) with /sbin/skernel",
        path.display(),
        DISK_SIZE / (1024 * 1024)
    );
}

/// Create a pflash image containing "pfld" magic at offset 0.
fn create_pflash_image(path: &Path, arch: &str) {
    let size = match arch {
        "aarch64" => 64 * 1024 * 1024, // 64MB for virt.flash1
        _ => 32 * 1024 * 1024,         // 32MB default (riscv64)
    };
    const PFLASH_MAGIC: &[u8] = b"pfld";

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create pflash image: {}", e);
            process::exit(1);
        });
    file.set_len(size).unwrap();

    let mut writer = std::io::BufWriter::new(&file);
    writer.write_all(PFLASH_MAGIC).unwrap();
    writer.flush().unwrap();

    println!(
        "Created pflash image: {} ({} bytes)",
        path.display(),
        size
    );
}

/// Build the kernel.
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
            "axstd",
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
fn do_run_qemu(arch: &str, elf: &Path, bin: &Path, disk: &Path, pflash: &Path) {
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
                "-drive".into(),
                format!(
                    "if=pflash,format=raw,unit=1,file={},readonly=on",
                    pflash.display()
                ),
            ]);
        }
        "aarch64" => {
            args.extend([
                "-cpu".into(),
                "max".into(),
                "-machine".into(),
                "virt,virtualization=on".into(),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
                "-drive".into(),
                format!(
                    "if=pflash,format=raw,unit=1,file={},readonly=on",
                    pflash.display()
                ),
            ]);
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
        Cmd::Build {
            ref arch,
            riscv64,
            aarch64,
            x86_64,
            loongarch64,
        } => {
            let arch = resolve_arch(arch, riscv64, aarch64, x86_64, loongarch64);
            let info = arch_info(&arch);
            install_config(&root, &arch);
            let _payload = build_payload(&root, &info);
            do_build(&root, &info);
            println!("Build complete for {arch} ({})", info.target);
        }
        Cmd::Run {
            ref arch,
            riscv64,
            aarch64,
            x86_64,
            loongarch64,
        } => {
            let arch = resolve_arch(arch, riscv64, aarch64, x86_64, loongarch64);
            let info = arch_info(&arch);
            install_config(&root, &arch);

            // 1. Build payload (skernel)
            let payload_bin = build_payload(&root, &info);

            // 2. Create disk image with payload
            let disk = root.join("target").join(format!("disk-{arch}.img"));
            create_fat_disk_image(&disk, &payload_bin);

            // 3. Create pflash image
            let pflash = root.join("target").join(format!("pflash-{arch}.img"));
            create_pflash_image(&pflash, &arch);

            // 4. Build kernel
            do_build(&root, &info);

            let elf = root
                .join("target")
                .join(info.target)
                .join("release")
                .join("arceos-guestmode");
            let bin = elf.with_extension("bin");

            if arch != "x86_64" {
                do_objcopy(&elf, &bin, info.objcopy_arch);
            }

            do_run_qemu(&arch, &elf, &bin, &disk, &pflash);
        }
    }
}
