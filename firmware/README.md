# 固件导航 — R0.4

[返回首页](../README.md) · [开发起步](../docs/GETTING_STARTED.md) · [当前状态](../docs/DEVELOPMENT_STATUS.md) · [架构](../docs/ARCHITECTURE.md)

**已有板级驱动、密码 / 认证模块和可编译 ARM 电气诊断；尚无完整生产认证镜像或签名更新包。** `.rlib`、主机程序和诊断 ELF 各自有用途，不能替代成品认证器。

## 目标硬件

STM32H573RIT6 / Cortex-M33、SE050E、认证 FPC2534AM、白背光 16 × 2 LCD、四个实体按键、USB-C USB 2.0 FS、FM11NC08S NFC、加密外部 Flash。[R0.4 引脚表](../hardware/r04/pinout.csv)与 CAD 摘要是板级构建契约。

## 从哪个模块开始

| 模块 | 作用 | 当前边界 |
|---|---|---|
| [board](board/README.md) | LCD、四键、Flash、FPC SPI、SE I2C 和板号检查 | 候选驱动；实片时序 / 故障 / 安全归属待核查 |
| [target](target/README.md) | H573 目标适配与电气诊断 | OPEN / TZEN=0；诊断关闭 SE、指纹、充电和私密操作 |
| [secure-mmio](secure-mmio/README.md) | 六个固定地址 PAC 构造的 Rust unsafe 边界 | 不是完整 TrustZone / GTZC 配置 |
| [device-policy](../crates/device-policy) | 指纹凭据、附加 PIN、LCD、一次授权和执行范围 | 真实 verifier / Root 必须实现硬件检查 |
| [authenticator-core](../crates/authenticator-core) | 固定 OpenSK 与受保护 FIDO 网关 | 生产硬件 Env / 密钥桥接 / 传输待集成 |
| [se-transport](../crates/se-transport/README.md)、[scp03](../crates/scp03/README.md) | 有界 T=1 与认证加密 SE 会话 | 实片身份、profile、配对和 ACL 待资格验证 |
| [se-state](../crates/se-state/README.md) | 持久对象、计数、提交锚点、SE 签名 / HKDF | 实际对象语义与完整密钥生命周期待验证 |
| [h573-crypto](../crates/h573-crypto/README.md) | SAES / RNG、共享所有权及故障关闭 | 芯片 KAT / SCA / FI 待执行 |
| [device-vault](../crates/device-vault/README.md)、[device-records](../crates/device-records/README.md) | 加密快照、流式记录和命名空间隔离 | Root 检查点、批量事务及重置生命周期待集成 |

## 受保护运行时必须负责什么

- 可信启动、签名更新、防降级、生产调试生命周期。
- FP / SE / LCD 总线、按键、可信时钟、DMA、密钥内存和 dispatcher 的安全归属。
- 每次操作的设备指纹、必要时实体 PIN、完整请求确认，以及取消 / 断连 / 挂起 / 故障后的撤权。
- 每设备唯一配对和密钥；不可在断电或备用域丢失后重新开放已配置设备的空白初始化。
- USB / NFC、认证加密管理、持久 PIN / 模板、充电 / 挂起和备用防拆服务。

SE 使能门是故障互锁，不是指纹证明。验证指纹所需的内部密码运算不等于用户获得私密操作权限；原始 APDU、任意密码接口或主机 Boolean 不可成为解锁通路。

## 构建和核查

完整软件 / ARM 命令见[开发起步](../docs/GETTING_STARTED.md)。诊断镜像的编译、用途与限制见[target](target/README.md)，已导出产物及源码摘要位于 [dist](dist)。

专项审查从[文档导航](../docs/README.md)进入。芯片 KAT、真实传感器 / SE 配对、物理攻击和端到端互通仍需按[安全测试计划](../docs/SECURITY_TEST_PLAN.md)逐项留证。
