# emulsiV-libos

面向 **ESEO-Tech emulsiV / Virgule** 的 Rust `no_std`、无堆依赖教学型 library OS。
不是 Linux、POSIX 兼容层，也不是带虚拟内存的完整操作系统。

> **当前状态：经过实际验证的教学预览版。**
> 2026-09-23，在 MCPX PlayGround 使用 Rust 1.85.1 完成 47 项默认配置、
> 48 项 format 配置 Rust 测试；全部 20 个示例通过 RV32I 构建和内存审核。
> 独立 Python CPU 通过 9 个固件场景；未修改的官方 emulsiV 核心启动全部 20 个示例，
> 并通过 10 项交互/显示协议检查。浏览器完整 UI 操作未做端到端测试。
> 详见 [验证记录](docs/VERIFICATION.md)。

## 功能

| 模块 | 已实现的源代码 |
| :--- | :--- |
| `textio` | 字节输入输出、非阻塞轮询、有限次数轮询、输入中断控制、精简整数/十六进制输出 |
| `gpio` | 方向配置、按位输出、翻转、输入采样、边沿事件、中断掩码、带边界检查的引脚编号 |
| `bitmap` | RGB332 256 色 framebuffer、裁剪像素、直线、矩形、实心矩形、圆、3×5 ASCII 字体、文字、滚屏、透明单色贴图 |
| `MonoBuffer` | 128 字节单色离屏缓冲，显示时选择前景色和背景色 |
| `console` | 固定容量行编辑、CR/LF、退格、Delete、Ctrl-U、Ctrl-C、整行溢出拒绝 |
| `queue` / `event` | 固定容量泛型 FIFO、显式满队列错误、类型化事件队列 |
| `input` | 按连续采样次数消抖、逻辑 tick 软件 PWM |
| `time` / `scheduler` | 可回绕逻辑计数、截止时间、固定任务数协作调度、执行预算、取消、漏过周期合并 |
| `fixed` / `arena` | UTF-8 定长字符串、对齐且清零的显式 bump arena |
| `shell` | 无分配命令解析、十进制/十六进制/二进制整数解析、溢出和参数检查 |
| `crc` / `rng` | 表驱动内存开销为零的 CRC16/CRC32、非密码学 XorShift32 |
| `runtime` | 固定复位/IRQ 入口、BSS 初始化、默认中断处理、用户回调、设备 IRQ 屏蔽、栈空间快照 |
| C ABI | 可被工具链覆盖的弱 `memcpy`、`memmove`、`memset`、`memcmp` 汇编实现 |
| 工具链 | ELF→Intel HEX、RV32I 指令白名单、RAM 布局审核、独立 CPU 参考执行器、CI 配置、发布脚本 |

所有高级功能按链接可达性裁剪。**不能把全部 API 同时使用却假定仍能放下。**

## 硬件边界

```text
0x00000000           reset: j __start
0x00000004           IRQ:   j __irq_entry
0x00000008..0x09ff   默认供代码、只读数据、初始化数据和 BSS 使用
0x00000a00..0x0bff   默认预留 512 字节栈
0x00000c00..0x0fff   32×32 framebuffer，1 字节/像素
0xb0000000/1        TextIO 状态 / 数据
0xc0000000          TextIO 输出
0xd0000000..10      GPIO dir / ien / rising / falling / value
```

GPIO `DIR=1` 表示输入。**GPIO 边沿事件寄存器是普通可写寄存器，不是写 1 清除。**
Framebuffer 使用 **RGB332**：bit7..5 为红，bit4..2 为绿，bit1..0 为蓝。
`RED=0xe0`、`GREEN=0x1c`、`BLUE=0x03`、`WHITE=0xff`。
`Color::from_bits` 保留全部 8 位；`Color::from_rgb888` 可将 8 位 RGB 通道量化。
不使用 CSR、ECALL、WFI、FENCE、压缩指令、原子指令或硬件乘除法。

