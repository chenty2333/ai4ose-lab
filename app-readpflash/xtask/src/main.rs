use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// ArceOS readpflash multi-architecture build & run tool
#[derive(Parser)]
#[command(name = "xtask", about = "Build and run arceos-readpflash on different architectures")]
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

/// Find SeaBIOS binary on the system (needed for x86_64 pflash).
fn find_seabios() -> PathBuf {
    let candidates = [
        "/usr/share/qemu/bios-256k.bin",
        "/usr/share/seabios/bios-256k.bin",
        "/usr/local/share/qemu/bios-256k.bin",
        "/usr/share/qemu/bios.bin",
        "/usr/share/seabios/bios.bin",
    ];
    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    eprintln!("Error: Could not find SeaBIOS binary for x86_64 pflash.");
    eprintln!("Looked in:");
    for p in &candidates {
        eprintln!("  - {p}");
    }
    eprintln!("Install with: sudo apt install seabios  (or equivalent)");
    process::exit(1);
}

/// Returns the required PFlash image size for each architecture.
///
/// QEMU virt machines have fixed pflash bank sizes that must be matched exactly:
/// - riscv64 virt: pflash0/1 each 32MB
/// - aarch64 virt: pflash0/1 each 64MB
/// - x86_64 q35:   pflash0 size is flexible (we use 4MB)
/// - loongarch64:  pflash0 size is flexible (we use 4MB)
fn pflash_size(arch: &str) -> usize {
    match arch {
        "riscv64" => 32 * 1024 * 1024,     // 32MB - fixed by QEMU virt machine
        "aarch64" => 64 * 1024 * 1024,     // 64MB - fixed by QEMU virt machine
        "x86_64" => 4 * 1024 * 1024,       // 4MB
        "loongarch64" => 4 * 1024 * 1024,  // 4MB
        _ => 4 * 1024 * 1024,
    }
}

/// Create a PFlash image with magic string "PFLA" at offset 0.
///
/// For x86_64, the image also includes SeaBIOS at the end so that
/// pflash0 can serve as both data storage and boot ROM.
fn create_pflash_image(root: &Path, arch: &str) -> PathBuf {
    let size = pflash_size(arch);
    let pflash_path = root.join("pflash.img");
    let mut image = vec![0xFFu8; size]; // CFI flash erased state is 0xFF

    // Write magic "PFLA" at offset 0
    image[0..4].copy_from_slice(b"PFLA");

    if arch == "x86_64" {
        // For x86_64 Q35: pflash0 replaces the BIOS ROM.
        // We embed SeaBIOS at the end of the image so the CPU reset
        // vector (0xFFFFFFF0) lands inside SeaBIOS code.
        let bios_path = find_seabios();
        let bios_data = std::fs::read(&bios_path).unwrap_or_else(|e| {
            eprintln!("Error: failed to read SeaBIOS binary: {}", e);
            process::exit(1);
        });
        let bios_size = bios_data.len();
        assert!(
            bios_size <= size - 4,
            "SeaBIOS binary ({bios_size} bytes) too large for {size}-byte pflash image"
        );
        println!(
            "Embedding SeaBIOS ({} bytes) from {}",
            bios_size,
            bios_path.display()
        );
        image[size - bios_size..].copy_from_slice(&bios_data);
    }

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

/// Run the kernel image in QEMU with PFlash attached.
fn do_run_qemu(arch: &str, elf: &Path, bin: &Path, pflash: &Path) {
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
            // pflash1 at 0x22000000 (pflash0 is for firmware)
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
            // pflash1 at 0x04000000 (pflash0 is for firmware)
            args.extend([
                "-cpu".into(),
                "cortex-a72".into(),
                "-machine".into(),
                "virt".into(),
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
            // pflash0 at 4GB-4MB = 0xFFC00000 (combined SeaBIOS + data)
            args.extend([
                "-machine".into(),
                "q35".into(),
                "-drive".into(),
                format!(
                    "if=pflash,format=raw,unit=0,file={},readonly=on",
                    pflash.display()
                ),
                "-kernel".into(),
                elf.to_str().unwrap().into(),
            ]);
        }
        "loongarch64" => {
            // pflash1 at 0x1d000000 (VIRT_FLASH region, pflash0 absent)
            // pflash0 is used for firmware, so we use pflash1 for data.
            // When pflash0 is not provided, pflash1 maps at the start of
            // the VIRT_FLASH region (0x1d000000).
            args.extend([
                "-machine".into(),
                "virt".into(),
                "-drive".into(),
                format!(
                    "if=pflash,format=raw,unit=1,file={},readonly=on",
                    pflash.display()
                ),
                "-kernel".into(),
                bin.to_str().unwrap().into(),
            ]);
        }
        _ => unreachable!(),
    }

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
                .join("arceos-readpflash");
            let bin = elf.with_extension("bin");

            // objcopy for non-x86_64 architectures
            if arch != "x86_64" {
                do_objcopy(&elf, &bin, info.objcopy_arch);
            }

            // Create pflash image with magic data
            let pflash = create_pflash_image(&root, arch);

            do_run_qemu(arch, &elf, &bin, &pflash);
        }
    }
}
