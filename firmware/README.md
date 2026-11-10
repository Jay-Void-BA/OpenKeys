# 固件导航 — R0.5

[状态](../docs/DEVELOPMENT_STATUS.md) · [架构](../docs/ARCHITECTURE.md) · [会话规则](../docs/PIN_SESSIONS_R05.md)

当前 H573 / SE050 / DFR0555 / 十键板的目标见 **[r05](r05/README.md)**。
其构建契约绑定保存的 R05 PCB 与引脚，现有 ARM 镜像仅公开电气诊断，
没有生产认证、真实 PIN 验证、USB/NFC 协议或充电功能。

| 模块 | 用途与限制 |
|---|---|
| [board](board/README.md) | 共享驱动；R05 的 `dfr0555` / `ten_keys` / `pin_ui` / `otp`，旧 LCD / 四键 / FPC 仅历史版本 |
| [device-policy](../crates/device-policy) | 六位本地输入、首次设置策略、配对会话租约与一次请求执行；真实 Root verifier 尚待接线 |
| [se-state](../crates/se-state/README.md) | SE 对象、计数、PIN / OTP 候选 APDU；实片资格和完整擦除尚待完成 |
| [authenticator-core](../crates/authenticator-core) | 固定 OpenSK 网关；生产 Env、硬件密钥与 USB/NFC 待集成 |
| [se-transport](../crates/se-transport/README.md)、[scp03](../crates/scp03/README.md) | SE 侧有界传输与认证加密；不等于主机管理通道已完成 |
| [h573-crypto](../crates/h573-crypto/README.md)、[secure-mmio](secure-mmio/README.md) | SAES / RNG 和明确 MMIO 边界；非完整 TrustZone 配置 |
| [device-vault](../crates/device-vault/README.md)、[device-records](../crates/device-records/README.md) | 加密快照和记录；Root 生命周期 / 完整重置仍待集成 |

生产 Root 必须独占键盘、LCD、SE、RTC / 时间、DMA、密钥内存和故障路径；
实现可信启动、签名更新、防降级、调试闭合、真实协议与加密管理。
SE_EN 是故障互锁，不是独立硬件 PIN 校验。详见[安全复查](../docs/SECURITY_REVIEW_R05.md)。

旧 [target](target/README.md) 和已有 R04 诊断保留历史测试用途，禁止用于 R05 板。
