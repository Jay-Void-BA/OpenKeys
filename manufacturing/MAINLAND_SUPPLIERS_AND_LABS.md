# 大陆厂商与实验室：个人、3 把项目筛选

[制造入口](README.md) · [委托确认模板](REQUEST_FOR_CONFIRMATION.md) · [R05 制作与验收](BUILD_R05_ZH.md)

复核日期：**2026-11-13**。当前为 R05 经典版：无指纹、六位本地 PIN、十键、SE051C2 / DFR0555；
四层板体 96×112×1mm，169 BOM 位置。必要条件是**中国大陆承接、接受个人作为委托方、接受 3 把设备的工程项目**。
实验室另需说明样品数，不能把“三把成品”当成破坏试验样品上限。

> **目前没有一家完成本项目的书面接单确认。**
> 下表是公开证据与相关回信的筛选结果。历史 iCloud 询问及回信见[联系状态](OUTREACH_STATUS.md)；
> Seeed 询问数量、泰尔转介 SCA 联系人，均未完成个人 / 数量 / R05 范围确认；SGS Brightsight 明确不接个人，已排除。
> 未下单、未授权付费服务，本轮没有发送新版询问。

## 状态定义

| 状态 | 代表什么 |
|---|---|
| 公开支持 | 官方资料明确说明了该项政策；仍不是本项目订单确认 |
| 部分证据 | 有个人用户 / 工程师或某项服务依据，但范围、日期或合同主体尚不完整 |
| 待确认 | 本次官方资料中未找到足够承诺，不据此判断接受或拒绝 |
| 数量不匹配 | 公开数量门槛超过本项目需要，未取得例外 |
| 个人条件不符 | 本次明确不承接个人，移出个人默认候选 |
| 本项目已确认 | 须有机构书面回复，覆盖个人合同、数量、范围与费用；**当前无此状态的机构** |

## 工厂筛选

| 候选 | 个人客户依据 | 小数量依据 | 本项目结论 |
|---|---|---|---|
| **嘉立创 SMT** | 部分证据：平台历史协议允许个人用户；2026 招股说明书列个人工程师及个人下单 / 付款模式 | 工艺页检索摘要列 2 片起贴；直接页面本轮受访问验证限制，需按实际报价审核 | 优先核实个人合同、3 把双面贴装 / 后焊 / 装配；**待书面确认** |
| **Seeed Fusion／矽递，深圳** | 2024 ODM 条款有个人依据；2026-11-12 回信仍未明确个人 PCBA 合同 | 本轮官网及 FAQ 仍列小数量装配、至少 5 裸板可仅装 1 片 | 已回信询问首批量；可澄清 3 把目标 / 先工程样板，当前 **未确认个人与 R05 接单** |
| **华秋智造，深圳** | 待确认：本次未找到明确个人 PCBA 合同承诺 | 公开提供打样 / 小批量 PCBA，但不是对本板装 3 把的确认 | 保留技术候选；取得个人与数量回复后再进入可选名单 |
| **PCBWay，大陆制造服务** | 待确认：账号 / 在线下单本身不作为证明 | 能力页最低装配 **5 片**；本板另需工程边 / 拼板审核 | **数量不匹配，移出 3 把默认选项**；仅书面同意装 3 把或用户接受装 5 把后再比较 |

SE 与 LCD 不应仅因裸板 MOQ 而多买。双面 SMT、LCD / NTC 后焊、十键与机壳装配、逐台测试须分别报价；普通 PCBA 能力不等于安全配对 / 密钥注入能力。

### 工厂官方证据

