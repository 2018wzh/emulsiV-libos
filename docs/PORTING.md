# 开发、扩展与移植

## 工作区恢复

2026-09-23 MCPX 恢复后，项目已在 `/home/wzh/PlayGround/emulsiV-libos` 恢复，
实际 Rust 编译、全部示例和双模拟器验证已执行。当前结果见 `VERIFICATION.md`。
在新的环境中，可通过 GitHub 克隆或把 Git bundle 放到目标主机恢复历史：

```bash
cd /home/wzh/PlayGround
git clone /path/to/emulsiV-libos.bundle emulsiV-libos
cd emulsiV-libos
cargo test --locked --lib --tests
python3 tools/build.py --all
```

不要将未完成的构建描述为通过。优先修复 Rust 编译错误与超预算示例，再在上游界面实测。

## 新建程序

在 `examples/name.rs` 中使用 `#![no_std]`、`#![no_main]` 和 `entry!`。
通过同一 `Mmio` 句柄按作用域创建各外设驱动，避免无意持有长期互斥借用。
新示例可以直接被 `python3 tools/build.py --example name` 发现。

外部应用依赖此 crate 时，要在最终应用的构建流程中加入 `link.x`。
本 crate 中针对同包示例的 linker flags 不能假定自动传递给所有下游二进制。

## 先验证最小闭环

首先构建 hello，审核 ELF 架构、两个固定入口、程序与栈地址，转换为 HEX。
然后依次验证 TextIO、GPIO、Bitmap 和单一中断源。最后组合高级逻辑。
不要从大量格式化输出、复杂调度或全屏双缓冲开始。

## 超出预算时

使用 `dist/<example>.json` 查看静态镜像大小和余量。删掉未必要的消息字符串、
通用格式化、多余拷贝和大栈数组；保持 release、LTO、单 codegen unit、opt-level=z。
示例分开编译比把所有功能塞入一个演示菜单更符合这个平台。

不要修改 RAM 起始地址来躲开固定向量，也不要放宽 linker 的 RAM 长度而不修改并验证模拟器。
`__stack_size_override` 只允许调整真实 RAM 内的栈划分，不会增加总内存。
减少栈必须有最坏情况调用深度与 IRQ 开销依据。

## 新外设与测试

新驱动优先依赖 `RegisterIo`。在主机模拟总线中记录地址、位宽和写入值，测试设备语义。
只有新增了真实硬件映射，才能扩展 `Mmio` 的允许地址集合。
不要通过任意地址 peek/poke 接口破坏安全驱动边界。

工具层的 `supported()` 是刻意严格的指令白名单。新的 Rust 编译器即使成功链接，
也可能生成不兼容指令；不能直接关闭检查。检查新增指令是否受上游 Virgule 支持。

## 上游一致性与后续验证

官方上游固定为 `9e15421cd33511d4d2911fea1ae41cd65f33dae9`。
完整命令为 `bash tools/verify.sh /path/to/upstream`。
Bitmap 是 RGB332 256 色；不要用三个高位模拟独立 RGB 通道。
Shell 和 line_console 用分号提交，CR/LF 字节注入同样受支持。
浏览器完整 UI 端到端测试和所有可能路径的栈上界证明仍未完成。
新增功能必须重新运行验证，不要把当前测试结论自动扩展到修改后的固件。
