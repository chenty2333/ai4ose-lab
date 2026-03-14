# TanGram-rCore-Tutorial

[![Crates.io](https://img.shields.io/crates/v/tg-rcore-tutorial.svg)](https://crates.io/crates/tg-rcore-tutorial) [![Docs.rs](https://docs.rs/tg-rcore-tutorial/badge.svg)](https://docs.rs/tg-rcore-tutorial) [![License](https://img.shields.io/crates/l/tg-rcore-tutorial.svg)](LICENSE)

## 0. 简介
面向操作系统课程教学与自学的组件化 Tangram rCore Tutorial 操作系统内核实验教程的汇总仓库。

本实验教程的目标：学生与AI充分合作，按章节学习操作系统内核的逐步演进过程，或按组件视角学习操作系统内核的总体架构和功能组成，从而能够把操作系统的原理概率与设计实现关联起来，并掌握操作系统的系统架构级设计能力。  

本实验教程的任务：请看`rcore-tutorial-ch[1-8]`中的`README.md`中的`## 练习任务`小节的内容，了解并完成基础级和扩展级的任务要求。对于能力强的同学，请看`docs\challenges.md`的内容，了解并完成挑战级的任务要求。

汇总仓库包含：

- `tg-rcore-tutorial-ch1~tg-rcore-tutorial-ch8`：tg-rcore-tutorial-ch[1-8] 8 个渐进章节（每章是一个可独立运行的内核 crate + 指导文档）
- `tg-*`：可复用内核组件 crate（内存、虚存、上下文、同步、信号、文件系统等）
- `tg-rcore-tutorial-user`：用户态测试程序集合
- `tg-rcore-tutorial-checker`：测试输出检测工具
- `tg-rcore-tutorial-linker`：为 ch1~ch8的rCore Tutorial教学操作系统内核提供链接脚本生成功能

**本实验教程是[AI4OSE（AI for Operating System Engineering）的Lab1](https://crates.io/crates/ai4ose-lab1-2026s)。**

### 练习任务
#### 基础实验
充分利用各类AI工具，完成位于 `tg-rcore-tutorial-ch{3/4/5/6/8}` 下所列的5个基础实验练习，夯实操作系统理论基础并提升实践能力，最终提交一份包含与AI合作的实现过程与学习收获的总结报告。  
- 与AI合作的实现过程（包括如何与AI交互，碰到的问题/bug、解决过程等）
- 学习效果评估（包括自己在这个学习过程中知识和能力的提升/下降评价，与本校现有教学实验教程的定量/定性的对比分析）

#### 改进教程
充分利用各类AI工具，基于本教学实验教程--`tg-rcore-tutorial`，结合自身兴趣与学习需求，进行改进、扩展、裁剪、重构，或者自己从零构建，形成自己的个性化教学实验教程。需提交一份设计总结报告，内容包括：  
   - 设计思路与目标（包括适合自己的学习方式描述，初步设想和规划等）
   - 与AI合作的实现过程（包括如何与AI交互，碰到的问题/bug、解决过程等）
   - 学习效果评估（包括自己在这个学习过程中知识和能力的提升/下降评价，与本校现有教学实验教程的定量/定性的对比分析）

#### 扩展实验
充分利用各类AI工具，设计实现`tg-rcore-tutorial-ch{1~8}`的游戏应用，并进一步扩展`tg-rcore-tutorial-ch{1~8}`内核，实现支持这些游戏应用的新内核，夯实操作系统理论基础并提升实践能力和创造能力，最终提交一份包含与AI合作的实现过程与学习收获的总结报告，内容包括：    
   - 与AI合作的实现过程（包括如何与AI交互，碰到的问题/bug、解决过程等）
   - 学习效果评估（包括自己在这个学习过程中知识和能力的提升/下降评价，与本校现有教学实验教程的定量/定性的对比分析）

**游戏应用和支持游戏的内核**简要描述如下（注：下面的`ch`是`tg-rcore-tutorial-ch`的简写）

- `ch1-tangram`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch1-tangram.png)，在ch1基础上扩展内核功能，静态显示七巧板“OS”图案，即基于 VirtIO-GPU 驱动操作 Framebuffer，将OS的代码中数组定义的七巧板“OS”图案像素数据直接渲染到帧缓冲，实现静态图片显示。
- `ch2-moving-tangram`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch2-moving-tangram.gif)，在ch2基础上扩展内核功能，动态分步显示七巧板“OS”图案，即通过多程序批处理方式，每个程序渲染一块，逐块渲染七巧板“OS”的 n 个组成部分，实现动态拼接的视觉效果。
- `ch3-snake`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch3-snake.gif)，实现用户态贪吃蛇游戏，支持**轮询式输入**和**中断式输入**两种控制方式；在ch3基础上扩展内核功能，支持运行用户态贪吃蛇游戏。
- `ch4-tetris`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch4-tetris.gif)，实现用户态单人俄罗斯方块游戏，支持方块旋转、行消除、计分、速度递增，无额外依赖；在ch4基础上扩展内核功能，支持运行用户态单人俄罗斯方块游戏。
- `ch5-pingpong`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch5-pingpong.gif)，实现多进程协作的用户态双人乒乓游戏，支持键盘控制、碰撞反弹、计分，学习 2D 碰撞与帧刷新；在ch5基础上扩展内核功能，支持运行多进程协作的用户态双人乒乓游戏。
- `ch6-breakout`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch6-breakout.gif)，实现用户态打砖块游戏，支持碰撞反弹、计分，及通过快捷键保存/恢复游戏进度；在ch6基础上扩展内核功能，支持运行用户态打砖块游戏。
- `ch7-pacman`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch7-pacman.gif)，实现简化版用户态吃豆人游戏，还原经典玩法核心；在ch7基础上扩展内核功能，支持运行简化版用户态吃豆人游戏。
- `ch8-doom`：[demo](https://github.com/rcore-os/tg-rcore-tutorial-game-demo/blob/main/ch8-doom.gif)，移植[用户态跨平台版 Doom 游戏](https://github.com/ozkl/doomgeneric)，通过 Framebuffer 实现 3D 软件渲染；在ch8基础上扩展内核功能，支持运行用户态跨平台版 Doom 游戏。

**完成后，并请与我们联系，分享你的实现过程与学习收获。想接受进一步的挑战，可访问[AI4OSE（AI for Operating System Engineering）的Lab2](https://crates.io/crates/ai4ose-lab2-2026s)。**

## 1. 如何开始

### 1.0 基于Web IDE方式的快速试用
不需要配置开发运行环境，只需有一个能上网的浏览器即可。
 - [教程国内网址](https://cnb.cool/LearningOS/tg-rcore-tutorial/-/tree/test) 
   - 阅读[豆包提供的基于 cnb 的Web IDE 实践 tg-rcore-tutorial 简易指导书](https://www.doubao.com/thread/w236fa7686eb1d316)并按其提示操作
 - [教程国外网址](https://github.com/LearningOS/tg-rcore-tutorial/tree/test)
   - 阅读[豆包提供的基于 github 的codespaces Web IDE 实践 tg-rcore-tutorial 简易指导书](https://www.doubao.com/thread/w8fbf39ac661d8907)并按其提示操作

### 1.1 环境要求

- Rust toolchain：本仓库使用 `stable`（见 `rust-toolchain.toml`）
- 目标架构：`riscv64gc-unknown-none-elf`
- 组件：`rust-src`、`llvm-tools-preview`（`rust-toolchain.toml` 已声明）
- QEMU：`qemu-system-riscv64`（建议 >= 7.0）
- 推荐工具：`cargo-binutils`、`cargo-clone`

### 1.2 获取代码

#### 方式 A：直接获取完整实验仓库（推荐）
```bash
git clone --recurse-submodules  https://github.com/rcore-os/tg-rcore-tutorial.git  #注：由于cnb.cool不支持git协议的clone，所以在cnb.cool开发环境中不宜采用这种方式
cd tg-rcore-tutorial
# 缺省在test分支，可通过下面的命令确保在test分支
git checkout test #切换到test分支
```
补充：没有采用"--recurse-submodules" 进行`git clone` 后，拉取各个submodule的步骤（适合cnb.cool开发环境）
```bash
git clone https://github.com/rcore-os/tg-rcore-tutorial.git
cd tg-rcore-tutorial
# 缺省在test分支，可通过下面的命令确保在test分支
git checkout test #切换到test分支
# 初始化并拉取所有子模块代码
#注：在cnb.cool开发环境中，由于git协议不支持submodule，所以需要切换到https协议，需要执行如下命令
./scripts/switch-submodule-protocol.sh https
#注：如果开发环境支持git协议，则不需要切换到https协议，即不需要执行上面这条命令
git submodule init   # 初始化.gitmodules配置
git submodule update # 拉取子模块的指定版本代码
```

方式 B：通过 crates.io 集合包获取（内嵌压缩包）
先安装 cargo-clone

```bash
cargo install cargo-clone
```

再拉取集合包并解包完整工作区：

```bash
cargo clone tg-rcore-tutorial # 也可带具体版本号 @0.4.5-preview.2
cd tg-rcore-tutorial
bash scripts/extract_submodules.sh
cd workspace-full/tg-rcore-tutorial
```

解包后将得到完整教学目录（包含 ch1~ch8、tg-*、tg-user、tg-checker）。

#### 获取某个操作系统内核或内核功能组件（单独 crate）
```bash
cargo clone tg-rcore-tutorial-ch3  #tg-rcore-tutorial-chX 是发布到 crates.io上的组件化内核， X=1..8 代表8个内核 
cd tg-rcore-tutorial-ch3  # 进入 tg-rcore-tutorial-ch3 内核
```

### 1.3 最短上手路径（建议）

```bash
cd tg-rcore-tutorial-ch3
cargo run
```

如果你想直接做基本功能测试（例如 ch3）：
```bash
# 先安装tg-rcore-tutorial-checker`测试输出检测工具
cargo install tg-rcore-tutorial-checker
# 开始测试
cd tg-rcore-tutorial-ch3
cargo build   
./test.sh base
```

如果你想直接做练习章（例如 ch3）：

```bash
cd tg-rcore-tutorial-ch3
cargo build --features exercise
./test.sh exercise
```

## 2. 仓库结构总览

| 路径 | 作用 | 你通常在什么时候用 |
|---|---|---|
| `tg-rcore-tutorial-ch[1-8]` | 8个章节的内核 + 实验指导 | 按课程顺序学习、做章节实验 |
| `tg-rcore-tutorial-console` | 控制台输出与日志 | 需要统一日志/输出接口 |
| `tg-rcore-tutorial-linker` | 链接脚本生成工具 | 构建内核镜像、管理链接符号 |
| `tg-rcore-tutorial-sbi` | SBI 封装（含 `nobios` 支持） | 与固件/定时器/关机交互 |
| `tg-rcore-tutorial-syscall` | syscall 编号与 trait 框架 | 定义/实现系统调用 |
| `tg-rcore-tutorial-kernel-context` | 上下文切换与执行上下文 | Trap、任务/线程切换 |
| `tg-rcore-tutorial-kernel-alloc` | 内核内存分配器 | 需要 `#[global_allocator]` 时 |
| `tg-rcore-tutorial-kernel-vm` | 地址空间与页表管理 | ch4+ 虚存、映射、权限检查 |
| `tg-rcore-tutorial-task-manage` | 任务/进程/线程管理抽象 | 调度器与任务关系管理 |
| `tg-rcore-tutorial-easy-fs` | 教学文件系统实现 | ch6+ 文件、目录、pipe |
| `tg-rcore-tutorial-sync` | 同步原语（mutex/semaphore/condvar） | ch8 并发同步 |
| `tg-rcore-tutorial-signal-defs` | 信号号与结构定义 | ch7+ 信号语义定义 |
| `tg-rcore-tutorial-signal` | 信号处理 trait 抽象 | 信号框架扩展点 |
| `tg-rcore-tutorial-signal-impl` | 信号处理具体实现 | 直接复用信号实现 |
| `tg-rcore-tutorial-user` | 用户程序与测试用例 | 内核构建期打包/拉取用户态测例 |
| `tg-rcore-tutorial-checker` | 输出匹配检测工具 | 自动判定章节测试是否通过 |

一共 23 个 crates ，分为 4 层：

### Layer 0: 基础组件 (10个，无内部依赖)
- sbi , linker , console , kernel-context , kernel-alloc , kernel-vm , easy-fs , signal-defs , task-manage , checker
### Layer 1: 中间组件 (3个)
- syscall → 依赖 signal-defs
- signal → 依赖 kernel-context , signal-defs
- sync → 依赖 task-manage
### Layer 2: 上层组件 (2个)
- signal-impl → 依赖 kernel-context , signal
- user → 依赖 console , syscall
### Layer 3: 内核章节 (8个)
- ch1 → 仅依赖 sbi
- ch2 , ch3 → 依赖 5 个基础组件
- ch4 → 依赖 7 个组件（增加内存管理）
- ch5 → 依赖 8 个组件（增加进程管理）
- ch6 → 依赖 9 个组件（增加文件系统）
- ch7 → 依赖 11 个组件（增加信号处理）
- ch8 → 依赖 12 个组件（增加同步原语）

可进一步查看更详细的[**mermaid格式crates依赖关系分析图**](./deps-mermaid.md)和[**ascii格式crates依赖关系分析图**](./deps-ascii.md)

## 3. 章节与练习地图

| 章节 | 主题 | 默认运行 | 练习模式 |
|---|---|---|---|
| `tg-rcore-tutorial-ch1` | 裸机与最小执行环境 | `cargo run` | 无独立 exercise |
| `tg-rcore-tutorial-ch2` | Batch OS、Trap、基本 syscall | `cargo run` | 无独立 exercise |
| `tg-rcore-tutorial-ch3` | 多道程序与分时 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch4` | 地址空间与页表 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch5` | 进程与调度 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch6` | 文件系统 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch7` | IPC（pipe/signal） | `cargo run` | 基础测试为主 |
| `tg-rcore-tutorial-ch8` | 线程与并发同步 | `cargo run` | `cargo run --features exercise` |

5 个常见练习章：`tg-rcore-tutorial-ch[34568]`。

<a id="chapters-source-nav-map"></a>

### 3.1 tg-rcore-tutorial-ch[1-8] 源码导航总表（配套注释版）

下面这张表用于跨章节快速定位源码阅读入口；每章 README 里还有更细的“源码阅读导航索引”。

| 章节（点击直达导航） | 建议先读的源码文件（顺序） | 关注主线 |
|---|---|---|
| [`ch1`](tg-rcore-tutorial-ch1/README.md#source-nav) | `src/main.rs` | 裸机最小启动：`_start -> rust_main -> panic` |
| [`ch2`](tg-rcore-tutorial-ch2/README.md#source-nav) | `src/main.rs` | 批处理 + Trap + syscall 分发 |
| [`ch3`](tg-rcore-tutorial-ch3/README.md#source-nav) | `src/task.rs` -> `src/main.rs` | 任务模型 + 抢占/协作调度 |
| [`ch4`](tg-rcore-tutorial-ch4/README.md#source-nav) | `src/main.rs` -> `src/process.rs` | 页表与地址空间 + `translate` |
| [`ch5`](tg-rcore-tutorial-ch5/README.md#source-nav) | `src/process.rs` -> `src/processor.rs` -> `src/main.rs` | `fork/exec/wait` 与进程关系管理 |
| [`ch6`](tg-rcore-tutorial-ch6/README.md#source-nav) | `src/virtio_block.rs` -> `src/fs.rs` -> `src/main.rs` | 块设备到文件系统，再到 fd 系统调用 |
| [`ch7`](tg-rcore-tutorial-ch7/README.md#source-nav) | `src/fs.rs` -> `src/process.rs` -> `src/main.rs` | 管道统一 fd 抽象 + 信号处理 |
| [`ch8`](tg-rcore-tutorial-ch8/README.md#source-nav) | `src/process.rs` -> `src/processor.rs` -> `src/main.rs` | 线程化调度 + 同步原语阻塞/唤醒 |

## 4. 常用开发与测试流程

### 4.1 进入某章开发

```bash
cd tg-rcore-tutorial-ch<N>
cargo build
cargo run
```

### 4.2 运行章节测试脚本

```bash
./test.sh          # 默认（通常等价于 all 或 base）
./test.sh base     # 基础测试
./test.sh exercise # 练习测试（若该章支持）
./test.sh all      # 全量测试（若该章支持）
```

### 4.3 使用 `tg-rcore-tutorial-checker` 做输出检测

先安装（本地路径）：

```bash
cargo install --path tg-rcore-tutorial-checker
```

基础测试示例（以 tg-rcore-tutorial-ch2 为例）：

```bash
cargo run 2>&1 | tg-rcore-tutorial-checker --ch 2
```

练习测试示例（以 ch3 为例）：

```bash
cargo run --features exercise 2>&1 | tg-rcore-tutorial-checker --ch 3 --exercise
```

## 5. `tg-*` 内核功能组件开发工作流（开发者重点）

### 5.1 本地修改组件后如何验证

建议流程：

1. 在内核功能组件的目录先验证组件本身：
   ```bash
   cd tg-rcore-tutorial-sbi
   cargo check 
   ```
2. 测试依赖该组件的OS，做集成验证：
   ```bash
   cd tg-rcore-tutorial-sbi
   ./systest.sh -l  #测试依赖本地修改后的组件的各个内核是否能通过它们自身测试
   # 如果测试在crates.io上的组件是否能通过测试，则执行  ./systest.sh
   # 可通过修改 systest.txt 和sysdeps.txt 来灵活调整测试对象和测试本地修改的组件
   ```

## 6. 推荐学习/开发顺序

1. `tg-rcore-tutorial-ch1 -> tg-rcore-tutorial-ch2`：跑通启动、Trap、基础 syscall
2. `tg-rcore-tutorial-ch3 -> tg-rcore-tutorial-ch4`：完成任务调度到地址空间
3. `tg-rcore-tutorial-ch5 -> tg-rcore-tutorial-ch6`：完成进程与文件系统
4. `tg-rcore-tutorial-ch7 -> tg-rcore-tutorial-ch8`：完成 IPC、线程与并发同步
5. 回到 `tg-rcore-tutorial-*`：按组件抽象复盘与重构

## 7. 常见问题（FAQ）

### Q1：为什么我在根目录 `cargo run` 不会直接跑某个章节的内核？

因为章节 crate 不在根 workspace 默认成员里。请进入 `tg-rcore-tutorial-ch<N>` 目录运行。

### Q2：为什么 exercise 测试和 base 测试结果不同？

`exercise` 会启用章节额外需求（例如新增 syscall 或扩展行为），与基础模式测例不同是正常的。

### Q3：如何快速定位“实现错了还是输出格式错了”？

先使用章节 `./test.sh`，再用 `tg-rcore-tutorial-checker` 管线检测输出，可快速区分行为错误与输出不匹配。

## 8. 相关文档入口

- 章节文档：`tg-rcore-tutorial-ch[1-8]/README.md`
- 练习说明：`tg-rcore-tutorial-ch[34568]/exercise.md`

## 9. 高频错误速查表（学生版）

> 使用方法：先按“现象”定位，再执行“快速定位命令”，最后按“优先修复动作”处理。

| 现象 | 常见原因 | 快速定位命令 | 优先修复动作 |
|---|---|---|---|
| `can't find crate for core` 或目标不支持 | 未安装 RISC-V 目标 | `rustup target list --installed | rg riscv64gc-unknown-none-elf` | `rustup target add riscv64gc-unknown-none-elf` |
| `qemu-system-riscv64: command not found` | QEMU 未安装或不在 PATH | `qemu-system-riscv64 --version` | 安装 `qemu-system-misc`（Linux）或 `qemu`（macOS） |
| 在仓库根目录 `cargo run` 失败 | 章节 crate 不在根 workspace 默认成员 | `pwd`（确认当前目录） | 进入具体章节目录后再 `cargo run`，如 `cd ch4` |
| `cargo clone: command not found` | 缺少构建依赖工具 | `cargo clone --version` | `cargo install cargo-clone` |
| 构建阶段找不到 `rust-objcopy` | 缺少 `cargo-binutils/llvm-tools` | `rust-objcopy --version` | `cargo install cargo-binutils && rustup component add llvm-tools` |
| ch6+ 运行时报块设备/镜像相关错误 | `fs.img` 未生成或路径不匹配 | `test -f target/riscv64gc-unknown-none-elf/debug/fs.img && echo ok || echo missing` | 在对应章节先 `cargo build`，再 `cargo run` |
| 日志出现 `unsupported syscall` | syscall 未注册或用户/内核接口不一致 | `LOG=trace cargo run` | 检查 `tg_syscall::init_*` 与对应 `impls` 是否已实现并初始化 |
| 运行中出现 `page fault` / `stval` 异常 | 用户指针未翻译、权限标志不匹配、映射缺失 | `LOG=trace cargo run` 并关注 trap 日志 | 优先检查 `translate()` 调用、`VmFlags` 权限、`map/unmap` 范围 |
| `base` 能过但 `exercise` 失败 | 练习功能未实现或 feature 开关不一致 | `./test.sh base && ./test.sh exercise` | 对照 `exercise.md` 完成功能后使用 `--features exercise` 回归 |
| 测试输出看似正确但仍判失败 | 输出格式与 checker 期望不一致 | `cargo run 2>&1 | tg-rcore-tutorial-checker --ch <N> [--exercise]` | 先修行为再修日志格式，避免额外杂项输出污染 |

---

如果你是课程开发者，建议先读完本 README，把`tg-rcore-tutorial-ch[1-8]`的README.md看看，并运行一下，再从 `tg-rcore-tutorial-ch3` 或 `tg-rcore-tutorial-ch4` 开始做一个完整练习闭环（实现 -> 测试 -> 回归 -> 文档化），可以最快理解本仓库的“章节驱动 + 组件复用”开发模式。

## License: GNU GENERAL PUBLIC LICENSE v3