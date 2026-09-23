# 验证记录

日期：2026-09-23。本文记录实际执行结果，不把源代码、CI 配置或待执行脚本等同于通过验证。

## 本地已执行

在当前会话的容器环境执行以下命令，退出码均为 0：

```bash
python3 -m unittest discover -s tools -p 'test_*.py' -v
python3 tools/check_runtime.py
```

35 项 Python 工具/独立参考模型测试通过。日志见 [python-tests.log](verification/python-tests.log)。
共享启动汇编与链接脚本通过 clang/LLD 编译；汇编级测试验证复位、BSS 清零、栈对齐、
framebuffer 保留、100 次中断往返的寄存器/PC 恢复、弱内存函数、用户中断回调覆盖，
以及故意超出 RAM 预算的链接失败。完整结果见 [assembly-runtime.json](verification/assembly-runtime.json)。

此测试的固件为汇编夹具，不是 Rust 编译产物。100 次中断往返也不是 Rust 应用栈峰值证明。

## 已提供但尚未运行

42 项 Rust 主机测试、20 个 Rust 裸机示例、默认/format 两种 feature 的测试、
全部 Rust 示例的交叉编译和体积检查，以及基于 Rust 编译产物的参考执行器 smoke tests。
上游 emulsiV 浏览器实测同样未执行。
当前本地容器没有 rustc、cargo 或 rustup，Rust 下载域名解析失败；没有据此声称 Rust 编译通过。

## MCPX 环境选择结果

MCPX 返回 5 个已注册工作区。成功读取的 PlayGround 与 VeriSpecOSLab 环境
均报告同一 MCPX PID 830396、64 个逻辑 CPU、Arch Linux 和 Rust 1.99.0-nightly。
因此切换这两个工作区不等于切换独立主机。Docker CLI 存在不代表容器守护进程或镜像可用。

本项目独立 PlayGround 会话创建失败：`database or disk is full (13)`。
通过已有会话发起的最小诊断、随后 LabWeaver 项目读取和工作区列表查询均返回 HTTP 502。
没有取得有效命令输出或新任务 ID，无法确认可用磁盘空间、已安装 RV32I 目标或 GitHub CLI 认证。
未删除、清理或覆盖用户现有项目。详见 [environment-selection.json](verification/environment-selection.json)。

本次没有确认到可执行验证和发布的 MCPX 环境；没有把本地容器称作 MCPX 环境。
`/home/wzh/PlayGround/emulsiV-libos` 的创建与 GitHub 推送均未完成。

## 发布前必须完成

MCPX 服务与数据库恢复可写后，在独立项目目录恢复本 Git 仓库，核对 Rust 工具链和
`riscv32i-unknown-none-elf` 目标，然后执行：

```bash
cargo test --locked --lib --tests
cargo test --locked --features format --lib --tests
python3 tools/build.py --all
python3 tools/smoke.py
```

修复所有编译错误、指令不兼容和 RAM 超预算后，再运行 `bash tools/publish.sh`。
发布脚本默认创建 private 仓库，只接受目标账号 2018wzh，拒绝无关 origin 和强制推送。
脚本本身及 GitHub Actions 尚未通过真实发布流程验证。
