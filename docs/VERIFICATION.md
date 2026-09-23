# 验证记录

日期：2026-09-23。以下是恢复后的 MCPX 远端实际执行结果，不是待运行测试列表。

## 环境与源码恢复

工作区：`PlayGround/emulsiV-libos`，Arch Linux x86_64。
使用固定 Rust 1.85.1、`riscv32i-unknown-none-elf`、Clang 22.1.8、
Rust 自带 `rust-lld`、Python 3.14.7 和 Node 26.8.2。
原 Git 提交 `a3f54000db77b05623bfea056a6dfa9c4d09abd5` 已精确恢复并通过 git fsck，
当前修复提交建立在原历史之上。早先数据库/磁盘故障记录保留在 `verification/`，
那些文件属于历史诊断，不描述当前可用性。

## 已执行的验证层次

| 验证层 | 实际结果 |
| :--- | :--- |
| Rust 默认 feature 主机测试 | 42 项原测试 + 5 项发布回归，47/47 通过 |
| Rust format feature 主机测试 | 42 项原测试 + 6 项发布回归，48/48 通过 |
| Python 工具/独立参考模型 | 35/35 通过 |
| 共享汇编、链接器与 C ABI | 10 项检查通过，含 100 次中断往返 |
| RV32I Rust 固件 | 20/20 构建成功，指令及静态内存审计通过 |
| 独立 Python CPU | 9 个实际 Rust HEX 交互场景通过 |
| 官方 emulsiV JavaScript 核心 | 20/20 启动测试，每例 30,000 条指令；10 项交互/显示协议检查通过 |

官方上游固定提交：`9e15421cd33511d4d2911fea1ae41cd65f33dae9`。
`tools/upstream_smoke.mjs` 导入未修改的 Processor、Bus、Memory、TextIO、GPIO、
BitmapOutput 和 HEX 解析器，不重新实现它们。测试拒绝上游版本不符或存在未提交修改。
Bitmap 视图测试调用上游 BitmapOutputView，使用无界面的 canvas 适配器，
验证 1024 个像素、全部 256 种 RGB332 编码对应的渲染颜色。

Python 与上游场景覆盖文本回显、GPIO 输入输出、Bitmap、绘图、两种 IRQ、
分号行提交，以及 Shell 的 GPIO/像素操作、非法坐标、整数溢出、额外参数、
超长整行拒绝与恢复。数值解析回归还覆盖 4096 个伪随机 u32 的四种表示。

## 本轮修复

1. 纠正 RGB332 协议：红 7..5、绿 4..2、蓝 1..0，而不是三个单独高位。
   更新颜色常量、256 色调色板、绘图示例、RGB888 量化和断言。
2. Shell 数字前缀检测和命令参数共享解析；MMIO 常量调用内联消除冗余检查，
   不取消安全驱动的地址限制；减少 Shell 重复的逐键输出。
3. Shell 与 line_console 支持用 `;` 提交，适配过滤 Enter 的官方 TextInputView。
4. 汇编测试自动回退至 Rust 自带 LLD；清除无用 unsafe 告警。

## 内存与测试边界

默认普通 RAM 3072 字节，framebuffer 1024 字节，保留栈 512 字节。
最终 Shell 静态镜像结束地址为 2528，因此剩余静态空间为 **32 字节**。
独立模型与官方核心在已测 Shell 交互路径上均观测到 **160 字节**栈使用。
这是样本执行观测，不是任意输入/任意程序的最坏情况栈证明。
全部示例构建采用默认 feature；format feature 的主机测试不代表全部固件开启格式化也能放下。

本轮没有浏览器完整 DOM、键盘、鼠标和页面渲染的端到端测试；
官方核心测试与 canvas 适配器不能冒称浏览器 UI 验收。
未执行完整 RISC-V 合规套件、形式化内存安全证明或真实硬件认证。
软件 tick/PWM 不是毫秒，单字节 TextIO 也不是无损输入 FIFO。

## 复现与报告

```bash
bash tools/verify.sh /path/to/pinned/emulsiV
```

成功必须以 `ALL_RELEASE_GATES_PASSED` 结束。`dist/` 输出包括：

- `verification.log`：全部验收命令的原始输出。
- `build-report.json` 和每例 JSON：ISA、链接布局与 ELF SHA256。
- `runtime-verification.json`：汇编启动、中断和内存 ABI。
- `rust-smoke.json`：独立参考 CPU 场景。
- `upstream-smoke.json`：上游版本、20 例启动观测和交互场景。

上述报告随预览版固件 ZIP 发布，`manifest.json` 记录源码提交号和逐文件 SHA256。
GitHub Actions 使用同一验收脚本；远端 CI 状态以 Actions 运行记录为准。