- **嘉立创**：[当前 SMT 服务与工艺](https://www.jlc-smt.com/)、[联系入口](https://www.jlc-smt.com/vcontact)；[平台用户协议](https://www.szlcsc.com/news/14918.html)是 2020 生效版本，不能当成现行专属 SMT 合同；[2026 招股说明书](https://disc.static.szse.cn/disc/disk03/finalpage/2026-07-30/67b3d36a-633d-4a95-9b15-d111365d4a75.PDF)正文 251–252 页涉及个人工程师及样品 / 小批量交易。个人付款也可能是替单位付款，需另问能否完全以个人签约。
- **Seeed Fusion**：[当前 PCBA](https://www.seeedstudio.com/pcb-assembly.html)、[数量 / 客供料 FAQ](https://support.seeed.cc/portal/en/kb/articles/seeed-fusion-pcb-assembly-service-quick-questions)；FAQ 较旧，数量以当前报价为准。[在线 ODM 定制条款](https://files.seeedstudio.com/fusion/odm/TermsAndConditionsForOnlineCustomServices.pdf)更新于 2024-01-19，个人条款有服务范围，不能泛化。[大陆联系](https://www.seeed.cc/contacts)：fusion@seeed.io。
- **华秋**：[PCBA 服务](https://smt.hqchip.com/)、[询价入口](https://smt.hqchip.com/quote/)、[设备](https://smt.hqchip.com/equipment.html)。须确认个人合同、3 把装配及安全模块客供；不套用过期活动价格。
- **PCBWay**：[装配能力及 MOQ](https://www.pcbway.com/pcb_prototype/PCB_assembly_Capabilities.html)本轮仍列 5 片起装、最小 50×100mm。当前 R05 板体 96×112mm 已覆盖该尺寸下限；USB 舌部 / 异形板工程边仍需审核，**数量 3 把仍未匹配**。

## 实验室筛选

**尚无机构完成“个人委托 + 3 把项目 + 当前 R05 安全范围”的完整确认。** 下表保留实际状态；未确认者不列为已合格服务商，明确不接个人者已排除。

| 候选 | 可询的技术范围 | 个人委托 | 3 把项目 / 样品数 |
|---|---|---|---|
| 泰尔无锡 | SCA；电压、EM、激光 FI；配置 / 调试安全 | **待确认**；已转介 SCA 联系人，未确认个人合同 | **待确认**；已询费用，未获最低费 / 样品数承诺 |
| atsec 中国／北京 | 架构 / 源码、FIDO 安全预评估 | **待确认** | **待确认**；另问可否只做研发预评估、无需整套认证 |
| SGS Brightsight | 原为安全预评估候选 | **本次明确不接个人，已撤回询问** | 移出本项目个人默认名单；不外推其答复为 SGS 集团所有部门政策 |
| SGS 中国 EMC 团队 | ESD、传导 / 辐射抗扰、电源跌落、电池 / 热 | **待确认** | **待确认**；按工程摸底逐项报价，不默认送检 1 台即足够 |

泰尔终端北京的指纹 FAR / FRR / PAD 方向仅保留历史来源；当前无指纹 R05 不需要这项服务。

### 实验室官方范围和联系

- **泰尔无锡**：[官方能力](https://gma.caict.ac.cn/plat/cttl-t/cttl-promote-the-service-capability-for-ic-industry-via-wuxi-ic-public-service-platform)明确列 SCA 和电压 / EM / 激光故障注入；该页发布于 2023-04-25，需核对当前承接和设备。公开联系：zhuliang@caict.ac.cn。
- **atsec 北京**：[中国 FIDO 服务](https://www.atsec.cn/services/identity/fido/)、[FIDO 官方条目](https://fidoalliance.org/lab/atsec-beijing-information-technology-co-ltd-atsec-china/)、[联系](https://www.atsec.cn/contact/)，info_cn@atsec.com。L2 资质不自动证明全部高级 SCA / FI 能力。
- **SGS Brightsight**：[安全评估](https://www.brightsight.com/security-evaluations)、[大陆地点](https://www.brightsight.com/locations)、[联系](https://www.brightsight.com/contact)，brs.sales@sgs.com。[FIDO 条目](https://fidoalliance.org/lab/sgs-brightsight/)的具体地址为西班牙，不能认定大陆各点共享相同认可范围。
- **泰尔终端北京**：[官方业务](https://service.caict.ac.cn/ywjs/202011/t20201130_1800.html)，ifs@caict.ac.cn；与无锡团队分开确认范围。
- **SGS 中国**：[EMC](https://www.sgsgroup.com.cn/zh-cn/services/electromagnetic-compatibility-emc-testing)、[电池](https://www.sgsgroup.com.cn/zh-cn/services/battery-and-accumulators)。EMC 与电池报告不能代替 SCA / FI 或指纹安全报告。

CNAS / CMA 应核对实际签约主体及能力附件，FIDO 另查[联盟目录](https://fidoalliance.org/accredited-lab-finder/)。不把联系表单、个人信息隐私条款或“个人身份验证”业务名称误读成接受个人客户。

## 怎样才算“确保支持”

先使用[确认模板](REQUEST_FOR_CONFIRMATION.md)，要求对方逐项书面回答：

1. 接受不以公司 / 高校名义、无企业营业执照的个人委托与付款。
2. 接受 3 把成品目标，写清首板、装配量和额外牺牲样品的最低数量。
3. 接受本项目确切器件 / 服务范围，明确大陆履约地点、分包和交付物。
4. 提供一次性费用、最低项目费、税运、返工 / 复测、排期及报价有效期。
5. 可用测试密钥 / 合成数据完成工作，不要求生产密钥、真实 PIN 或恢复码。

只有在回复中覆盖适用项目、记录确认日期 / 有效期及证据编号后，才标“本项目已确认”。拒绝个人或最低数量不合适时移出可选名单；没有回复则保持待确认。涉及个人资料或合同原件的证据私下保存，公开仓库只放脱敏摘要。

实验室合同需明确覆盖 **notes、recovery codes 和管理器**；常规 FIDO 的评估对象可能不包含这些。三把日用设备与破坏试验样品分开预算。所有本项目接单、最低费用、样品数和正式报告资格目前都未获书面确认。
