# 验证记录：R0.3历史与R0.4续更

日期：2026-11-08（北京时间）。对象为R0.3工程设计及当前Rust源码；全部测试材料为公开或合成数据。历史R0.1/R0.2结果保留在Git历史，不能冒充新版测试。

本地31项Rust测试通过：device-policy23项、vault-import5项、AuthDesk3项。包括指纹不可被正确PIN替代、请求/连接/内容替换、一次消费、过期/时钟回退、慢硬件调用、GCM帧逐位变异与断电重放、持久写不确定、PIN预扣与取消、LCD长身份分页及锁定清屏、拒绝旧OTP格式、导入边界和锁屏不绘制私密内容。

`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo check -p device-policy --target thumbv8m.main-none-eabihf --locked`通过。Cortex-M33检查对象是no_std库，不是可烧录固件。CC/CXX设置为拒绝编译脚本，自己的运行源码不使用C/C++；不对OS/第三方库/芯片ROM作相同承诺。

RustSec `--deny warnings`检查当前锁文件：0条已知漏洞、0条维护警告。数据库提交`309392b18f6b17022f82c701f300433126901068`（2026-11-06）。不代表不存在未知漏洞，也不代替对密码、硬件与调用策略的审查。精确数据见build-report及dependency-audit。

本地KiCad9电路导出8页PDF、SVG和XML；115个BOM位号，ERC0电气错误、2项明确未解决RF封装警告。32项独立XML网表检查通过。OpenSCAD三组STL成功导出，Blender渲染及尺寸图已经目视检查；结构模型尚未打印装配。没有完整PCB、Gerber、DRC制造验收或物理测量。

GitHub Actions分别运行Linux、macOS、Windows原生构建、测试、fmt、Clippy与依赖审计。编译器/Rustdoc警告和Clippy/审计警告视为失败，官方Actions固定完整提交。R0.3原生远端结果在本版上传后记录于build-report；旧R0.2成功任务不计为新版成功证据。

未完成：设备端到端认证、真实SE/指纹/计数/PIN/管理通道、FIDO/U2F/hmac-secret互操作、vault掉电事务、iPhone NFC、充电/suspend/热/备用域防拆、侧信道/故障注入/EMC、完整覆盖模糊测试/独立审计、正式安装包签名公证。详见[安全复查](SECURITY_REVIEW_R03.md)。

