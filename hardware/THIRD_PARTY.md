# 硬件来源、许可与数据手册

本目录自作设计与工具以CERN-OHL-S-2.0发布。主要软件仍为根目录MIT许可。

- 原理图嵌入KiCad 9.0.2发行版的标准符号，并引用标准封装，库采用[CC-BY-SA-4.0及设计例外](https://www.kicad.org/libraries/license/)。通用Flash符号使用W25Q32JVSS脚位表示经核对兼容的W25Q64JVSSIQ，实际BOM值为64Mbit，封装仍须制造核对。
- SoloKeys [solo2-hw](https://github.com/solokeys/solo2-hw/tree/e7d964cb1ce775b1faa1c0bd1e694b6a124aa630)（CERN-OHL-S-2.0）作为FM11NC08原理符号引脚检查和工程组织参考。未把Solo2原PCB/天线作为本项目制造文件；其上游本身缺少完整NFC天线交付。
- 结构生成脚本和OpenSCAD/Blender模型为本项目新作；Blender场景中的文字使用[DejaVu Sans](https://dejavu-fonts.github.io/License.html)，没有重新分发字体文件。标注的模块几何为设计包络，不是原厂STEP模型。
- 厂商PDF不重新分发；下表链接供审查。数据手册中的典型应用不构成整机安全/性能担保。

| 器件 | 来源 |
|---|---|
| STM32U585RI | https://www.st.com/resource/en/datasheet/stm32u585ri.pdf |
| STM32U585安全指南 | https://www.st.com/resource/en/user_manual/um2852-stm32u585xx-security-guidance-for-psa-certified-level-3-with-sesip-profile-stmicroelectronics.pdf |
| SE050 | https://www.nxp.com/docs/en/data-sheet/SE050-DATASHEET.pdf |
| SE050 APDU | https://www.nxp.com/docs/en/application-note/AN12413.pdf |
| TPS2116 | https://www.ti.com/lit/ds/symlink/tps2116.pdf |
| BQ24074 | https://www.ti.com/lit/ds/symlink/bq24074.pdf |
| TPS63031 | https://www.ti.com/lit/ds/symlink/tps63030.pdf |
| TS3USB30E | https://www.ti.com/lit/ds/symlink/ts3usb30e.pdf |
| RV-3028-C7 | https://www.microcrystal.com/fileadmin/Media/Products/RTC/Datasheet/RV-3028-C7.pdf |
| FM11NC08 | https://www.fm-chips.com/fm11nc08-15403638655782941.html |

审查时下载的PDF SHA-256与UTC日期见review/datasheets.json，PDF未放入Git。安全设计必须结合最新勘误、已知问题和采购修订再次核对。

R0.2新增：SE050实际封装使用[NXP SOT1969-1](https://www.nxp.com/docs/en/package-information/SOT1969-1.pdf)第4–6页的尺寸与钢网事实自行生成：0.4mm节距、0.725×0.2mm焊盘、1.9mm EP、四个0.75mm钢网开口。FPC资料/接口交叉检查与MIT参考声明见[指纹设计](../docs/BIOMETRIC_PROTOCOL.md)。FPC2534AM、SE050内部固件仍非本项目开放源码。
