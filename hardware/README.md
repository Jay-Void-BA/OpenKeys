> **R0.2 历史硬件资料**：当前电气PCB及制造数据位于 [hardware/r04](r04/README.md)。下文 USB-A、OLED、OTP 和 PIN 备用不再属于当前需求。

# OpenKeys R0 硬件交付

版本：0.2 design review，2026-11-08。此包可用于电路/结构评审与打样准备；不应直接下单生产。尚未布线，未制造、烧录、调谐或测量。安全要求见 [发布门槛](RELEASE_GATES.md)。

## 文件

| 文件 | 用法 |
|---|---|
| `kicad/OpenKeys.kicad_pro` | KiCad 9 项目，打开 `OpenKeys.kicad_sch` 查看全部 7 页 |
| `kicad/OpenKeys.kicad_sym`、`sym-lib-table` | 固定库符号，原理图亦嵌入副本，可移植打开 |
| `exports/OpenKeys-schematic.pdf` | 全部原理图，A3 矢量 PDF |
| `exports/schematic/*.svg` | 每页独立矢量预览 |
| `BOM.csv`、`pinout.csv` | 位号/封装状态、MCU 真实 LQFP64 引脚及复用 |
| `kicad/OpenKeys-floorplan.kicad_pcb` | 38 × 82 × 1.0 mm 四层板分区，安装孔和 MCU 封装；**未布线** |
| `exports/OpenKeys-dimensions.svg` | 44 × 88 × 16 mm 机身三视/剖面与部件占位尺寸 |
| `mechanical/OpenKeys.scad` | 参数化底壳/上盖/按键/装配；尺寸单位 mm |
| `exports/OpenKeys-{base,lid,plungers}.stl` | 3D 打印样件，上盖正面朝打印平台，未经实物公差核对 |
| `mechanical/OpenKeys-assembly.blend` | 可编辑着色装配场景；芯片/模块为标明的包络 |
| `exports/OpenKeys-assembly.png` | 外观模型预览，不是已制造实物照片 |
| `review/erc.json`、`review/netlist-check.json` | 实际检查原始记录 |

板分区是器件空间规划，不包含完整元件布局、走线、阻抗、层叠材料或 Gerber。USB-A/NFC/天线的 `PENDING` 封装仍未伪造；指纹目标已选FPC2534AM，SE050焊盘依据SOT1969-1而非通用QFN。不能通过删除警告来把它变成制造包。

真实成品缺口和价格取舍见[制造交付入口](../manufacturing/README.md)。

## 重建

需要 KiCad 9 的官方符号/封装库、Python 3、OpenSCAD；渲染另需 Blender 4.3 和 DejaVu Sans 字体。Debian 的 `pcbnew` 通常只在 `/usr/bin/python3` 可用。

```sh
/usr/bin/python3 hardware/tools/footprints.py
python3 hardware/tools/schematic.py
/usr/bin/python3 hardware/tools/mechanical.py
sh hardware/tools/export.sh
# 可选，重新生成装配渲染：
blender -b -t 2 -P hardware/tools/render.py
```

生成脚本基于此版本库符号；升级 KiCad 库后必须审阅差异。OpenSCAD `part` 可设为 `base`、`lid`、`plungers`、`assembly`、`exploded`。密钥与生产熔丝状态不属于图纸的一部分，不执行任何烧录操作。
