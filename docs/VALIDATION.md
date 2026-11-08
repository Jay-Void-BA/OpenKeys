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
