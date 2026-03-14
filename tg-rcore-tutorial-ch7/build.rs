use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::PathBuf, process::Command};
use tg_easy_fs::{BlockDevice, EasyFileSystem};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";
const BLOCK_SZ: usize = 512;

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
    println!("cargo:rerun-if-env-changed=TG_USER_VERSION");
    println!("cargo:rerun-if-env-changed=TG_USER_CRATE");
    println!("cargo:rerun-if-env-changed=TG_USER_LOCAL_DIR");
    println!("cargo:rerun-if-env-changed=TG_SKIP_USER_APPS");

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // 只在 RISC-V64 架构上使用链接脚本
    if target_arch == "riscv64" {
        write_linker();
        if should_skip_build_apps() {
            return;
        }
        build_apps_and_pack_fs();
    }
}

fn should_skip_build_apps() -> bool {
    if env::var_os("TG_SKIP_USER_APPS").is_some() {
        return true;
    }

    is_packaged_build()
}

fn write_linker() {
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, tg_linker::NOBIOS_SCRIPT).unwrap_or_else(|err| {
        panic!("failed to write linker script to {}: {}", ld.display(), err)
    });
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

fn is_packaged_build() -> bool {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_dir = out_dir.to_string_lossy();

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let manifest_dir = manifest_dir.to_string_lossy();

    out_dir.contains("/target/package/")
        || out_dir.contains("\\target\\package\\")
        || manifest_dir.contains("/target/package/")
        || manifest_dir.contains("\\target\\package\\")
}

fn build_apps_and_pack_fs() {
    let tg_user_root = ensure_tg_user();
    let cases_path = tg_user_root.join("cases.toml");
    println!("cargo:rerun-if-changed={}", cases_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        tg_user_root.join("Cargo.toml").display()
    );
    println!("cargo:rerun-if-changed={}", tg_user_root.join("src").display());

    let cfg = fs::read_to_string(&cases_path).unwrap_or_else(|err| {
        panic!("failed to read cases.toml from {}: {}", cases_path.display(), err)
    });
    let mut cases_map: HashMap<String, Cases> = toml::from_str(&cfg).unwrap_or_else(|err| {
        panic!("failed to parse cases.toml: {err}")
    });

    let cases = cases_map.remove("ch7").unwrap_or_default();
    let base = cases.base.unwrap_or(0);
    let step = cases.step.unwrap_or(0);
    let names = cases.cases.unwrap_or_default();

    if names.is_empty() {
        panic!("no user cases found for ch7 in {}", cases_path.display());
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fs_target_dir = manifest_dir
        .join("target")
        .join(TARGET_ARCH)
        .join("debug");
    let app_target_dir = tg_user_root
        .join("target")
        .join(TARGET_ARCH)
        .join("debug");

    for (i, name) in names.iter().enumerate() {
        let base_address = base + i as u64 * step;
        build_user_app(&tg_user_root, name, base_address);
    }

    easy_fs_pack(&names, &app_target_dir, &fs_target_dir).unwrap_or_else(|err| {
        panic!(
            "failed to pack easy-fs image in {}: {err}",
            fs_target_dir.display()
        )
    });
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

struct BlockFile(std::sync::Mutex<std::fs::File>);

impl BlockDevice for BlockFile {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = self.0.lock().unwrap();
        file.seek(SeekFrom::Start((block_id * BLOCK_SZ) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SZ, "Not a complete block!");
    }
}

fn easy_fs_pack(
    cases: &[String],
    app_target: &PathBuf,
    fs_target: &PathBuf,
) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::sync::Arc;

    fs::create_dir_all(fs_target)?;
    let fs_file = fs_target.join("fs.img");
    println!("cargo:rerun-if-changed={}", fs_file.display());
    let block_file = Arc::new(BlockFile(std::sync::Mutex::new({
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(fs_file)?;
        f.set_len(64 * 2048 * BLOCK_SZ as u64).unwrap();
        f
    })));

    let efs = EasyFileSystem::create(block_file, 64 * 2048, 1);
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));

    for case in cases {
        let mut host_file = std::fs::File::open(app_target.join(case)).unwrap();
        let mut all_data: Vec<u8> = Vec::new();
        host_file.read_to_end(&mut all_data).unwrap();
        let inode = root_inode.create(case.as_str()).unwrap();
        inode.write_at(0, all_data.as_slice());
    }

    Ok(())
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
        fs::write(&cargo_toml, format!("{}
[workspace]
", content))
            .unwrap_or_else(|err| panic!("failed to patch Cargo.toml in {}: {}", dir.display(), err));
    }
}