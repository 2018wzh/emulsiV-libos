# emulsiV-libos v0.1.0-preview.1

Rust no_std 教学型 libOS，包含 TextIO、GPIO、RGB332 Bitmap、裸机运行时，
以及固定容量容器、行编辑、协作调度、消抖、软件 PWM、图形和 20 个参考程序。

实际验收：47 项默认配置 Rust 测试、48 项 format 测试、35 项 Python 测试，
20 个固件构建和 ISA/RAM 审计、100 次汇编中断往返，
9 个独立 CPU 固件场景，官方核心 20 例启动和 10 项交互/显示检查。
上游提交：9e15421cd33511d4d2911fea1ae41cd65f33dae9。

固件 ZIP 内含 20 个可加载的 Intel HEX、验证日志、JSON 报告以及逐文件 SHA256 manifest。
先加载 firmware/hello.hex 或 firmware/bitmap_palette.hex，完整复位后运行。
Shell 用分号提交，例如 p 16 16 0xe0; 画红点，w 1; 控制低位 GPIO。

注意：Bitmap 原生编码为 RGB332，不是三位八色。Shell 保留 512 字节栈后，
静态空间仅余 32 字节。已测 Shell 栈使用 160 字节不是最坏情况上界证明。
浏览器完整 UI 端到端测试尚未执行。软件 tick 不是毫秒，不提供 POSIX、MMU、网络或抢占式线程。

这是预览版，API 和内存开销可能随后续修订变化。完整边界见 docs/VERIFICATION.md。
