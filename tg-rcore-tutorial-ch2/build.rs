use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::PathBuf, process::Command};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";

// 以下三项均从 .cargo/config.toml [env] 节读取，不再硬编码：
//   TG_USER_CRATE     — crates.io 包名
//   TG_USER_LOCAL_DIR — 本地缓存目录名
//   TG_USER_VERSION   — 版本号

#[derive(Deserialize, Default)]
struct Cases {
    base: Option<u64>,
    step: Option<u64>,
    cases: Option<Vec<String>>,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    println!("cargo:rerun-if-env-changed=TG_USER_DIR");
    println!("cargo:rerun-if-env-changed=TG_USER_CRATE");
    println!("cargo:rerun-if-env-changed=TG_USER_LOCAL_DIR");
    println!("cargo:rerun-if-env-changed=TG_USER_VERSION");

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // 只在 RISC-V64 架构上使用链接脚本
    if target_arch == "riscv64" {
        write_linker();
        if should_skip_build_apps() {
            write_dummy_app_asm();
        } else {
            build_apps();
        }
    }
}

fn should_skip_build_apps() -> bool {
    if env::var_os("TG_SKIP_USER_APPS").is_some() {
        return true;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let manifest_dir = manifest_dir.to_string_lossy();
    manifest_dir.contains("/target/package/") || manifest_dir.contains("\\target\\package\\")
}

fn write_linker() {
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, tg_linker::NOBIOS_SCRIPT).unwrap_or_else(|err| {
        panic!("failed to write linker script to {}: {}", ld.display(), err)
    });
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

fn build_apps() {
    let tg_user_root = ensure_tg_user();
    let cases_path = tg_user_root.join("cases.toml");
    println!("cargo:rerun-if-changed={}", cases_path.display());
    println!("cargo:rerun-if-changed={}", tg_user_root.join("Cargo.toml").display());
    println!("cargo:rerun-if-changed={}", tg_user_root.join("src").display());

    let cfg = fs::read_to_string(&cases_path).unwrap_or_else(|err| {
        panic!("failed to read cases.toml from {}: {}", cases_path.display(), err)
    });
    let mut cases_map: HashMap<String, Cases> = toml::from_str(&cfg).unwrap_or_else(|err| {
        panic!("failed to parse cases.toml: {err}")
    });

    let cases = cases_map.remove("ch2").unwrap_or_default();
    let base = cases.base.unwrap_or(0);
    let step = cases.step.unwrap_or(0);
    let names = cases.cases.unwrap_or_default();

    if names.is_empty() {
        panic!("no user cases found for ch2 in {}", cases_path.display());
    }

    let target_dir = tg_user_root.join("target").join(TARGET_ARCH).join("debug");
    let mut bins: Vec<PathBuf> = Vec::with_capacity(names.len());

    for (i, name) in names.iter().enumerate() {
        let base_address = base + i as u64 * step;
        build_user_app(&tg_user_root, name, base_address);
        let elf = target_dir.join(name);
        let app_path = if base_address != 0 {
            objcopy_to_bin(&elf)
        } else {
            elf
        };
        bins.push(app_path);
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let app_asm = out_dir.join("app.asm");
    write_app_asm(&app_asm, base, step, &bins);
    println!("cargo:rustc-env=APP_ASM={}", app_asm.display());
}

fn build_user_app(tg_user_root: &PathBuf, name: &str, base_address: u64) {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "--manifest-path",
        tg_user_root.join("Cargo.toml").to_string_lossy().as_ref(),
        "--bin",
        name,
        "--target",
        TARGET_ARCH,
    ]);

    if base_address != 0 {
        cmd.env("BASE_ADDRESS", base_address.to_string());
    }

    let status = cmd.status().expect("failed to execute cargo build for user app");
    if !status.success() {
        panic!("failed to build user app {name}");
    }
}

