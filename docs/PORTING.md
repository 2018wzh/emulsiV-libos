# 开发、扩展与移植

## 工作区恢复

本次 MCPX 在新建会话时返回 `database or disk is full (13)`。
复用已有无运行任务的 PlayGround 会话后，携带 Activity 的目录读取仍返回
`INVALID_ACTIVITY: database or disk is full (13)`。没有绕过审计记录，也没有清理用户文件。
因此 `/home/wzh/PlayGround/emulsiV-libos` 并未创建。

解除空间/数据库故障后，可以把 Git bundle 放到目标主机：

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

## 尚待完成

Rust 1.85.1 实际编译、42 项 Rust 测试、20 个示例内存审核、已编译 Rust 固件 smoke tests、
浏览器 emulsiV 实测、GitHub 新仓库创建与推送，以及 GitHub Actions 运行。
本次已运行验证不覆盖上述项目。
