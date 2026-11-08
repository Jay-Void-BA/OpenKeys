# R0.3 验证记录

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
