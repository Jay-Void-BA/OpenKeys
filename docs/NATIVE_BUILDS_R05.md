# R05 GPUI 开发包与诊断

[源码与安全边界](../apps/manager-gpui/README.md) · [检查证据](ci-evidence-r05.json)

这些是工程开发产物，**没有真实认证设备后端**。GPUI 可以查看英文界面，设备操作明确拒绝；不能导入真实 OTP / notes 或解锁安全密钥。没有正式签名、安装包或移动 App。

源码提交 `60f0e601fb41986e18a2ee1d4d2207f63d1c26df`：[三平台 CI](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43414654181)通过。各平台362项测试（356项 workspace / 文档及6项默认关闭探针）；完整日志与注释中 warning 为0。文件 / 编译 / 模型结果不能证明实物安全。

下表 ZIP 摘要来自 GitHub API。逐包下载结果见[构建元数据](native-builds-r05.json)，未执行这些二进制；此前本地 Linux 界面观察不等于当前发布包或三系统设备互通验收。

| 产物 | 下载 | GitHub 提供的 ZIP SHA-256 |
|---|---|---|
| openkeys-gpui-development-windows-2022 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43414654181/artifacts/14032350866) | `sha256:65580c063981b55bbb6b624d25965eec894508ef8b7ce237a595345f656851b7` |
| openkeys-gpui-development-ubuntu-24.04 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43414654181/artifacts/14032295330) | `sha256:14763988f3647365ec53e95612be8fe32be6d27170c1eb5be4e0190ee331dfc0` |
| openkeys-gpui-development-macos-15 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43414654181/artifacts/14031587707) | `sha256:86be22cc59eb7663b149c8676322b49a7d87dcbb1c917f1861a64ece51677172` |
| r05-diagnostic-arm-elf | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43414654181/artifacts/14030843069) | `sha256:f2a3e5e2e953d88a7988f5637d2cc736b949754db0895503f418dc437ef5d58f` |

macOS 目标为 Apple Silicon，Windows 为 x64 MSVC，Linux 为 Ubuntu24.04 x86_64。平台运行库、GPU、原生对话框、下载权限及有效期分别核实。

R05 ARM 产物是 DFR0555 V1.0 / V1.1 电气诊断 ELF，**不是生产认证固件**。受源摘要绑定的 ELF / BIN 位于 [firmware/dist/r05](../firmware/dist/r05)；只验证公开 LCD、十键与 Flash ID，SE / 充电 / USB/NFC 协议关闭。不要输入真实 PIN / 凭据。
