# 中国大陆制造与安全测试候选

公开资料核对日期：2026-11-09。以下是**能力匹配候选**，不是已接单、报价或对 OpenKeys 安全性的认可。没有替用户联系、发送项目或下单。服务页面会变化，签约前重新确认承接主体、实际测试地点、数量和范围。

## 制造厂

| 候选 | 公开能力与本项目适配 | 三把设备的询价重点 | 官方入口 |
|---|---|---|---|
| 嘉立创，中国大陆生产 | PCB、SMT、BOM备料、邮寄元器件；集团另有3D打印/CNC | 先询4层68×92×1mm板及3把装配；确认所选SE特殊QFN、客供FPC模块、字符LCD引脚/背光线、真实NTC和整机手工装配；不要把SMT报价当完整成品价 | [SMT](https://www.jlc-smt.com/)、[服务范围](https://www.jlcgroup.cn/products-service) |
| 华秋智造，深圳及大陆工厂 | 小/中/大批量PCBA、客供料、DIP插件，公开设备含AOI/X-ray | 对130个BOM位置整单报价，确认QFN底焊盘检查、LCD后焊、指纹排线及防拆触点装配；说明是否包含上电/电源测试 | [PCBA服务](https://smt.hqchip.com/)、[设备](https://smt.hqchip.com/equipment.html)、[联系](https://www.huaqiu.com/about/contactus) |
| Seeed Fusion／矽递，深圳 | 原型到批量PCBA、非OPL料及手工通孔装配；官网有大陆业务联络 | 公开FAQ写PCB最少5片、装配可少至1片，可询“5裸板、只装3把”；该FAQ最后修改于2021年，务必以当前书面报价为准。客供料需要单独确认 | [装配服务](https://www.seeedstudio.com/pcb-assembly.html)、[数量与客供FAQ](https://support.seeed.cc/portal/en/kb/articles/seeed-fusion-pcb-assembly-service-quick-questions)、[联系](https://www.seeed.cc/contacts) |
| PCBWay，大陆制造服务 | 客供/全代料/混合代料、SMT+通孔、QFN X-ray、按客户程序功能测试及box build | 本次核对能力页写**最低5片装配**，不假定接受3片；本板92mm边小于其列示100mm最小尺寸，需工程边/拼板审核。问清多余板、器件和整机装配费用 | [能力及数量](https://www.pcbway.com/pcb_prototype/PCB_assembly_Capabilities.html) |

优先拿嘉立创与华秋的同口径报价，再用Seeed Fusion比较小数量复杂后焊的总成本；这只是采购判断，不是最低价结论。PCBWay适合作为复杂装配的备选，数量门槛必须计入。昂贵FP模块/SE不要因为裸板最小数量而默认买5套。普通PCBA厂的质量体系或X-ray能力不等于安全密钥生产/密钥注入能力。

### 可直接复制的制造询价内容

> OpenKeys，个人3把，四层68×92×1mm，130个BOM位置，唯一USB-C母口，NFC PCB线圈，16×2白背光字符LCD，FPC2534AM flex模块，STM32H573RIT6与SE050E2HQ1/Z01Z3Z。需报价：DFM/工程边、PCB、钢网、贴片/底焊盘检查、客供料收料、LCD及线束后焊、结构打印/装配、真实贴芯NTC、限流首板上电及逐台功能测试。请分列一次性工程费、每板装配费、采购/MOQ损耗、治具、测试、税运费和返工费用。先做一块工程样板，验证后再完成其余设备；不能代换安全芯片或指纹模块。生产密钥、指纹模板与真实恢复码不会随BOM交付。

附件使用仓库当前R04 Gerber/钻孔、BOM、坐标、装配尺寸图与[硬件验收](HARDWARE_ACCEPTANCE.md)。提供确切提交和SHA256。完整设备固件尚未完成时只能报价/做工程验证，不得把诊断固件当出厂认证器。安全配对/启动密钥注入需另列受控工序；普通代焊无须持有生产密钥。

## 安全实验室

| 候选／大陆地点 | 官方证据支持的范围 | 对本项目适合询的工作 | 核对边界与入口 |
|---|---|---|---|
| 中国信通院／泰尔无锡，江苏无锡 | 2023-04-25官方服务页明确列出侧信道、EM/电压/激光故障注入、侵入/半侵入及配置/调试安全能力 | **优先询物理攻击专项**：H573/VCAP/SE/FP分电源域FI、功耗/EM泄漏、启动和指纹旁路、调试与探针攻击 | [官方能力和联系方式](https://gma.caict.ac.cn/plat/cttl-t/cttl-promote-the-service-capability-for-ic-industry-via-wuxi-ic-public-service-platform)。资料有年份；确认当前设备、整板/成品受理及是否接受个人3把项目 |
| atsec中国，艾特赛克北京 | FIDO官方实验室条目为Security Testing；中国服务页列FIDO2/CTAP1/CTAP2和L2评估 | **优先询源码/架构及FIDO安全预评审**：UV/UP、凭据、固件更新、密钥/指纹、notes管理范围 | [FIDO官方条目](https://fidoalliance.org/lab/atsec-beijing-information-technology-co-ltd-atsec-china/)、[中国服务](https://www.atsec.cn/services/identity/fido/)、[联系](https://www.atsec.cn/contact/)，info_cn@atsec.com。不能把L2资质等同已确认可做全部高级SCA/FI |
| SGS Brightsight，北京/上海/广州 | FIDO条目列Security Testing；官方地点页列北京/上海/广州，提供预评估到正式安全评估 | 硬件安全、芯片/整机边界、SCA/FI与FIDO预评估的备选或交叉复核 | [FIDO条目](https://fidoalliance.org/lab/sgs-brightsight/)、[安全评估](https://www.brightsight.com/security-evaluations)、[大陆地点](https://www.brightsight.com/locations)、[联系](https://www.brightsight.com/contact)，brs.sales@sgs.com。该FIDO条目的具体地址是西班牙，**不能据此认定大陆分部持同一认可范围**；须明确大陆承接地点、授权与分包 |
| 中国信通院泰尔终端，北京 | 官方服务页列指纹等生物识别、活体/准确性及FIDO L2服务 | 整个指纹输入/匹配/认证链的FRR/FAR、PAD及协议/执行环境评估 | [官方业务和联系方式](https://service.caict.ac.cn/ywjs/202011/t20201130_1800.html)，ifs@caict.ac.cn。与无锡物理攻击专项分开确认团队和范围 |
| SGS中国EMC团队，大陆实验室网络 | 官方列ESD、辐射/传导抗扰、电压骤降等测试 | **电气/抗干扰单独询价**：USB、电池供电及NFC工作时ESD/EMS、充电与电池/热；与硬件攻击报告配套 | [EMC服务](https://www.sgsgroup.com.cn/zh-cn/services/electromagnetic-compatibility-emc-testing)、[电池服务](https://www.sgsgroup.com.cn/zh-cn/services/battery-and-accumulators)。EMC合格不能代替SCA/FI或指纹安全测试 |

公开联系方式只是询价入口，不表示机构已经确认接个人项目或当前仍使用该人员邮箱。CNAS/CMA证书须核对**实际签约实验室及附件能力范围**；FIDO资质另向[联盟目录](https://fidoalliance.org/accredited-lab-finder/)核对。不要把联盟网页中的“ISCCC”等可接受基础资质列表误读成具体实验室已获FIDO授权。

### 可直接复制的实验室需求

> 需要开放源码USB/NFC指纹FIDO2认证器的独立预评估。范围含PCB/电源/MCU/SE/传感器、不可绕过指纹、四键附加PIN、可信字符LCD、启动/签名更新/降级/调试、FIDO2/U2F/hmac-secret、加密notes/recovery、三平台管理及NFC管理通道。请先评估架构与源码，再报价SCA/FI/生物识别及EMC专项、修复复测；说明测试地点、认可范围、样品/牺牲板数量、可否个人委托、排期与交付原始证据。仅用合成账户和测试密钥，不要求生产私钥。

通过判据不能只写“未发现异常”：无有效设备指纹时获得有效签名、hmac-secret、note/recovery明文或管理/调试权限均失败；旧/未签名镜像启动、重放/计数回退也失败。需记录攻击预算、时间/脉宽/温度/频段、仪器、确切固件/配置/器件、原始功耗/EM/故障波形及未覆盖项。TVLA阴性和安全复位都不能被写成绝对安全证明。

FIDO认证的TOE可能不包含notes/恢复码和管理器；请把这些明确写进独立评估合同。源码审计、功能互操作、物理攻击和EMC分项验收。先架构预评审，再制板和实测，避免把三把日用钥匙全部用于破坏试验。所有价格、样品数和报告资格目前均**未取得本项目正式确认**。
