# OpenKeys R0.5：PIN＋十键

[项目首页](../../README.md) · [状态](../../docs/DEVELOPMENT_STATUS.md) · [安全复查](../../docs/SECURITY_REVIEW_R05.md) · [制作与测试](../../manufacturing/BUILD_R05_ZH.md)

本版正式取消指纹。身份验证使用设备本地 **6 位数字 PIN**，0–9 十个独立实体键；电脑和手机均不输入设备 PIN。保留 FIDO2 / U2F / hmac-secret、可信请求确认、凭据管理和加密 notes / recovery codes 的目标范围。新增 TOTP / HOTP、OpenPGP / PIV / SSH 目标；动态验证码仅钥匙 LCD 显示，种子不可导出。

**工程样板候选，生产放行仍为 BLOCKED。** CAD 检查、Rust 模型测试与诊断固件不能替代生产 Root、实片配置和物理安全验收。不得给三把最终密钥写入真实账户或恢复码。

## 文件

| 内容 | 入口 |
|---|---|
| 整机尺寸与真实 PCB 线条 | [装配图 PDF](exports/OpenKeys-dimensions.pdf)、[PNG](exports/OpenKeys-dimensions.png)、[坐标 JSON](mechanical/dimensions.json) |
| 电路 | [10 页原理图 PDF](exports/OpenKeys-schematic.pdf)、[KiCad 源](kicad/OpenKeys.kicad_sch)、[网表](exports/OpenKeys-netlist.xml) |
| PCB | [四层板源](kicad/OpenKeys.kicad_pcb)、[正面](exports/OpenKeys-PCB-front.svg)、[背面](exports/OpenKeys-PCB-back.svg) |
| 工程打样资料 | [Gerber / 钻孔](exports/OpenKeys-fabrication.zip)、[BOM](BOM.csv)、[贴片坐标](exports/OpenKeys-SMD-position.csv)、[引脚表](pinout.csv) |
| 结构 | [OpenSCAD](mechanical/enclosure.scad)、[板体 STEP](exports/OpenKeys-PCB.step)、[结构导出](exports) |
| 检查 | [ERC](review/erc.json)、[DRC](review/routed-drc.json)、[独立网表核对](review/netlist-check.json)、[板网核对](review/board-check.json)、[摘要](review/cad-manifest.json) |
| 模拟范围 | [天线 / 复位 RC](review/simulation.json)、[输入保护直流角落](review/protection-calculations.json)、[LCD 断电假设模型](review/display-power-model.json) |

硬件 PIN 与协议 PIN 分别按[同一认证会话的 30 秒规则](../../docs/PIN_SESSIONS_R05.md)缓存；断连 / NFC 离场立即锁定。改硬件 PIN 或第 11 次完整错误须清空全部数据；当前仅擦除锁存，完整销毁服务待集成。

## LCD 采购与兼容性

选用 **DFRobot DFR0555**：16×2 字符 LCD、蓝底白字、可调背光、I2C、标称 3.3–5V、模块外形 87×32×13mm。2026-11-10 中国大陆[官方商城](https://www.dfrobot.com.cn/goods-1697.html)页面标价 **¥55，显示“有货”**。这是页面核验，未预订三件，也不能保证下单时的库存或发货时间。先确认版本、境内发货和同批数量，再买一个尺寸样品。

[官方文档](https://wiki.dfrobot.com/dfr0555/docs/24390)区分 V1.0 与 V1.1：字符地址均为七位 `0x3E`，背光分别为 `0x60`、`0x6B`。BOM 固定 V1.1；若收到 V1.0，必须改用明确标注的 V1.0 诊断镜像并重新冻结配置，禁止自动探测降级。Rust 驱动见 [dfr0555.rs](../../firmware/board/src/dfr0555.rs)，参考寄存器来源见[来源记录](../../docs/DFR0555_SOURCE.md)。

模块无外部复位脚，因此增加 U15–U21：复位 / PC3 异步清除电源锁存、TPS22919 切断 VCC，双模拟开关同时隔离 SCL/SDA。复位释放后不会自行恢复 LCD 供电，必须有新的 PC4 上升沿。1k 泄放电阻与 500ms 关断保持用于清除残留电荷；模型依赖“模块＋板上总电容≤100µF、无注入”等**待测条件**。背光熄灭本身不等于文字消失。

供电取自 **3.3V 标称**主电源。厂家网页写“3.3–5V”不能证明本板在稳压误差、线损、温度、背光负载下始终满足工作条件。必须取得厂商保证或实测后调整电源；这是未关闭的电气交付条件。不能接 5V 到 MCU / SE 总线。

## 布局与按键

主板本体 96×112×1.0mm，USB 小舌部伸出 2.5mm，总包络 96×114.5mm。外壳候选 102×118×30mm。较大的现货 LCD 需要更大外壳，R0.4 的外壳不兼容。

| 场景 | 实体按键功能 |
|---|---|
| 请求到达，尚未授权 | 5 开始本地 PIN；0 取消。仅接收主机请求不扣 PIN 次数 |
| 输入 PIN | 短按 0–9 输入；长按 9 删除一位；长按 0 取消；只显示星号，满 6 位提交 |
| 验证后审阅请求 | 2 上一页，8 下一页，5 确认，0 取消；完整身份各页写入成功后才能确认 |
| 换场景 / 故障 | 丢弃按住的键，要求全部释放；复位、时钟回退、超时、断连撤销授权 |

采用专用 GPIO，没有键盘矩阵和二极管鬼键问题。25ms 去抖，800ms 长按，只在稳定释放时产生一次事件；组合键、滚键、过长按住和轮询间断不产生确认。[实际 MCU 引脚](pinout.csv)与[目标固件构建约束](../../firmware/r05/build.rs)共同核对，禁止向 R0.4 板烧录 R0.5 镜像。

## 电源、NFC 与制造边界

只有一个外部 USB-C 母口，USB 2.0 数据兼 5V 充电；没有 USB-A。可用合规 USB-C 线接 Apple 20W 充电器，不申请 PD 的 9V 档。设计输入 100mA、约 **0.50W**；电池充电目标 100mA、最高约 **0.42W**，还会受系统负载和输入限流影响。当前诊断镜像保持充电禁止，并非可充电成品固件。

保护型 1S 500mAh 电池须有真实贴附的 10k NTC，先测连接器极性。NFC 沿板左侧布置，不在绕组背后放电池、金属或地平面；LCD 和 PIN 操作需要电池供电且主开关开启，未设计成纯 NFC 取电。旋转后的线圈仍须在实际外壳、电池、手持状态下重新调谐和做手机测试。

J7 是内部 LCD 线束，J2/J6 为内部电池 / NTC；J3 为生产 DNP 的内部调试位置，均不应暴露为第二外部接口。原理图的监督器、SE 使能门和壳体触点提供故障处理路径，不能保证任意短脉冲、桥接、激光或探针攻击均被阻断。

## 重建与核对

使用 KiCad 9、Python 3 / pcbnew，附加导出使用 CairoSVG、OpenSCAD、ngspice 47。日常检查运行 `tools/export.sh`，核对**保存的已布线板**；不要默认重建 / 自动布线。`board.py` 无参数会覆盖布局，只有修改硬件并另行评审时使用。自动路由器不是 CI 的组成部分。

许可：[CERN-OHL-S-2.0](../LICENSE-CERN-OHL-S-2.0.txt)。模型和图纸均标为工程候选；尺寸试装、DFM、EMC / ESD、故障注入和侧信道的实物结果仍为 NOT_RUN。
