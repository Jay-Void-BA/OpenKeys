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