## 构建

需要 Rust/rustup 和 Python 3.10+。`rust-toolchain.toml` 固定 Rust 1.85.1，
并声明 `riscv32i-unknown-none-elf` 目标。不依赖 crates.io 第三方库。
首次安装工具链需要联网；安装后项目源码构建无需下载第三方 crate。

```bash
rustup show
cargo test --locked --lib --tests
python3 tools/build.py --example hello
```

构建成功后得到 `dist/hello.hex` 与 `dist/hello.json`。在 emulsiV 界面中加载 HEX，
使用完整复位/重新加载后运行。编译结果未通过指令和 RAM 审查时，不生成成功报告。

```bash
# 构建并检查所有示例；任何一个失败都会返回非零状态。
python3 tools/build.py --all

# 在独立 Python 参考 CPU 中运行选定的已编译 Rust 程序。
python3 tools/smoke.py

# 单独运行一个 HEX；每个输入字节等待前一字节被消费。
python3 tools/rv32.py dist/text_echo.hex --input 'Hello' --steps 10000
```

该参考 CPU **不是上游 emulsiV，也不替代浏览器实测**。
如果某示例链接报 `firmware exceeds stock 3 KiB RAM budget`，必须缩减代码/状态，
不能简单放宽 RAM 地址冒充默认平台支持。先从 `hello`、`text_echo`、`bitmap_palette` 开始。

## 20 个参考程序

| 示例 | 作用与操作 |
| :--- | :--- |
| `hello` | 输出欢迎文本 |
| `text_echo` | 前台轮询回显 |
| `line_console` | 行编辑和完整行回显，以 `;` 提交 |
| `gpio_mirror` | GPIO16..31 输入映射到 GPIO0..15 输出 |
| `gpio_debounce` | GPIO31 按钮消抖后翻转 GPIO0 |
| `gpio_pwm` | GPIO0 的 16 tick 周期、4 tick 高电平 PWM |
| `bitmap_palette` | 显示完整 256 色 RGB332 调色板 |
| `bitmap_shapes` | 直线、矩形、实心块、圆 |
| `bitmap_text` | 绘制字母和数字 |
| `mono_sprite` | 单色 sprite 经 128 字节缓冲显示 |
| `cooperative` | LED 和心跳两个周期任务 |
| `event_loop` | 文本事件队列，空格翻转 GPIO0 |
| `irq_echo` | TextIO 中断回显 |
| `irq_gpio` | GPIO31 上升沿中断翻转 GPIO0 |
| `crc_demo` | 已知输入的 CRC 校验值 |
| `arena_demo` | 显式对齐内存分配 |
| `shell` | 文本命令控制 GPIO 和 Bitmap，以 `;` 提交；静态镜像 2528 字节 |
| `random_pixels` | 确定性伪随机彩色像素 |
| `diagnostics` | 链接布局和瞬时剩余栈空间 |
| `paint` | WASD 移动画笔、0..7 选色、c 清屏，并翻转 GPIO0 |

使用 GPIO 示例前，在模拟器中把输出脚配置成 LED，输入脚配置成按钮或开关。
所有延时和 PWM/调度 tick 都是软件逻辑尺度，**不是毫秒**。

### shell 命令

```text
?;                查看命令
r;                读取 GPIO
w 0x55aa;         修改低 16 个输出脚
c 0;              黑色清屏
p 16 16 0xe0;     在 (16,16) 画红色像素
p 17 16 0x03;     在 (17,16) 画蓝色像素
```

每次输入一条命令，等待处理后再输入下一条，不要把多字符粘贴当作串口发送。
官方 TextIO 界面过滤 Enter 等多字符键名，因此两个行输入示例额外用 `;` 提交。
直接注入 CR/LF 字节仍受支持；Shell 利用输入区显示已键入内容，不重复逐字符回显。
状态和参数采用严格解析。超长行整行拒绝，不执行前缀。
Shell 保留 512 字节栈后仅余 **32 字节**静态空间，扩展时必须重新审核。
TextIO 是文本区域而非 VT100 终端；退格示例输出 `<` 标记，不假定 ANSI 控制序列可用。
控制字节能否从键盘直接送入取决于上游输入界面，测试也可以直接注入字节。

