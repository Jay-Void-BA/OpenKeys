# 文档导航

[返回首页](../README.md) · [当前状态](DEVELOPMENT_STATUS.md) · [开发起步](GETTING_STARTED.md) · [制造入口](../manufacturing/README.md)

当前版本为 **R0.4 工程候选**。先看状态和范围，再查看专项实现；“目标功能”“模型测试通过”“实物验证通过”是不同状态。不熟悉缩写时查看[术语速查](GLOSSARY.md)。

## 按阅读目的选择

| 目的 | 建议顺序 |
|---|---|
| 第一次了解项目 | [首页](../README.md) → [当前状态](DEVELOPMENT_STATUS.md) → [架构与授权规则](ARCHITECTURE.md) |
| 本地构建 / 阅读源码 | [开发起步](GETTING_STARTED.md) → [固件模块](../firmware/README.md) → [贡献指南](../CONTRIBUTING.md) |
| 查看或评审硬件 | [硬件文件](../hardware/r04/README.md) → [对抗性复核](ADVERSARIAL_REVIEW_R04.md) → [验收步骤](../manufacturing/HARDWARE_ACCEPTANCE.md) |
| 第一次委托制作 | [完整制作说明书](../manufacturing/BUILD_MANUAL_ZH.md)、[打印版 PDF](../manufacturing/OpenKeys-R04-build-manual-zh.pdf) → [工厂确认单](../manufacturing/ORDER_SHEET_R04.md) → [采购表](../manufacturing/procurement-r04-board.csv) |
| 委托工厂 / 实验室 | [制造入口](../manufacturing/README.md) → [服务商筛选](../manufacturing/MAINLAND_SUPPLIERS_AND_LABS.md) → [委托确认模板](../manufacturing/REQUEST_FOR_CONFIRMATION.md)；[联系进度](../manufacturing/OUTREACH_STATUS.md) |
| 查看管理软件 | [桌面管理器](../apps/manager/README.md)、[开发包](NATIVE_BUILDS.md)、[导入格式](IMPORT_FORMAT.md)、[移动端前提](../apps/mobile/README.md) |
| 审核测试证据 | [当前 CI 摘要](ci-evidence-r04.json)、[机器构建记录](r04-build-report.json)、[验证时间线](VALIDATION.md) |

## 安全与实现专题

| 专题 | 文档 |
|---|---|
| 整机安全边界 | [架构](ARCHITECTURE.md)、[R0.4 安全复查](SECURITY_REVIEW_R04.md)、[对抗性复核及未关闭风险](ADVERSARIAL_REVIEW_R04.md) |
| 指纹与授权 | [指纹协议](BIOMETRIC_PROTOCOL.md)、[SPI / 运行时核查](FPC_RUNTIME_REVIEW.md)、[一次请求执行范围](EXECUTION_SCOPE_REVIEW.md) |
| FIDO 与密钥 | [认证核心核查](AUTHENTICATOR_CORE_REVIEW.md)、[SE 状态和密钥模块](../crates/se-state/README.md) |
| SE 通信与持久性 | [I2C 核查](SE_I2C_REVIEW.md)、[持久实现约束](SE_PERSISTENCE_REQUIREMENTS.md)、[SCP03](../crates/scp03/README.md)、[T=1](../crates/se-transport/README.md) |
| MCU 密码与内存访问 | [H573 密码核查](H573_CRYPTO_REVIEW.md)、[Secure MMIO 边界](../firmware/secure-mmio/README.md) |
| 加密存储 | [快照事务](../crates/device-vault/README.md)、[逻辑记录](../crates/device-records/README.md)、[导入格式](IMPORT_FORMAT.md) |
| 测试与供应链 | [安全测试计划](SECURITY_TEST_PLAN.md)、[依赖审计](dependency-audit.json)、[审计来源](dependency-audit-provenance.json) |
| 开源参考与许可 | [方案比较](OPEN_SOURCE_REVIEW.md)、[参考版本](../references.lock.tsv)、[第三方说明](THIRD_PARTY_NOTICES.txt) |

## 当前资料与历史记录

- **当前设计**：[R0.4 硬件](../hardware/r04/README.md)、[当前状态](DEVELOPMENT_STATUS.md)、[制造任务书](../manufacturing/HANDOFF.md)。
- **验证时间线**：[VALIDATION](VALIDATION.md) 保留历次结果；其中旧测试数量、旧硬件状态只适用于对应提交。
- **历史版本**：[R0.3 硬件](../hardware/r03/README.md)、[R0.2 硬件说明](../hardware/HISTORY_R02.md)、[R0.3 安全复查](SECURITY_REVIEW_R03.md)、[旧分档比较](TIERS.md)、[旧构建记录](build-report.json)。

固定引脚和 CAD 文件以 R0.4 的清单为准。专项文档说明各模块的实现和限制，不能代替整机生产放行。
