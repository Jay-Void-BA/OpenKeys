# 制造与验收

[首页](../README.md) · [文档导航](../docs/README.md) · [当前状态](../docs/DEVELOPMENT_STATUS.md)

**当前生产放行：BLOCKED。** R0.4 已有电路、完整布线和制造导出；完整认证固件、真实管理通道、生产配置与实物资格仍未完成。

第一次制作先看[从原材料到验收的完整说明书](BUILD_MANUAL_ZH.md)或[11 页打印版 PDF](OpenKeys-R04-build-manual-zh.pdf)。需要直接和工厂沟通时使用[工程订购 / 代焊确认单](ORDER_SHEET_R04.md)，并附[板上采购表](procurement-r04-board.csv)与[板外采购表](procurement-r04-external.csv)。

## 按步骤准备

| 步骤 | 资料 | 完成后应得到什么 |
|---|---|---|
| 1. 明确委托范围 | [任务书](HANDOFF.md)、[未关闭风险](../docs/ADVERSARIAL_REVIEW_R04.md) | 明确工程样板、源码评审、物理测试和三把成品分别交付什么 |
| 2. 确认个人、小批量受理 | [候选与证据](MAINLAND_SUPPLIERS_AND_LABS.md)、[确认模板](REQUEST_FOR_CONFIRMATION.md) | 机构对“个人合同 / 付款、3 把项目、具体范围”的书面确认 |
| 3. 工程审核 / 备料 | [R0.4 硬件](../hardware/r04/README.md)、[关键器件](critical-parts.csv) | DFM、物料 / 数量、后焊、结构及测试报价；安全器件不擅自替换 |
| 4. 首块样板 | [硬件验收](HARDWARE_ACCEPTANCE.md) | 上电、电源、NTC、USB、NFC、结构和实际测量记录 |
| 5. 安全与互通验证 | [测试计划](../docs/SECURITY_TEST_PLAN.md) | 明确版本、样品和范围的功能 / 负向 / SCA / FI / PAD / EMC 证据及修复复测 |
| 6. 三把成品验收 | [逐台记录要求](HARDWARE_ACCEPTANCE.md)、[交付预检](readiness-report.json) | K01 / K02 / K03 各自完整证据与人工放行 |

个人客户与 3 把数量是服务商筛选的必要条件。技术能力匹配、公开小批量服务和个人账号注册均不能替代本项目接单确认；目前已向 5 家提交资格询问，尚未取得这样的书面回复。见[联系进度](OUTREACH_STATUS.md)。

## 给工厂的文件

[R0.4 文件入口](../hardware/r04/README.md)集中提供 Gerber / 钻孔、BOM、坐标、原理图和结构源。使用固定提交与摘要，先做工程审核；安全配对与密钥注入是独立受控工序，不把生产密钥、指纹模板或真实恢复码交给普通代焊厂。

## 交付检查

```sh
python3 scripts/check-production-ready.py
```

该命令更新 `readiness-report.json`，当前应返回 **2 / BLOCKED**。缺少生产固件、签名更新、启动配置、正式管理器或实物证据时不放行。

`release-manifest.json` 只记录真实存在的文件和摘要；诊断 ELF、空占位或测试通过的库不能代替对应成品。自动预检不烧录、不注入密钥，也不是最终生产批准。

## 采购表与打印手册维护

普通阅读不需要安装 PDF 生成工具。下面的检查只使用 Python 标准库：

```sh
python3 scripts/export-manufacturing-worksheet.py --check
python3 scripts/export-build-manual.py --check
```

采购表由原始 BOM / CPL 分类生成；带报价的填写副本私下另存，不覆盖仓库模板。PDF 的[来源清单](build-manual-manifest.json)绑定手册、工厂确认单、采购模板、生成脚本和原始制造文件。

需要重生成时先运行 `python3 scripts/export-manufacturing-worksheet.py`。PDF 使用 Pandoc、Pango、Noto CJK 字体及[锁定的 Python 文档依赖](manual-renderer-requirements.txt)；在独立 Python 环境安装这些依赖后运行 `python scripts/export-build-manual.py`，再检查中文、表格和分页。仅核对摘要不等于完成排版或硬件验收。
