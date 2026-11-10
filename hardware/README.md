# 硬件导航

[返回首页](../README.md) · [文档导航](../docs/README.md) · [制造入口](../manufacturing/README.md)

## 当前版本：R0.4

查看 [R0.4 硬件说明](r04/README.md)，其中包含实际电路 / PCB、BOM、引脚、板层、天线、机械坐标和导出工具说明。

| 我需要 | 文件 |
|---|---|
| 原理图 | [10 页 PDF](r04/exports/OpenKeys-schematic.pdf)、[KiCad 源文件](r04/kicad/OpenKeys.kicad_sch) |
| PCB 与备料 | [KiCad PCB](r04/kicad/OpenKeys.kicad_pcb)、[BOM](r04/BOM.csv)、[引脚表](r04/pinout.csv) |
| 制造文件 | [Gerber / 钻孔 ZIP](r04/exports/OpenKeys-fabrication.zip)、[贴片坐标](r04/exports/OpenKeys-SMD-position.csv) |
| 外壳与装配 | [尺寸 PDF](r04/exports/OpenKeys-dimensions.pdf)、[结构源](r04/mechanical/enclosure.scad)、[STL / STEP 导出](r04/exports) |
| 检查证据 | [ERC](r04/review/erc.json)、[DRC](r04/review/routed-drc.json)、[独立板网核对](r04/review/board-check.json)、[CAD 摘要](r04/review/cad-manifest.json) |
| 样板验收 | [硬件步骤](../manufacturing/HARDWARE_ACCEPTANCE.md)、[完整安全测试计划](../docs/SECURITY_TEST_PLAN.md) |

本版是工程候选。连通性和规则检查不能代替首板、RF、电源、防拆或物理攻击验证；完整认证固件仍在开发。

## 历史版本

| 版本 | 位置 | 使用范围 |
|---|---|---|
| R0.3 | [旧硬件说明](r03/README.md)及 `r03/` | 历史电路与未完成 PCB 阶段 |
| R0.2 | [原硬件说明](HISTORY_R02.md)及本目录旧 `kicad/`、`exports/`、`mechanical/`、`tools/` | 旧 USB-A / OLED / OTP / PIN 备用方案，仅供追溯 |

历史文件保留原路径以便追溯，不与 R0.4 的制造包或引脚表混用。重新导出当前板时遵循 R0.4 工具说明，不能运行旧版生成脚本覆盖当前板。
