# tg-arceos-tutorial

`tg-arceos-tutorial` 是一个集合 crate，用于把与`arceos`相关的 `app-*` 教学 crate 的源码打包到一个压缩包里，便于通过 `cargo clone` 后离线解包恢复完整目录。

## 准备

```bash
cargo clone tg-arceos-tutorial # 或 git@github.com:rcore-os/tg-arceos-tutorial.git
cd tg-arceos-tutorial
bash scripts/extract_crates.sh
```

解包后会在当前目录生成以下 15 个 crate 目录：
### unikernel
- `app-helloworld` : https://github.com/arceos-org/app-helloworld
- `app-collections` : https://github.com/arceos-org/app-collections
- `app-readpflash` : https://github.com/arceos-org/app-readpflash
- `app-childtask` : https://github.com/arceos-org/app-childtask
- `app-msgqueue` : https://github.com/arceos-org/app-msgqueue
- `app-fairsched` : https://github.com/arceos-org/app-fairsched
- `app-readblk` : https://github.com/arceos-org/app-readblk
- `app-loadapp` : https://github.com/arceos-org/app-loadapp
### monolithic kernel
- `app-userprivilege` : https://github.com/arceos-org/app-userprivilege
- `app-lazymapping` : https://github.com/arceos-org/app-lazymapping
- `app-runlinuxapp` : https://github.com/arceos-org/app-runlinuxapp
### hypervisor
- `app-guestmode` : https://github.com/arceos-org/app-guestmode
- `app-guestaspace` : https://github.com/arceos-org/app-guestaspace
- `app-guestvdev` : https://github.com/arceos-org/app-guestvdev
- `app-guestmonolithickernel` : https://github.com/arceos-org/app-guestmonolithickernel


## 运行 
进入任意 `app-*` 目录即可独立构建/运行，如：
```
cd tg-arceos-tutorial/app-helloworld
cargo xtask run # 或 cargo xtask run --arch=riscv64
cargo xtask run --arch=aarch64
cargo xtask run --arch=loongarch64
cargo xtask run --arch=x86_64
```

或在所有 `app-*` 目录中执行，
```
cd tg-arceos-tutorial/
./scripts/batch_exec.sh -c "cargo xtask run"
./scripts/batch_exec.sh -c "cargo xtask run --arch=aarch64"
./scripts/batch_exec.sh -c "cargo xtask run --arch=loongarch64"
./scripts/batch_exec.sh -c "cargo xtask run --arch=x86_64"
```

## 练习
有5 个练习需要完成

- app-helloworld
- app-collections
- app-msgqueue
- app-loadapp
- app-runlinuxapp

## 维护者：重新生成 bundle

在包含 `app-*` 目录的路径中执行：

```bash
cd tg-arceos-tutorial
SOURCE_ROOT=.. bash scripts/compress_crates.sh
```

将生成：

- `bundle/apps.tar.gz`

该压缩包会被 `Cargo.toml` 的 `include` 字段打包进发布产物。