# 制作与验收 — R0.5

[首页](../README.md) · [当前状态](../docs/DEVELOPMENT_STATUS.md)

**生产放行 BLOCKED。** 当前 R05 已有电路、布线和制造导出；完整认证固件、真实管理器后端、生产配置和实物安全资格仍未完成。

从 **[R05 完整制作手册](BUILD_R05_ZH.md)** 开始，依次准备材料、确认个人小批量受理、DFM、订板、代焊、线束、首板、功能与安全测试，再决定三把最终密钥。诊断镜像不能替代生产认证固件。

| 需要 | 资料 |
|---|---|
| 当前打样文件 | [R05 硬件、BOM、CPL、Gerber 和结构](../hardware/r05/README.md)、[R05 关键器件表](critical-parts-r05.csv) |
| 境内物料 | [具体渠道与 MOQ 限制](MAINLAND_COMPONENT_CHANNELS.md)、[DFR0555 现货记录](../docs/DFR0555_SOURCE.md) |
| 工厂 / 实验室 | [支持个人 / 小批量的资格筛选](MAINLAND_SUPPLIERS_AND_LABS.md)、[确认模板](REQUEST_FOR_CONFIRMATION.md)、[既有联系记录](OUTREACH_STATUS.md) |
| 安全测试 | [R05 手册测试步骤](BUILD_R05_ZH.md)、[备份 / 防拆 / 采样](BACKUP_TAMPER_TESTS_R05_ZH.md)、[Root 存储](ROOT_STORAGE_TESTS_R05_ZH.md)、[PIN / NFC 会话测试](../docs/PIN_SESSIONS_R05.md)、[未关闭风险](../docs/SECURITY_REVIEW_R05.md) |
| PIN 公开对象布局 | [原生 UserID v4 schema](se-native-pin-layout-r05.json)，不含真实秘密；旧 HKDF v2 仅供历史回归 |

公开广告中的“小批量”、个人账号注册和技术能力，不等于机构已书面接受你这个个人三把项目。现有联系记录不能视作新版本报价或接单确认；本次没有发送新询问或下单。

历史 R04 的 [PDF 手册](OpenKeys-R04-build-manual-zh.pdf)、[订购确认单](ORDER_SHEET_R04.md)、采购表和 [预检报告](readiness-report.json)仅供追溯，包含指纹和旧 LCD，不得照单用于 R05。`check-production-ready.py` 仍针对历史 R04，不能用于宣称 R05 放行。