fn objcopy_to_bin(elf: &PathBuf) -> PathBuf {
    let bin = elf.with_extension("bin");
    let status = Command::new("rust-objcopy")
        .args([
            elf.to_string_lossy().as_ref(),
            "--strip-all",
            "-O",
            "binary",
            bin.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to execute rust-objcopy");
    if !status.success() {
        panic!("rust-objcopy failed for {}", elf.display());
    }
    bin
}

fn write_app_asm(path: &PathBuf, base: u64, step: u64, bins: &[PathBuf]) {
    use std::io::Write;
    let mut asm = fs::File::create(path)
        .unwrap_or_else(|err| panic!("failed to create {}: {}", path.display(), err));

    writeln!(
        asm,
        "\
.global apps
.section .data
.align 3
apps:
    .quad {base:#x}
    .quad {step:#x}
    .quad {}",
        bins.len(),
    )
    .unwrap();

    for i in 0..bins.len() {
        writeln!(asm, "    .quad app_{i}_start").unwrap();
    }

    writeln!(asm, "    .quad app_{}_end", bins.len() - 1).unwrap();

    for (i, path) in bins.iter().enumerate() {
        writeln!(
            asm,
            "\
app_{i}_start:
    .incbin {path:?}
app_{i}_end:",
        )
        .unwrap();
    }
}

fn write_dummy_app_asm() {
    use std::io::Write;

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let app_asm = out_dir.join("app.asm");
    let mut asm = fs::File::create(&app_asm)
        .unwrap_or_else(|err| panic!("failed to create {}: {}", app_asm.display(), err));

    writeln!(
        asm,
        "\
.global apps
.section .data
.align 3
apps:
    .quad 0
    .quad 0
    .quad 0
    .quad 0"
    )
    .unwrap();

    println!("cargo:rustc-env=APP_ASM={}", app_asm.display());
}

fn ensure_tg_user() -> PathBuf {
    // 优先使用 TG_USER_DIR 显式指定的目录
    if let Ok(dir) = env::var("TG_USER_DIR") {
        let path = PathBuf::from(dir);
        if path.join("Cargo.toml").exists() {
            return path;
        }
    }

    // 从 .cargo/config.toml [env] 读取三个配置项
    let crate_name = env::var("TG_USER_CRATE")
        .expect("TG_USER_CRATE not set; add it to .cargo/config.toml [env]");
    let local_dir_name = env::var("TG_USER_LOCAL_DIR")
        .expect("TG_USER_LOCAL_DIR not set; add it to .cargo/config.toml [env]");
    let version = env::var("TG_USER_VERSION")
        .expect("TG_USER_VERSION not set; add it to .cargo/config.toml [env]");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tg_user_dir = manifest_dir.join(&local_dir_name);

    // 本地缓存目录已存在则直接使用
    if tg_user_dir.join("Cargo.toml").exists() {
        ensure_workspace_table(&tg_user_dir);
        return tg_user_dir;
    }

    // 从 crates.io 克隆指定包
    let crate_spec = format!("{crate_name}@{version}");
    let status = Command::new("cargo")
        .args([
            "clone",
            crate_spec.as_str(),
            "--",
            tg_user_dir.to_string_lossy().as_ref(),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute cargo clone {crate_spec}: {e}"));

    if !status.success() {
        panic!(
            "failed to clone {crate_spec} into {}; ensure cargo-clone is installed or set TG_USER_DIR",
            tg_user_dir.display()
        );
    }

    if !tg_user_dir.join("Cargo.toml").exists() {
        panic!(
            "{crate_spec} clone did not produce a valid crate at {}",
            tg_user_dir.display()
        );
    }

    // 克隆后补加 [workspace]，防止父 workspace 将其识别为非成员而报错
    ensure_workspace_table(&tg_user_dir);

    tg_user_dir
}

/// 若 Cargo.toml 末尾尚无 [workspace] 表，则追加一个空的，
/// 使该 crate 成为独立 workspace 根，避免父 workspace 冲突。
fn ensure_workspace_table(dir: &PathBuf) {
    let cargo_toml = dir.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).unwrap_or_default();
    if !content.contains("[workspace]") {
        fs::write(&cargo_toml, format!("{}\n[workspace]\n", content))
            .unwrap_or_else(|err| panic!("failed to patch Cargo.toml in {}: {}", dir.display(), err));
    }
}
