# emulsiV-libos

[English](README.md) | [简体中文](README.zh-CN.md)

面向 [emulsiV / Virgule 模拟器](https://eseo-tech.github.io/emulsiV/)的 Rust `no_std` 库操作系统。
版本：**0.1.0-preview.2**。许可证：**MIT**。

本库提供 TextIO、GPIO、RGB332 图形、裸机运行时和可选的可回收堆。
Rust `xtask` 负责构建、审计、测试和打包。
仓库包含 25 个独立示例。

[快速开始](#quick-start) · [堆支持](#heap) · [示例](#examples) · [命令](#xtask) · [验证](#verification)

<a id="scope"></a>
## 适用范围

本项目适合学习裸机 Rust，或为原版 emulsiV 编写小型程序。
应用直接调用库函数，不经过系统调用。
默认构建不启用全局堆。

这是教学预览版，不是生产级操作系统。
项目不提供 POSIX、进程、虚拟内存、文件系统、网络或抢占式线程。
在 1.0 版本之前，API 可能发生变化。

“固件镜像”指一个应用与其使用的库函数链接后的程序。
“堆”指应用可以在运行时分配和释放的内存。
“tick”指应用定义的一次计数器递增，不代表一毫秒。

<a id="requirements"></a>
## 环境要求

运行发布包中的 HEX 文件只需要浏览器和 emulsiV，不需要安装 Rust。

从源码构建需要通过 `rustup` 安装 Rust，并准备 Git 和 Rust 主机平台所需的标准链接器。
仓库固定使用 Rust **1.85.1** 和 **`riscv32i-unknown-none-elf`** 目标。
首次构建会下载固定的 Rust 工具链和锁定的主机依赖。
目标端库没有第三方 crate 依赖。

`xtask` 实现不依赖 Python、Node.js、Shell 脚本、Clang 或独立的 RISC-V 工具链。
汇编检查使用 `rustc` 和自带的 `rust-lld`。
上游一致性测试通过 Rust Boa 引擎执行官方模拟器的 JavaScript 模块。
这些模块是外部测试输入，没有被替换为另一套实现。

发布到 GitHub 还需要已登录的 `gh` CLI。
完整验证流程在 Linux x86_64 上测试。
其他主机平台不在本版本的验证范围内。

<a id="quick-start"></a>
## 快速开始

### 运行已构建的固件

1. 打开[发布页面](https://github.com/2018wzh/emulsiV-libos/releases/tag/v0.1.0-preview.2)。
2. 下载固件 ZIP 和对应的 SHA256 文件。
3. 解压 ZIP。
4. 打开 emulsiV。
5. 加载解压目录中的 `firmware/hello.hex`。
6. 复位处理器。
7. 启动执行。

TextIO 应输出 `Hello, emulsiV Rust!`。
加载 `firmware/bitmap_palette.hex` 可以查看全部 256 种 RGB332 颜色。
每次只加载一个镜像。每个镜像都是独立应用。

### 构建示例

```bash
git clone https://github.com/2018wzh/emulsiV-libos.git
cd emulsiV-libos
git checkout v0.1.0-preview.2
rustup show
cargo xtask build hello
```

输出文件是 `dist/preview2/hello.hex`。
同一目录还包含 ELF 文件和审计报告。
镜像包含不支持的指令或超过内存预算时，命令会失败。

在独立的 Rust CPU 模型中运行示例：

```bash
cargo xtask run hello
cargo xtask run text_echo --input "Hello"
cargo xtask run heap_box
```

该模型是开发工具，不测试浏览器界面。

<a id="features"></a>
## 功能

| 分类 | 功能 |
| :--- | :--- |
| TextIO | 字节输入输出、轮询、输入中断、整数输出、可选的 `core::fmt::Write` |
| GPIO | 方向配置、掩码输出、翻转、输入采样、边沿事件、中断掩码 |
| Bitmap | 32 × 32 RGB332 像素、裁剪、直线、矩形、圆、3 × 5 文字、滚屏、单色贴图 |
| 固定存储 | FIFO 队列、类型化事件、UTF-8 字符串、128 字节单色缓冲、借用式 arena |
| 任务与输入 | 协作回调、截止时间、执行预算、消抖、逻辑 tick 软件 PWM |
| 工具函数 | CRC16、CRC32、非密码学随机数、内存诊断 |
| 运行时 | 固定复位和中断向量、BSS 初始化、寄存器保存、C 内存函数 |
| 可选堆 | 对齐分配、释放、复用、清零、重新分配、OOM 处理、使用统计 |

| Cargo feature | 默认状态 | 作用 |
| :--- | :--- | :--- |
| `runtime` | 开启 | 加入目标端启动代码和 panic 处理器 |
| `heap` | 关闭 | 启用 `alloc` 和目标端全局分配器，同时启用 `runtime` |
| `format` | 关闭 | 为 TextIO 实现 `core::fmt::Write` |

使用 `--no-default-features` 可以只使用不依赖硬件的库组件，不包含运行时。
在主机构建中启用 `heap` 只会提供用于测试的借用缓冲分配器。
它不会替换主机进程的全局分配器。

### 硬件限制

```text
0x00000000           reset vector
0x00000004           interrupt vector
0x00000008..         code, constants, initialized data, BSS
align16(image_end)   optional heap start
0x00000a00           heap end / stack bottom
0x00000a00..0x0bff   512-byte reserved stack
0x00000c00..0x0fff   1024-byte framebuffer
0xb0000000/1        TextIO status / input byte
0xc0000000          TextIO output byte
0xd0000000..0xd0000013   five 32-bit GPIO registers
```

普通 RAM 共 3072 字节，由代码、数据、堆和栈共享。
帧缓冲另占 1024 字节。
链接器禁止堆与栈或帧缓冲重叠。

GPIO 方向位 `1` 表示输入。GPIO 边沿事件寄存器可直接读写，不是“写 1 清除”寄存器。
RGB332 的 bit7..5 表示红色，bit4..2 表示绿色，bit1..0 表示蓝色。
饱和颜色常量为 `RED=0xe0`、`GREEN=0x1c`、`BLUE=0x03` 和 `WHITE=0xff`。

固件不要求 CSR、ECALL、WFI、FENCE、原子指令、压缩指令或硬件乘除法指令。
中断使用模拟器的固定入口及其支持的 MRET 指令。

<a id="heap"></a>
## 堆支持

通过 `xtask` 构建堆示例时，工具会自动选择所需 feature：

```bash
cargo xtask build heap_vec
cargo build --locked -p emulsiv-libos --release --target riscv32i-unknown-none-elf --features heap --example heap_vec
```

全局分配器在首次使用时初始化。
它管理 `__heap_start` 到 `__heap_end` 之间的对齐内存。
堆尾固定为预留栈的底部，不跟随当前栈指针移动。

分配器使用首次适配位图，以 16 字节为分配单位。
释放后的单位可以再次使用。相邻空闲单位可以满足更大的分配请求。
分配器状态在 RV32I 上占 36 字节，编译器还可能加入额外的分配标志。
每次分配最多因取整浪费 15 字节。
对齐要求和碎片也会减少实际可用空间。

在目标端启用 `heap` 后，`Box`、`Vec` 和 `String` 使用此分配器。
下面的应用分配并释放一个装箱整数：

```rust
#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use emulsiv_libos::{bus::Mmio, textio::TextIo};

emulsiv_libos::entry!(main);

fn main() -> ! {
    // SAFETY: this program runs on stock emulsiV with the supplied link.x.
    let mut bus = unsafe { Mmio::new() };
    let value = Box::new(42u32);
    TextIo::new(&mut bus).write_u32(**core::hint::black_box(&value));
    drop(value);
    emulsiv_libos::runtime::halt()
}
```

需要从分配失败中恢复时，使用 `Vec::try_reserve_exact` 或 `String::try_reserve_exact`。
原始分配接口在失败时返回空指针。
`Box::new` 等不可恢复的分配操作可能在 OOM 时进入 panic 处理器。
panic 处理器输出 `panic` 后停止，不执行栈展开。

重新分配会先取得另一块内存，再复制公共前缀并释放原块。
重新分配失败时，原分配保持有效，内容不变。
该方法需要临时空闲空间，即使某些缩小请求理论上可以原地完成。

`heap::stats()` 返回容量、已用字节、空闲字节、最大连续空闲块和峰值用量。
统计值按分配单位折算为字节，不等于精确的有效负载长度。
`Heap::new(&mut buffer)` 可以在独占借用的缓冲区上提供相同的分配算法。
调用原始指针接口时，必须遵守文档中的生命周期和释放规则。

| 堆示例 | 静态镜像字节 | 可用堆字节 | 预留栈字节 |
| :--- | ---: | ---: | ---: |
| `heap_box` | 2336 | 224 | 512 |
| `heap_vec` | 2244 | 304 | 512 |
| `heap_string` | 2292 | 256 | 512 |
| `heap_reuse` | 1572 | 976 | 512 |
| `heap_oom` | 1484 | 1072 | 512 |

上述值对应固定工具链和仓库中的示例。新增代码可能改变预算。
不要根据无堆 `hello` 的剩余空间推算启用堆后的容量。
默认 `shell` 仅剩 32 字节静态余量。为它添加堆还需要继续压缩体积。
详见[堆设计与安全约束](docs/HEAP.md)。

<a id="examples"></a>
## 示例

`xtask` 为堆示例启用 `heap`，其他示例使用默认 feature。
使用 GPIO 示例前，请将模拟器输出脚设置为 LED，将输入脚设置为按钮或开关。

| 示例 | 行为或操作 |
| :--- | :--- |
| `hello` | 输出问候文本 |
| `text_echo` | 轮询并回显输入字节 |
| `line_console` | 编辑并回显完整行，使用 `;` 提交 |
| `gpio_mirror` | 将 GPIO16..31 输入复制到 GPIO0..15 输出 |
| `gpio_debounce` | 对 GPIO31 消抖，再翻转 GPIO0 |
| `gpio_pwm` | GPIO0 输出 16 tick 周期、4 tick 高电平的 PWM |
| `bitmap_palette` | 显示全部 256 种 RGB332 颜色 |
| `bitmap_shapes` | 绘制直线、矩形、实心块和圆 |
| `bitmap_text` | 绘制字母和数字 |
| `mono_sprite` | 通过 128 字节缓冲显示单色贴图 |
| `cooperative` | 执行 LED 和心跳回调 |
| `event_loop` | 将文本加入事件队列，空格翻转 GPIO0 |
| `irq_echo` | 在 TextIO 中断中回显输入 |
| `irq_gpio` | 处理 GPIO31 边沿中断并翻转 GPIO0 |
| `crc_demo` | 输出已知 CRC 校验值 |
| `arena_demo` | 从借用式 arena 分配对齐存储 |
| `shell` | 通过严格检查的文本命令控制 GPIO 和 Bitmap |
| `random_pixels` | 绘制确定性的伪随机像素 |
| `diagnostics` | 报告静态布局和当前栈空间 |
| `paint` | 用小写 `w`、`a`、`s`、`d` 移动，`0`..`7` 选色，`c` 清屏 |
| `heap_box` | 分配 Box 并输出真实堆地址 |
| `heap_vec` | 扩容 Vec，并在扩容失败后保持原内容 |
| `heap_string` | 分配、追加、输出并释放 String |
| `heap_reuse` | 连续 100 次分配、清零并释放对齐存储 |
| `heap_oom` | 拒绝超大分配，然后再次正常分配 |

### Shell 输入

加载 `shell.hex`，然后在 TextIO 中逐条键入：

```text
?;
w 0x55aa;
r;
c 0;
p 16 16 0xe0;
p 17 16 0x03;
```

这些命令依次显示帮助、写 GPIO、读 GPIO、清屏以及绘制红色和蓝色像素。
官方输入界面会过滤 Enter 等特殊键，因此行输入示例额外接受 `;` 作为终止符。
直接注入 CR/LF 字节仍然受支持。
输入设备只能保存一个字节，粘贴文本不是可靠的串行传输。
解析器会拒绝整条超长输入，不执行被截断的命令。

<a id="xtask"></a>
## Rust 任务命令

请在 Git 检出目录中执行这些命令。库的 `.crate` 文件不包含主机端 `xtask` 工作区成员。

| 命令 | 结果 |
| :--- | :--- |
| `cargo xtask doctor` | 显示工具链和输出路径 |
| `cargo xtask build --all` | 构建并审计全部 25 个固件 |
| `cargo xtask build NAME` | 使用所需 feature 构建一个示例 |
| `cargo xtask audit ELF` | 检查 ELF 结构、指令和内存限制 |
| `cargo xtask hex ELF OUTPUT` | 审计外部 ELF 并创建 Intel HEX 文件 |
| `cargo xtask run NAME --input TEXT` | 构建示例并在 Rust CPU 中运行 |
| `cargo xtask test` | 执行库、堆、文档和任务工具的单元测试 |
| `cargo xtask runtime` | 检查汇编启动、中断寄存器保存和内存函数 |
| `cargo xtask fetch-upstream` | 获取固定版本的官方模拟器 |
| `cargo xtask smoke` | 在 Rust CPU 中执行全部固件行为测试 |
| `cargo xtask upstream` | 通过 Boa 对官方核心执行相同的行为检查 |
| `cargo xtask docs` | 检查双语 README 和示例覆盖 |
| `cargo xtask crate-check` | 验证 Cargo 包及独立的下游堆应用 |
| `cargo xtask verify` | 执行全部发布检查 |
| `cargo xtask package` | 打包未变化且已验证的源码与产物 |
| `cargo xtask check-archive ZIP` | 检查归档路径、内容和 SHA256 |
| `cargo xtask release` | 推送 `main`，在 CI 成功后创建 GitHub 预览版 |

完整构建后，`smoke` 和 `upstream` 也可以接受单个示例名。
产物和报告写入 `dist/preview2/`。
旧脚本以历史文本形式保存在 `legacy/`，当前命令不会调用它们。

<a id="integration"></a>
## 在其他应用中使用

开发时可以使用路径依赖，也可以指定 Git 发布标签：

```toml
[dependencies]
emulsiv-libos = { git = "https://github.com/2018wzh/emulsiV-libos.git", tag = "v0.1.0-preview.2", features = ["heap"] }

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
```

将此版本的 `link.x` 复制到应用目录。
在应用的 `build.rs` 中明确设置最终链接脚本。
不要假定依赖库的构建脚本会传递最终二进制链接参数。
完整配置见[集成指南](docs/PORTING.md)。
`crate-check` 会针对 Cargo 解包后的实际发布包构建并运行独立应用，而不是直接使用当前检出目录。

<a id="verification"></a>
## 验证与安全边界

先获取官方测试依赖，再执行全部检查：

```bash
cargo xtask fetch-upstream
cargo xtask verify
```

发布检查覆盖双语 README、Rust 格式、Clippy、feature 组合、堆行为、ELF/HEX 解析、汇编 ABI 和全部固件。
它还会验证独立应用能使用打包后的库。
每个固件在独立 CPU 和官方核心上使用同一组 Rust 断言。
官方源码固定为 `9e15421cd33511d4d2911fea1ae41cd65f33dae9`。

发布要求提交状态干净，并且产物摘要没有变化。
失败或未完成的验证不能授权打包。
ZIP 包含验证记录、原始命令日志、报告和文件摘要。
详见[验证说明](docs/VERIFICATION.md)与 [GitHub Actions](https://github.com/2018wzh/emulsiV-libos/actions)。

这些检查不等于完整 RISC-V 合规验证、内存安全证明或浏览器 UI 测试。
已观测到的栈使用不是所有输入的最坏情况上界。
运行时栈超过预留空间时，程序仍可能破坏内存。
模拟器没有隔离代码、堆和栈的硬件保护。

全局分配器在访问内部状态时屏蔽两个已知设备中断源。
这种串行化方式只适用于原版单 hart 平台。
它不是通用的多 hart 锁，也不保证设备输入无损。
增加其他中断源前，必须重新审查此约束。
需要可预测延迟时，应避免在中断回调中分配内存。

<a id="troubleshooting"></a>
## 故障排查

| 现象 | 处理方法 |
| :--- | :--- |
| `firmware exceeds stock 3 KiB RAM budget` | 减少输出、格式化或状态，不要放宽链接脚本的 RAM 大小 |
| 分配返回空指针或 `try_reserve_exact` 失败 | 缩小请求，检查空闲空间、对齐、碎片和重新分配所需的临时空间 |
| 出现 `panic` | 停止执行，检查断言、边界和不可恢复的分配请求 |
| Enter 不能提交行 | 在 `shell` 和 `line_console` 中使用 `;` |
| 输入字符丢失 | 降低输入速度，硬件是单字节锁存器，不是 FIFO |
| 上游版本检查失败 | 恢复到固定提交的干净检出，不要关闭检查 |
| 打包提示源码或证据变化 | 提交预期修改，然后重新执行 `cargo xtask verify` |
| CI 尚未完成 | 查看 Actions，待该提交通过后重新执行 `cargo xtask release` |

需要恢复已修改的初始化数据时，应在冷启动前重新加载 HEX。
仅跳转到复位向量不会恢复 `.data` 的初始字节。

<a id="release"></a>
## 发布流程

发布前更新包版本、双语 README 和变更记录。
最终验证前，将预期源码修改全部提交到 `main`。

```bash
cargo xtask fetch-upstream
cargo xtask verify
cargo xtask package
cargo xtask release
```

`release` 只接受配置的目标仓库和 `2018wzh` GitHub 账户。
它保留仓库可见性，不强制推送，也不覆盖已有标签或发行版。
第一次执行可能先推送提交，再因 CI 尚未完成而停止。CI 成功后重新执行即可。
随后命令会上传固件 ZIP、SHA256 文件和已验证的 `.crate` 文件。

GitHub 发布与 crates.io 发布相互独立。
项目会验证 Cargo 包，但 `xtask release` 不会把它上传到 crates.io。
注册表发布需要 crates.io 所有者另行审核并授权。

<a id="contributing"></a>
## 参与开发

每次修改行为时，增加对应的主机回归测试。
新增固件示例时，增加 Rust 行为场景。
命令、前提或限制发生变化时，同步更新两份 README。
提交修改前执行全部发布检查。
详见 [CONTRIBUTING.md](CONTRIBUTING.md) 和[写作规范](docs/WRITING.md)。

<a id="license"></a>
## 许可证与参考资料

项目代码采用 [MIT 许可证](LICENSE)。
单独下载的官方模拟器保留其 MPL-2.0 许可证。
固件归档不重新分发官方模拟器源码。
主机依赖保留各自许可证。

主要参考：[emulsiV 官方文档](https://eseo-tech.github.io/emulsiV/doc/)、
[Rust 裸机目标](https://doc.rust-lang.org/rustc/platform-support/riscv32-unknown-none-elf.html)、
[GlobalAlloc 安全约定](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html)、
[Boa 0.20 API](https://docs.rs/boa_engine/0.20.0/boa_engine/)。