## 最小 Rust 应用

```rust
#![no_std]
#![no_main]
use emulsiv_libos::{bus::Mmio, textio::TextIo};
emulsiv_libos::entry!(main);
fn main() -> ! {
    // 安全前提：正在 emulsiV 上运行，使用所附链接布局。
    let mut bus = unsafe { Mmio::new() };
    TextIo::new(&mut bus).write_str("Hello\n");
    emulsiv_libos::runtime::halt()
}
```

外部项目集成本库时，必须把本项目 `link.x` 纳入自己的最终链接设置；
不要假定依赖库的 build script 会替应用传递所有链接参数。
本仓库的示例是同一 Cargo package 的 example targets，已配置链接脚本。

## 测试与诊断

```bash
python3 -m unittest discover -s tools -p 'test_*.py' -v
python3 tools/check_runtime.py  # clang + ld.lld 或 Rust 自带 rust-lld
cargo test --locked --lib --tests
cargo test --locked --features format --lib --tests
cargo fmt --all                # 统一 Rust 源码排版
```

`format` feature 为 TextIO 增加 `core::fmt::Write`；默认使用更精简的数字输出函数。
请注意格式化代码可能增加固件体积。全部示例固件按默认 feature 构建。
完整验收入口（需要 Node 22.7+ 及固定版本的上游源码）：

```bash
git init ../.emulsiv-upstream
git -C ../.emulsiv-upstream fetch --depth 1 https://github.com/ESEO-Tech/emulsiV.git 9e15421cd33511d4d2911fea1ae41cd65f33dae9
git -C ../.emulsiv-upstream checkout --detach FETCH_HEAD
bash tools/verify.sh
```

仅在尚无上游检出时执行以上初始化步骤。已有目录请保持干净且版本一致。
也可运行 `bash tools/verify.sh /path/to/emulsiV`。输出保存到 `dist/verification.log`。
GitHub Actions 执行相同入口；运行结果以 Actions 页面为准。

## GitHub 发布

目标仓库为 `2018wzh/emulsiV-libos`。发布脚本默认创建私有仓库，
不会泄漏 token，不覆盖无关 origin，也不会强推。预览版固件包包含 20 个 HEX、
原始验证日志、审计报告、源码提交号和 SHA256 清单。

```bash
# 从提供的 bundle 恢复完整提交记录。
git clone emulsiV-libos.bundle emulsiV-libos
cd emulsiV-libos
gh auth login
bash tools/publish.sh
```

脚本只向登录用户 `2018wzh` 的 `emulsiV-libos` 仓库发布，首次创建默认使用 private。
它会先执行 `tools/verify.sh` 的全部检查，包括上游模拟器；失败就停止。
提交源码且验收成功后，`python3 tools/package.py` 生成可校验的预览版固件 ZIP。
既有 origin 不匹配目标仓库时会拒绝推送。

## 文档

[架构与并发边界](docs/ARCHITECTURE.md) · [验证记录](docs/VERIFICATION.md) ·
[开发与移植](docs/PORTING.md) · [变更记录](CHANGELOG.md)

## 上游参考

- emulsiV 官方文档：https://eseo-tech.github.io/emulsiV/doc/
- GPIO 源码：https://github.com/ESEO-Tech/emulsiV/blob/master/src/devices/gpio.js
- Rust RV32 裸机目标：https://doc.rust-lang.org/rustc/platform-support/riscv32-unknown-none-elf.html

项目代码、字体、汇编辅助函数和 Python 参考执行器为本项目实现；未打包上游模拟器源码。
许可证：MIT。