复现：

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo check -p device-policy --target thumbv8m.main-none-eabihf --locked
hardware/r03/tools/export.sh
python3 scripts/check-production-ready.py
```

最后一项当前预期返回2并报告BLOCKED，不应篡改为成功。CI无编译/依赖警告不表示硬件无封装警告，更不代表整机安全认证。

## 上传后的最终CI结果

[运行42468485292](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42468485292)，输入提交`63e1e1672fc5484140cfbaeeb3297f133b239892`：Linux、macOS、Windows原生任务及依赖审计全部成功。31项测试在三个原生目标上通过；Linux任务还通过Cortex-M33策略库检查。完整日志375528字节已核对，warning行0、warning注释0；macOS有1条运行平台容量notice。

较早运行42467886762曾有1条审计任务隐式安装Rust的warning，已改为显式安装/选择固定工具链，并由上述完整复跑确认消除。未通过删除历史日志来宣称没有发生过警告。

三个开发版ZIP已下载并与GitHub公布的SHA256逐一相符，检查构建信息确认Linux x86_64、Windows x86_64 MSVC、macOS Apple Silicon。仅检查ZIP/二进制摘要及元信息，未在Mac/Windows执行或连接真实设备；不等于签名安装包/可用设备管理器。见[NATIVE_BUILDS](NATIVE_BUILDS.md)。

公开合成notes基准一次结果：51200条解析并计算摘要约16.48ms；只代表该输入和容器的优化构建，不代表SE、USB/NFC、指纹或设备写入耗时。

## 输入保护与NFC封装续更

在原生CI所测运行源码不变的基础上，硬件增加TPS259470L独立输入保护与使能钳位、独立直流误差计算，以及按复旦原始图纸制作的NFC候选焊盘。现9页电路、126个BOM位号、37项独立网表检查，ERC0电气错误/1项天线封装警告。OVLO直流范围5.5612–5.8436V；动态/热/射频/实机防攻击资格未通过。已通过的31项软件测试不被拿来证明这些硬件性能。

## R0.4 电气PCB与模拟

R04取代以上R03“无完整PCB/无Gerber”的硬件状态：10页原理图、130位置，ERC0错误/0警告；最终KiCad9.0.2 DRC0违规/0未连接，624项独立原理图XML—PCB焊盘/封装/板层/短电源线核对通过，1430段走线、216过孔。Gerber/钻孔/贴片坐标、板体STEP及四组结构STL已导出。

ngspice47被动线圈和复位RC模拟通过已列数值断言且无模拟警告；Neumann线圈积分两步长收敛差约0.69%，约1.20µH仅估计，不含真实FM11输入非线性。56pF初始匹配及13.81MHz名义线性谐振均不能替代VNA调谐。eFuse OVLO DC范围5.5612–5.8436V，不包含瞬态/热验证。

尺寸图和修正三维坐标后的渲染已目视检查。物理装配、电源、NFC、侧信道、降压和指纹强制路径均未标为通过。新增硬件CI固定KiCad容器摘要，复查交付文件哈希、ERC、DRC和板网一致性；其远端运行结果须在实际完成后记录。前述31项Rust测试及旧原生CI不证明新硬件安全。新增加源码时必须重新跑相关软件检查。

## R04板级Rust续更

新增后本地全工作区41项测试通过：授权策略23、导入5、管理器4、四键3、LCD/Flash驱动4、critical-section补丁2。fmt、工作区Clippy `-D warnings`、Cortex-M33板级检查、实际目标诊断ELF编译及目标Clippy均通过。ARM诊断BIN为25296字节，向量表栈/入口检查通过；构建脚本逐项验证PCB/引脚哈希及GPIO/AF表。实际上电/运行没有被标为通过。

硬件远端[运行42498058624](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42498058624)成功，完整日志42157字节检查warning行0；KiCad9.0.7容器再次得到ERC/DRC零违规及零未连接。首次容器运行存在Git默认分支提示，已显式配置默认分支并完整复跑确认消除。没有通过过滤日志或更改电气规则掩盖警告。

新增依赖链的`bare-metal`维护警告已从实际依赖图消除，Cortex-M源码补丁、理由、许可证和测试随源提供，没有RustSec忽略项。新的三平台CI和目标ELF任务配置将在本次源码提交后实际运行；不能拿旧R03软件CI当作这些新源码的结果。

跨平台续更：[运行42563970747](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42563970747)，输入提交54c279a：macOS、Windows、Linux原生任务和严格审计全部成功。完整日志435229字节，逐行核对warning0。较早Windows任务因Git把PCB改成CRLF而被原始字节哈希正确拦下；已用仓库属性固定LF，保持原有属性规则，完整复跑确认修复。该运行覆盖41项软件测试及ARM诊断；新快照核心和SAES适配需由后续提交单独验证。

快照续更本地全工作区48项测试通过：新增快照6项和双区物理地址边界1项。包括14种模型断电/不确定提交组合、授权在解密后失效、9个写入检查点取消、旧镜像重放及分块篡改。工作区/ARM目标Clippy `-D warnings`和目标编译通过，诊断镜像重新生成且源码摘要一致。349个锁定依赖的严格审计通过。真实SE ledger、SAES芯片KAT和物理断电仍为NOT_RUN；后续远端CI结果单独记录。

快照远端[运行42565375583](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42565375583)，输入提交7cf8cf8：Linux/macOS/Windows及依赖审计全部成功，完整日志448007字节，warning行0；同提交硬件[运行42565375425](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42565375425)成功。这是48项测试版本的证据，不自动覆盖后续源码。

## SE与H573密码路径续更

2026-11-09 UTC，本地全工作区81项测试通过：授权23、导入5、管理器4、板级与critical-section11、快照6、T=1通信13、SCP03十二项、H573寄存器/熵七项。完整fmt、工作区Clippy `-D warnings`、Cortex-M33全部核心库检查、实际目标诊断编译与目标Clippy通过。诊断BIN仍为25296字节，仍不启用SE/指纹/充电/私密服务；新源码哈希已重新绑定，不将它重命名为生产认证镜像。

355个锁定依赖的严格审计通过，0已知漏洞/0维护警告；当前审计数据库提交及原始JSON单列于[审计来源](dependency-audit-provenance.json)。`--no-fetch`模式的原始审计JSON不提供commit字段，所以另取已核对干净工作树的实际Git提交记录，未把旧数据库提交冒充最新结果。硬件89文件SHA再次核对通过。生产预检仍BLOCKED，有13项未完成条件。

已修复SAES IV寄存器顺序、故障标志误清和末次输出后/擦除前故障被覆盖的问题，增加H573专用RNG与明确Secure MMIO边界。具体范围和实片未执行项见[专项核查](H573_CRYPTO_REVIEW.md)；[R04机器记录](r04-build-report.json)将远端新源码CI与上述本地结果分开记录。

SE/密码远端[运行42575127427](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42575127427)，输入提交1ca41e3：三个原生目标及审计全部成功，81项测试通过；Linux同时编译/检查实际ARM适配。完整日志481514字节，warning行0、warning注释0；macOS有1条平台排队容量notice。完整日志SHA256及各范围见R04机器记录。

最终资料续更复跑[42577949376](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42577949376)，输入提交1dc1370：三平台及审计全部成功；完整日志481498字节，warning行0、warning注释0。macOS仍仅有平台容量notice。文档记录SE持久实现约束和NACK端口缺口，不改变诊断镜像的用途。

## 持久SE与共享密码服务续更

本地全工作区100项测试通过：新增SE状态15项、共享密码服务4项，之前81项保留。SE模型逐条APDU前后中断/应答丢失、head半写、旧head/NOR重放、请求替换、nonce不退款、失败预算与tag耗尽、schema/marker/计数回退和重入均作负向测试；真实vault核心使用新ledger及测试AES-GCM进行组合验证。模型中的counter原子/持久行为仍是实片待验证前提，不是实验结果。

完整fmt、工作区/目标Clippy `-D warnings`、Cortex-M33编译及实际诊断ELF/BIN导出通过。公开SE schema由独立Python生成，与Rust固定marker向量一致；`--check`不操作设备。356个锁定依赖严格审计0已知漏洞/0维护警告，硬件89文件SHA核对通过。诊断仍为25296字节，全部私密服务关闭；新源码摘要已绑定。新提交远端CI需在上传后单独记录，不能用前述81项版本替代。

持久SE远端[运行42586336802](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42586336802)，输入提交0a5c207：三平台各100项测试、编译/Clippy/格式/严格依赖审计及Linux ARM检查全部成功。从Actions ZIP取全部四个顶层任务日志、排序后以单个LF连接，共313524字节，warning行0、warning注释0；macOS有1条平台容量notice。摘要见R04机器记录。`gh run view --log`一度返回空结果，未把空文件当作已核查日志，另从完整ZIP验证。

## 桌面导入隐私续更

全工作区111项本地测试通过；AuthDesk从4项增至15项，覆盖遮蔽前撤销、只选路径不读文件、重新授权、真实后台fixture读取、批次总大小/条数、文件类型、取消后的单线程上限、旧结果/过期结果、焦点/休眠/回拨与代际耗尽。完整格式与工作区Clippy `-D warnings`通过。356依赖严格审计0已知漏洞/0维护警告；ARM诊断重导出并继续保持25296字节及非认证用途。新管理器源码三平台CI将在本次上传后单独记录；原生对话框/合成器/慢盘实机验证仍NOT_RUN。

管理器远端[运行42601410470](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42601410470)，输入提交e7b78ba：三平台各111项测试、构建/Clippy/格式与严格审计全部成功，Linux ARM检查成功。全部四个任务日志从ZIP按路径排序以LF连接，共317359字节，warning行0、warning注释0；macOS仅1条平台容量notice。原生UI实机资格仍NOT_RUN。

## 指纹SPI与加密请求续更

全工作区125项本地测试通过：新增4项加密请求构造测试和10项板级SPI测试。双向AES-GCM独立公开向量已核对并提供Python复现脚本；真实ExclusiveDevice配合模型SPI检查整帧CS、600µs延迟、半传输/错帧/洪泛/引脚故障、错请求和跨截止时间写入。格式、完整工作区与实际ARM目标Clippy `-D warnings`、全部M33核心库检查、诊断导出通过。356依赖严格审计0已知漏洞/0维护警告，89个硬件文件SHA不变。诊断仍25296字节、无私密服务。新源码远端CI将在上传后单列，不使用111项版本替代；FPC/SAU/GTZC/watchdog/IV故障和侧信道实测仍NOT_RUN。

最新源码（包含全部Bidi_Control拒绝与未知窗口焦点锁定）[运行42606524747](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42606524747)，输入提交3bf60ce：三平台各125项测试、严格依赖审计及全部编译/Clippy/格式成功；Linux ARM检查成功。完整四任务日志323538字节，SHA256 `934e894581cf79041cc486ad589da52535a9ebad239e0394429c51d5d90f43cc`，warning行0、warning注释0，macOS有1条平台容量notice。之前8bcdec3的125项运行42605076674也成功；机器记录分列两者，不混用源码。

最新三个原生开发包已通过GitHub工具实际取得并核对ZIP摘要、程序架构与rustc元信息；均未执行或连接实物。下载/摘要入口更新到[NATIVE_BUILDS](NATIVE_BUILDS.md)。硬件89个CAD文件保持相同摘要。生产预检再次明确BLOCKED、13项缺口，未用开发包或诊断ELF冒充成品/安全认证。

## OpenSK认证核心与Secure I2C续更

当前155项本地测试通过：新增SE I2C寄存器模型11项、OpenSK网关/注册/登录/U2F/hmac-secret v1/v2/连续请求11项、SE密钥APDU与独立验签8项。工作区fmt/Clippy `-D warnings`、实际ARM目标编译和诊断目标Clippy通过。新增测试里的软件密钥和模型仅在测试构建存在，未冒充实物SE或已部署认证器。

399个锁定依赖的严格审计通过，0已知漏洞/0维护警告；OpenSK83个源文件及3处明确补丁的哈希核对通过，保留Apache许可和原始上游摘要。89个硬件文件摘要不变；诊断重新导出仍25296字节、无私密服务。新源码远端CI将在上传后记录，前述125项版本不覆盖这些新增源码。硬件Env、密钥桥接、实际协议/管理通道和生产启动仍待完成；硬件验收见manufacturing/HARDWARE_ACCEPTANCE.md。

155项版本远端[42619548062](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42619548062)，输入31ec3e366e60e7c38d9be49465ed374458db9a49：三平台各155项测试及严格审计成功。四个顶层任务完整日志共365107字节，SHA256 ad09257915035f1b308487f8dd8963f662e3e9376859cbdf767730ada56fff4f；warning行0、warning注释0、macOS平台容量notice1。此结果不覆盖下面的新记录/持久适配源码。

## 流式记录与OpenSK持久适配续更

当前183项本地测试通过；工作区fmt/Clippy、Cortex-M33认证核心/目标适配和实际诊断目标Clippy均通过。增加完整1MiB流式编辑、范围后段故障/取消全输出清零、严格记录索引、notes/recovery借用解析、主机规范编码、OpenSK持久接口、明确profile初始化、专用签名counter与槽预留、通用重置隔离及101账户连续请求容量回归。共享的AES/介质fixture仅以测试源编译，不进入生产代码。

SE schema2新增两个独立counter并拒绝schema1，marker=3c4ab91dae4b19c804e5cdfc6c82a083d442023327822d9b6d70fa8f09154fe5；只导出公开布局，没有对设备编程或自动迁移。400个锁定依赖以实际更新的RustSec数据库d9cb059f3217db87aeccd190b81eaffe8cc29cb9严格审计，0已知漏洞/0维护警告。CAD89文件摘要不变；ARM诊断仍25296字节、无私密服务。新源码远端CI另记；完整Root、生产密钥/启动、管理通道和实片资格仍未完成。对抗性复核明确拒绝据此断言整机理论安全，制造/实验室候选只代表公开能力核对。

183项版本远端[42719762400](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42719762400)，输入e5340fdfc701fb3bd4a3d3b2f355ca62f95441be：三平台各183项测试、全部编译/Clippy/格式、严格依赖审计及Linux ARM检查成功。完整四任务日志379288字节，SHA256 2ee49a68d91bf03d1161d115a899c1c6d5ac6f32d20c0e1dbf873a2676eb56d5，warning行0、warning注释0、平台notice1。三个开发包及ARM诊断ZIP经GitHub连接器实际取得并匹配GitHub摘要，ELF/PE/Mach-O与构建信息相符；未执行这些二进制、未接真设备，明确保留开发/诊断用途。下载索引与机器记录更新到本版本。

## 一次请求执行范围续更

当前191项本地测试通过，完整fmt、工作区Clippy `-D warnings`、Cortex-M33认证核心/目标编译及诊断目标Clippy通过。新增一次消费的Execution、固定操作预算、不可续期和持续代际/时间/状态检查；FIDO网关已转移执行所有权，并测试激活回退、密码操作前撤权及输出后撤权。真实Root检查点、硬件密钥桥接和批量staging仍未接通，不能把策略库测试当成生产部署。

严格依赖审计重新使用上游HEAD d9cb059f3217db87aeccd190b81eaffe8cc29cb9的独立干净工作树，400包0已知漏洞/0维护警告。先前本地数据库缓存存在未提交差异，本次未使用该缓存，来源记录已改正；不能以旧缓存的HEAD字段证明其文件内容。新源码远端CI将在实际运行后单列，前述183项结果不覆盖本次变更。
