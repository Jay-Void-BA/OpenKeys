# 文档导航

[首页](../README.md) · [当前状态](DEVELOPMENT_STATUS.md) · [开发起步](GETTING_STARTED.md)

当前 **R0.5 工程候选**：六位设备 PIN、十键、DFR0555 双行 LCD。目标功能、候选源码、模型测试和实物验证是不同状态。

| 目的 | 资料 |
|---|---|
| 第一次阅读 | [状态](DEVELOPMENT_STATUS.md)、[架构](ARCHITECTURE.md)、[术语](GLOSSARY.md) |
| 经典版优先 / 后量子版隔离 | [分版、功能范围与同步规则](EDITIONS.md)、[版本元数据](../edition.json)；[双 SE 评估](DUAL_SE_REVIEW_R05.md)仅属历史研究 |
| 审核安全 / PIN 缓存 | [R05 安全复查](SECURITY_REVIEW_R05.md)、[会话规则与测试映射](PIN_SESSIONS_R05.md)、[SE 原生 PIN 候选](SE_NATIVE_PIN_FEASIBILITY_R05.md)、[样板会话探针](NATIVE_UID_QUALIFICATION_R05.md) |
| 电路、板子与固件 | [经典 SE / 无电池供电修订](CLASSICAL_SE_POWER_R05.md)、[长 APDU 边界](CLASSICAL_APDU_R05.md)、[R05 硬件](../hardware/r05/README.md)、[固件导航](../firmware/README.md)、[R05 目标](../firmware/r05/README.md) |
| 从原材料到打样、代焊和测试 | [R05 制作手册](../manufacturing/BUILD_R05_ZH.md)、[制造入口](../manufacturing/README.md) |
| 境内供应与个人小批量 | [料号与渠道](../manufacturing/MAINLAND_COMPONENT_CHANNELS.md)、[机构筛选](../manufacturing/MAINLAND_SUPPLIERS_AND_LABS.md)、[委托确认](../manufacturing/REQUEST_FOR_CONFIRMATION.md) |
| 管理器 / 手机 | [GPUI](../apps/manager-gpui/README.md)、[移动平台约束](../apps/mobile/README.md)、[现有 notes 导入格式](IMPORT_FORMAT.md) |
| 上游方案与许可 | [四类仓库研究](UPSTREAM_REVIEW_R05.md)、[锁定版本](../references-r05.lock.json)、[DFR0555 来源](DFR0555_SOURCE.md)、[第三方说明](THIRD_PARTY_NOTICES.txt) |
| 共用实现审查 | [FIDO](AUTHENTICATOR_CORE_REVIEW.md)、[SE 持久性](SE_PERSISTENCE_REQUIREMENTS.md)、[I2C](SE_I2C_REVIEW.md)、[H573 密码](H573_CRYPTO_REVIEW.md)、[MMIO](../firmware/secure-mmio/README.md) |

历史 R04 的[手册](../manufacturing/BUILD_MANUAL_ZH.md)、[安全复核](ADVERSARIAL_REVIEW_R04.md)、[CI](ci-evidence-r04.json)、[开发包](NATIVE_BUILDS.md)只适用于对应版本，不代表 R05 通过。更早硬件从[硬件历史入口](../hardware/README.md)查阅。不要把旧指纹 / 四键的引脚、BOM、壳体或镜像用于 R05。
