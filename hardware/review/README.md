# R0.2 交付检查

日期2026-11-08（北京时间）；工具KiCad 9.0.2、OpenSCAD 2021.01、Blender 4.3.2、Python 3。Linux容器没有连接目标硬件。

- 7页原理图实际导出PDF/SVG/XML网表，116个BOM位号。
- 原始[ERC](erc.json)：0错误、3个封装链接警告（J1 USB-A、U11 NFC封装、J5天线）。未修改规则来排除告警。
- [网表检查](netlist-check.json)：29项条件通过，包含指纹12pin的电源/ESD地/模式选择和SPI映射。数字检查不是芯片或RF测试。
- SE050独立封装按NXP SOT1969-1重新生成，20个0.4mm节距焊盘+1.9mm EP和四个0.75mm钢网窗口。PCB源仅增加该器件的空间占位，整体仍是未布线分区。
- 指纹目标选FPC2534AM；实际封装配合、供电、GCM、SE持久operation tag/失败计数及独立解锁验证未完成。详情见[协议设计](../../docs/BIOMETRIC_PROTOCOL.md)。
- 软件26项测试、fmt、clippy -D warnings通过；新增no_std策略在Cortex-M33目标检查通过。原生Linux开发版重新编译；设备后端依旧默认拒绝。
- 新Cargo.lock的RustSec扫描0条已知漏洞、0条维护警告；新AES-GCM仅作为测试依赖，生产代码尚无可用硬件GCM后端。
- 外壳STL仍为打印原型设计，未实物装配；像素预览/打印网格生成不能证明显示/指纹/电池的真实配合。

[制造预检](../../manufacturing/readiness-report.json)仍为BLOCKED。没有Gerber/贴片坐标、完整设备固件、iOS/NFC实测或侧信道/故障/EMC报告。文件完整性脚本也不会自动批准生产。

SHA256SUMS覆盖当前硬件源/导出物；厂商PDF只保留摘要与来源，不上传PDF正文。
