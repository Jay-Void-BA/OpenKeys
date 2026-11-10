# R0.4 元器件与结构件：境内采购渠道

> **版本更新：R0.5 已正式取消指纹、改为十键和 DFR0555 LCD。** 当前屏幕从 [DFRobot 中国商城](https://www.dfrobot.com.cn/goods-1697.html)核验为 ¥55、显示有货（2026-11-10，未下单）。R0.5 以[新版制作说明](BUILD_R05_ZH.md)和[新版 BOM](../hardware/r05/BOM.csv)为准。下表旧指纹 / Newhaven LCD / 外壳仅属 R0.4 历史采购表，不要购买来混装。

[制造入口](README.md) · [完整制作说明](BUILD_MANUAL_ZH.md) · [板上采购表](procurement-r04-board.csv) · [板外采购表](procurement-r04-external.csv)

核对日期：**2026-11-10**。适用于先做 1 块工程样板、最终制作 3 把的采购评审。当前生产放行仍为 **BLOCKED**；本页不改变硬件、固件或安全资格状态，也没有下单、锁库存或取得专项报价。

**可找到境内采购入口，但尚未核实全套物料均可从大陆现货按 1–3 件买齐。** 特别是指纹模块、LCD 和电池。国内公司销售、人民币结算、内地交货、境内仓现货是不同条件；中文网站也可能从境外采购。

## 先向谁采购

| 渠道 | 本项目适合采购什么 | 个人与数量依据 | 发货 / 采购限制 |
|---|---|---|---|
| **立创商城** | 主控、Flash、充电 / 保护芯片、连接器、普通阻容；先用完整 MPN 配单 | [购物流程](https://www.szlcsc.com/help/102.html)列手机注册与在线结算；[平台协议](https://www.szlcsc.com/news/14918.html)包括个人，但该协议版本为 2020，订单仍以当前规则为准 | 商品页有目录不等于已有本地库存；逐项核对自营 / 合作库存 / 海外代购、MOQ 和交期 |
| **嘉立创 SMT 物料库** | 与贴片订单绑定的安全芯片、NFC、按键等 | 型号和库编号可作为国内 SMT 工程师的询价索引；个人 PCBA 与 1 块首件仍需确认，见[服务商筛选](MAINLAND_SUPPLIERS_AND_LABS.md) | 下述 JLCPCB 国际站页面明确该类库料仅供后续 PCBA，**不能单独寄散件**；不把国际站价格 / 库存自动视为国内站报价 |
| **云汉芯城** | 特殊模块 / LCD 的拆零询价或代采备选 | [FPC 品牌页内用户协议](https://www.ickey.cn/brand/34901.html)将自然人列入用户资格；但本次查到的两个特殊料条目 MOQ 超过本项目 | 指纹条目是 Future 供应、进口交付；LCD 条目虽标国内现货，却有 100 件 MOQ 和交付说明矛盾，须销售澄清 |
| **RS 欧时中国** | LCD 的企业采购候选，可交代焊厂询价 | 目录有单件计价；[2026 年 6 月销售条款](https://www.rsonline.cn/web/content/about-rs/articles/conditions)说明网站面向企业客户，因此**不列为已确认个人直购** | 请由代焊厂确认其企业采购资格、实际仓库及交期；没有本项目接单确认 |
| **华秋** | 备选整包 BOM / PCBA 代料询价 | [PCBA 入口](https://smt.hqchip.com/)；个人、1 块工程板和特殊料客供仍需书面确认 | 本次没有逐型号的现货 / MOQ 证据，不将其列为下面器件已确认有货的商家 |

## 对应当前 BOM 的型号与目录

以下为**找料索引**，不是已批准采购替代清单。数量是 3 把成品净用量，不含 E01、损耗和破坏试验。产品目录、封装名或推荐 CAD 模型不能替代原厂数据手册和本项目焊盘核对。

| 位号 / 部件 | 完整型号或采购候选 | 三把净用量 | 已找到的渠道 / 料号 | 必须确认 |
|---|---|---:|---|---|
| U4 主控 | STM32H573RIT6 | 3 | [立创 C22371377](https://item.szlcsc.com/23858910.html) | LQFP64、完整后缀、revision、来源；不换 H563 或其他封装 |
| U5 安全芯片 | SE050E2HQ1/Z01Z3Z | 3 | [LCSC C5190917](https://www.lcsc.com/product-detail/C5190917.html)；[JLCPCB 装配库](https://jlcpcb.com/partdetail/NXPSemicon-SE050E2HQ1Z01Z3Z/C5190917) | 国内零售 / 代料、库存和批次另核实；不能用 SE050F2 等“相似料”自动替换 |
| U6 Flash | W25Q64JVSSIQ | 3 | [立创 C179171](https://item.szlcsc.com/190565.html) | 完整型号及 208 mil 封装，不用相似后缀猜尺寸 |
| U7 NFC | FM11NC08S-DN-T-G | 3 | [JLCPCB C18185266](https://jlcpcb.com/partdetail/FM-FM11NC08S_DN_TG/C18185266)；国内另有 [FM11NC08S / C182885](https://item.szlcsc.com/194285.html) | C182885 省略订货后缀，仅作为询价线索；确认 SPI、DFN10、EP、原厂图纸及供应 revision，不能自动等同于指定料 |
| U2 充电 | BQ25185DLHR | 3 | [立创 C19725033](https://item.szlcsc.com/21048130.html) | WSON10 2.2×2 mm 与特殊 EP；不换充电模块 |
| U3 稳压 | TPS63031DSKR，待完整订货号冻结 | 3 | [立创 C15516](https://item.szlcsc.com/16195.html) | 与 BOM 中 TPS63031DSK 的 DSK 封装核对；R 为具体包装订货号，仍需工程批准 |
| U14 输入保护 | TPS259470LRPWR | 3 | [立创 C3662793](https://item.szlcsc.com/4230592.html) | 完整 L 后缀、HotRod 焊盘和保护配置 |
| U10 电压监控 | TPS3702CX33DDCR | 3 | [LCSC C2068031](https://www.lcsc.com/product-detail/C2068031.html) | 国内采购和库存另核实；不因同系列便宜改阈值 / 回差版本 |
| J5 指纹连接器 | FH12-12S-0.5SH(55) | 3 | [立创 C88360](https://item.szlcsc.com/89539.html) | 12P、0.5 mm、底接触、排线厚度；它不含指纹模块 |
| J1 USB-C 母座 | HRO TYPE-C-31-M-12 | 3 | [LCSC C165948](https://lcsc.com/product-detail/USB-Connectors_Korean-Hroparts-Elec_C165948.html)；[装配库](https://jlcpcb.com/partdetail/C165948) | 原厂尺寸 / 机械脚；MOQ 按结算，不能换外形相似母座 |
| SW2–SW6 | TL3342F160QG | 15 | [JLCPCB C2886898](https://jlcpcb.com/partdetail/eswitch-TL3342F160QG/C2886898) | 四键加开盖触点；包装与高度核对，不自动改为其他力度 / 高度 |
| NTC1 | Semitec 103AT-2 | 3 | [LCSC C5346323](https://www.lcsc.com/zh-CN/product-detail/ntc-thermistors_semitec-103at-2_C5346323.html) | 国内零售另核实；真实贴合电芯的 10 kΩ / B3435 热敏，线束另买 |
| FP1 指纹模块 | FPC2534AM 100026230 flex | 3 | [云汉 / Future 条目](https://www.ickey.cn/detail/100030101977445/100026230.html) | 本次页面快照起订 31 件，库存数字与 MOQ 不一致；显示进口交期。**不适合直接为 3 把下单**，只能询拆零 / 其他供货源 |
| U8 LCD | NHD-C0216CiZ-FSW-FBW-3V3 | 3 | [云汉条目](https://www.ickey.cn/detail/1003001016512110/NHD-C0216CIZ-FSW-FBW-3V3.html)；[RS 目录，264-4041](https://www.rsonline.cn/web/c/displays-optoelectronics/display-modules/lcd-monochrome-displays/?pn=7) | 云汉快照起订 100 件；RS 目录有单件计价但个人条件不满足默认筛选，直达详情本次无法读取。确认实际修订、控制器、白背光和接口 |

未列出的阻容、电感、晶体、开关、连接器和逻辑芯片，将[完整板上表](procurement-r04-board.csv)交立创 / 代焊厂配单，填入厂牌、完整 MPN、供应料号与数据手册后审核。**F1 完整型号及电流 / 温度 / I²t 协调仍未解决。** 所有采购候选尚未转为批准材料，不能仅靠商城“相似度”替代。

## 特殊件的可行采购路线

### 指纹模块

本次没有核实到同时满足“100026230 flex、可追溯、境内仓、1–3 个、个人购买”的供货记录。云汉确有对应原厂 / MPN 条目，但当前数量和进口交期不匹配。

如果接受境内人民币下单、正规进口交付，[得捷中国的 100026230](https://www.digikey.cn/zh/products/detail/fingerprint-cards-ab/100026230/25649789)提供单件阶梯和对应模块描述。它是补缺选项，**不冒充大陆仓现货**；页面中国交付说明涉及进口与 DDP，实际个人资格、供货和收货条件按订单确认。不要改买 100026223 AP / LGA，也不换普通 MATCH GPIO 指纹模块。

### 双行白背光 LCD

先请代焊厂按 RS **264-4041** 询企业采购和实际仓库；或询云汉能否为该型号拆零，未获得书面回复前不算可买 3 个。[得捷中国的对应 LCD](https://www.digikey.cn/zh/products/detail/newhaven-display-intl/NHD-C0216CIZ-FSW-FBW-3V3/2165872)也有单件阶梯，可作进口补缺。普通 5 V 1602 + 转接板不是本板可直接使用的替代品。

### 电池与线束

当前 BOM 的 **Adafruit 1578 保护型 500 mAh 包**未核实到大陆 1–3 包现货渠道。不能把通用“503035 / 500 mAh”标签当成同一获批电池包。

可以向[深圳格瑞普的聚合物 / PACK 服务](https://www.grepow.cn/ljhwdc.html)询问现有标准保护包样品；官网列有尺寸、电压、容量及 BMS / PACK 定制能力，但**没有确认接受本项目个人和 3 包委托**。因此只作工程询价备选，不推荐为三把直接启动定制。

更实际的首轮路径是请代工方寻找有追溯、可提供技术资料的现成保护包，再由工程师审核：1S、3.7 V 标称、4.2 V 充电、约 500 mAh、保护阈值、100 mA 充电条件、尺寸 / 公差、JST-PH2 极性、NTC 贴合、温度及寿命条件。当前电池安装预留约 32×40×6.5 mm；完整包、保护板、出线与膨胀余量都要核对。变更后更新采购表、结构和充电验收，未经审核不直接替换。

JST-PH 电池插头和 JST-SH 热敏线束由装配厂采购并压接，须对照 J2 / J6 实际引脚，不能只看红黑线色。不要让普通客供装配包含加热软包焊线。

## PCB、外壳和辅料

- PCB / 双面贴片：使用[工厂筛选与接单限制](MAINLAND_SUPPLIERS_AND_LABS.md)。先询个人项目、1 块工程首件、裸板 MOQ、后焊及客供模块。境内元件零售资格不能转移为整机服务确认。
- 外壳：可向[嘉立创 3D 打印](https://www.jlc-3dp.cn/placeOrder)上传 STL 询一套非金属工程壳；[官方流程](https://www.jlc.com/portal/server_guide_35104.html)说明按模型选择材料、数量、审核与下单。材料、公差、按键 / 触点行程仍要试装，不把打印服务等同于防拆认证。
- PC 窗、螺钉、绝缘与热敏贴附：按[板外表](procurement-r04-external.csv)由装配方配套。三份按钮柱打印件共 12 个柱；不要据通用 M2 名称猜螺钉长度，也不要在天线区添加金属固定件。

## 给客服的询价文字

> 我是个人客户，先做 1 块工程样板，后续制作 3 把。请按附件完整 MPN 报价，不自动替代。请逐项注明实际厂牌、完整订货号、MOQ / 包装、现货仓库所在地、是否进口、含税运到大陆地址总价、交期、批次及来源资料。特殊模块若需超过 3 件才能采购，请先说明能否拆零；不要按目录库存自动下单。若需企业资格或不接个人，请明确告知。

已填写的报价、个人地址和交易记录私下保存，不覆盖仓库的自动生成采购模板。本页记录公开目录的有限核查，未联系新的销售、未下单，也不是原厂授权链或产品真伪的独立鉴定。
