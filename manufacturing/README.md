# 制造成品交付入口

**当前状态：BLOCKED，尚未达到“交给代焊厂即可做成完整成品”的标准。**

- [委托开发/验收任务书与价格取舍](HANDOFF.md)
- [关键器件、采购入口、选型状态及价格快照](critical-parts.csv)
- [实际预检报告](readiness-report.json)
- [指纹独立解锁协议设计及实现边界](../docs/BIOMETRIC_PROTOCOL.md)
- [硬件电路/结构文件](../hardware/r04/README.md)
- [你、工厂和实验室分别需要做的硬件验收](HARDWARE_ACCEPTANCE.md)

运行 `python3 scripts/check-production-ready.py` 重新生成报告；当前应返回退出码2。报告检查真实文件、器件状态、ERC及声明的交付摘要，缺少完整设备固件、布线/制造文件或实物证据时拒绝给出放行状态。完整文件集也只代表可提交人工评审，不是自动获得生产批准。

`release-manifest.json`应在文件真实存在后建立，`artifacts`按报告中的角色分别提供仓库内相对`path`与`sha256`。不要创建假BIN、空Gerber、假签名或把失败证据改成pass来满足检查。生成工具不下载、不烧录、不操作设备、不访问生产密钥。

R04的完整电气PCB、制造数据、NFC线圈和电池/接口选型已经提供；它们仍需装配/RF/电源实测。当前剩余项包括完整Rust认证运行时、真实SE对象/指纹/加密管理适配、生产启动与配对、三平台成品安装包，以及目标硬件与iOS/NFC/EMC/侧信道验证。诊断BIN及独立库测试不能补齐这些项目。具体以报告和任务书为准。
