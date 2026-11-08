# R0.3 电路与结构交付

本目录是当前活跃的工程候选设计。原 `hardware/` 根目录下的 R0.2 文件保留作历史依据；不能混用其 USB-A/OLED/OTP 电路。当前文件可审查、可编辑和重生成，**尚不满足生产制造放行**。

- [8 页电路 PDF](exports/OpenKeys-schematic.pdf)、[KiCad](kicad/OpenKeys.kicad_sch)、[BOM](BOM.csv)、[引脚表](pinout.csv)。115 个 BOM 位号。
- [结构尺寸 PDF](exports/OpenKeys-dimensions.pdf)、[SVG](exports/OpenKeys-dimensions.svg)、[OpenSCAD](mechanical/OpenKeys.scad)、[3D 图](exports/OpenKeys-assembly.png)、[STL](exports)、[Blender](mechanical/OpenKeys-assembly.blend)。图是结构渲染，未拍摄实物。
- [PCB 放置研究](kicad/OpenKeys-floorplan.kicad_pcb)：68×92×1mm、4 层、RF 隔离区域及部分关键封装；无完整放置、电气网络和布线，未提供冒充制造包的空 Gerber。
- [ERC](review/erc.json)：0 电气错误、2 RF 封装警告；[31 项 XML 网表检查](review/netlist-check.json)。封装警告是制造阻断项，不是已解决的“允许警告”。

## 核心与授权

STM32H573RIT6 + STiRoT/TrustZone + SE050E，FPC2534AM 认证指纹接口，英文 16×2 白背光 LCD，四实体键。指纹验证必须先通过，PIN 只作附加验证；SE_EN 电路只提供供电/复位互锁，不负责判断指纹。

必须补齐安全启动配置、SE 对象权限/认证会话、受保护指纹计数与配对、可信 UI 驱动和实际操作网关，再测硬件归属及旁路。当前没有可据此烧录的完整认证器固件。

## 电源与电池

唯一外部数据口为 USB-C 母口，承载 USB2 FS 与充电。CC1/CC2 各 5.1kΩ Rd，无 PD 控制器；Apple 20W USB-C 适配器只使用 5V 档，合规 C-C 线即可，目标设备并不吸取 20W。整机输入硬件档位为 100mA，5V 时约 **0.50W 输入预算**；BQ25185 数据表给出的典型输入限流约 90mA，实际还受误差、线损和系统负载影响。

BQ25185 的 24kΩ ILIM/VSET 电阻选择 **4.2V/100mA** 档；3.00kΩ ISET 对应 100mA 快充目标，电池端最多约 4.2V×0.1A=**0.42W 目标上界**。系统优先供电，背光/指纹运行时电池充电会更慢；不能把输入功率当成实际电池充电功率。4.1V 不是该芯片的 100mA 输入组合，未伪造这个档位。

目标电池为约 500mAh、带过充/过放/过流保护和真实 10k NTC 的 1S 锂电包，按 32×37×6mm 预留含保护板空间。100mA 目标约为 0.2C，降低充电应力；**尚未冻结可采购包型号、NTC 曲线、容量/保护阈值和寿命报告**，不可拿裸电芯代替。热、鼓胀余量、绝缘、拉力和连接器极性需样件验收。不能承诺某年限或循环数。

CE 上电默认为禁止充电。主开关 ON 后，后台充电服务在不解锁任何秘密的情况下管理充电；OFF 时保留备用防拆电源但不启动充电。USB suspend 必须禁充、关背光和进入低功耗，实测 VBUS 总电流；不能只设置一个充电电阻就声称符合完整 USB 规范。Apple 适配器、Mac/PC USB-C、合规 A-C 线和低电启动须分别测。

白色背光通过 33Ω 和低侧 MOSFET 驱动，典型约 10mA，默认关闭；LCD 逻辑有专用 3.0V 稳压，避免 3.3V 主电源正偏差超出 LCD 额定逻辑电压。

## 防拆、抗扰与物理边界

TPS3702CX33 的 SET 高档标称欠压 3.17V、过压 3.43V，输出按原厂定义为开漏，直接控制 NRST；机壳闭合触点经开漏缓冲独立控制复位。复位与 MCU 请求共同门控 SE_EN。外部监测存在传播/RC 延迟，不能证明能拦截快 VCAP 毛刺、时钟/EM/激光故障或正在 SE 内部进行的运算。

MCP1700 独立 3.0V 供给 H573 VBAT 与触点，LSE 晶振提供备用域时钟。关主开关仍需保留 TAMP 配置/留痕；备用域完全丢失必须锁定，不能重新开放工厂初始化。需核对切换时序、Ioff/漏电、输入阈值和 TAMP 寄存器行为。简单触点仍可被短接，未实现经验证的主动防拆网格，也没有芯片开封级防护证明。

RF 窗为壳体 y≥76mm / PCB y≥73mm，不能放电池、金属、螺丝或无关铜面。FM11NC08S 实际 EP 焊盘与天线均未冻结；最终装配的 L/Q/谐振、场强、手机兼容性需量测。NFC 的 LCD/指纹流程依赖电池，不能假定 RF 采能足够。

结构约 74×98×20mm，属于小型手持器，不是银行卡厚度。60–100g 仅为重量预算；实际模块、材料与装配后称重才是交付值。LCD/指纹排线、接口开孔和盖板触点行程尚需实物配合。

## 重生成与来源

```sh
hardware/r03/tools/export.sh
blender -b -t 2 -P hardware/r03/tools/render.py
```

依赖 KiCad9、OpenSCAD、带 pcbnew 的系统 Python；PDF/SVG 源文件全部保留。工程脚本不进行烧录、熔丝写入或生产授权。

核对依据：[H573](https://www.st.com/resource/en/datasheet/stm32h573ri.pdf)、[STiRoT](https://wiki.st.com/stm32mcu/wiki/Security%3ASTiRoT_for_STM32H5)、[Newhaven LCD](https://newhavendisplay.com/content/specs/NHD-C0216CiZ-FSW-FBW-3V3.pdf)、[BQ25185](https://www.ti.com/lit/ds/symlink/bq25185.pdf)、[TPS3702](https://www.ti.com/lit/ds/symlink/tps3702.pdf)、[MCP1700](https://ww1.microchip.com/downloads/en/DeviceDoc/MCP1700-Data-Sheet-20001826F.pdf)、[FPC](https://www.fpc.com/media/legj15fr/spc27317-6-fpc2530-product-specification.pdf)。PDF 不重新分发；事实摘要、散列和链接保留在 review。
