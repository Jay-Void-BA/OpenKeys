# OpenKeys R0.4

开源认证器工程：**设备指纹必需、PIN仅附加、白背光16×2英文字符LCD、四个场景按键、一个USB-C母口兼USB2数据/5V充电、NFC**。已取消动态验证码和USB-A。

**实际电气PCB已经画完**：10页原理图、130个BOM位置、四层68×92×1mm板，1430段走线、216个过孔；KiCad ERC/DRC均0错误0警告、0未连接，624项独立封装/引脚/网络检查通过。已导出Gerber、钻孔、贴片坐标及结构加工源。电路/被动模拟与连通检查通过，不表示完成整机安全资格；**完整设备固件和真实管理通道仍在开发，当前不可存放真实账户秘密**。

![R04 CAD rendering, not a photograph](hardware/r04/exports/OpenKeys-assembly.png)

| 交付 | 入口与实际状态 |
|---|---|
| 电路图 | [10页PDF](hardware/r04/exports/OpenKeys-schematic.pdf)、[可编辑KiCad](hardware/r04/kicad/OpenKeys.kicad_sch)、[BOM](hardware/r04/BOM.csv)、[实际引脚](hardware/r04/pinout.csv) |
| 实际PCB与制造数据 | [KiCad电气板](hardware/r04/kicad/OpenKeys.kicad_pcb)、[Gerber/钻孔ZIP](hardware/r04/exports/OpenKeys-fabrication.zip)、[贴片坐标](hardware/r04/exports/OpenKeys-SMD-position.csv)、[最终DRC](hardware/r04/review/routed-drc.json) |
| 实物设计图 | [尺寸PDF](hardware/r04/exports/OpenKeys-dimensions.pdf)、[SVG](hardware/r04/exports/OpenKeys-dimensions.svg)、[OpenSCAD](hardware/r04/mechanical/enclosure.scad)、[STL/板体STEP](hardware/r04/exports)、[Blender](hardware/r04/mechanical/OpenKeys-assembly.blend)；外形74×98×20mm，须样件试装 |
| 模拟与核对 | [被动网络/线圈估计](hardware/r04/review/simulation.json)、[电源DC角落](hardware/r04/review/protection-calculations.json)、[624项板网核对](hardware/r04/review/board-check.json)、[文件哈希](hardware/r04/review/cad-manifest.json) |
| 固件 | [按实板编写的驱动](firmware/board)、[ARM诊断ELF/BIN与哈希](firmware/dist)、[授权/指纹/PIN策略](crates/device-policy)；诊断镜像保持SE/指纹关闭，完整安全FIDO运行时仍在开发，见[固件状态](firmware/README.md) |
| 管理软件 | [AuthDesk源码](apps/manager)、[三平台开发构建](docs/NATIVE_BUILDS.md)；macOS/Linux/Windows原生GUI，真实设备认证通道未完成，保持锁定 |
| Android/iOS NFC管理 | [实现前提](apps/mobile/README.md)；尚无可用APK/IPA，不把手机指纹等同于设备指纹 |
| 安全测试与复查 | **[你可以照做的安全测试方法](docs/SECURITY_TEST_PLAN.md)**、[R04发现与修正](docs/SECURITY_REVIEW_R04.md)、[完整交付预检](manufacturing/readiness-report.json) |

固定授权顺序：**设备指纹 → 必要时设备四键PIN → 完整请求与实体确认 → 单次操作**。导入、删除、恢复码读取、指纹管理、PIN修改、配对、升级和擦除要求附加PIN。PIN不能单独解锁。12次四符号PIN序列：短按1–4输入、长按4删除、长按2取消；菜单功能为Previous / Next / Confirm / Cancel。

PCB先冻结引脚和文件摘要，再写匹配固件。硬件已增加独立UV/OV、开盖复位、SE使能门控和LCD复位，**门控本身不能证明指纹通过**；必须配合真实认证传感器、可信启动、TrustZone/GTZC/DMA隔离和受保护执行路径。尚无实物侧信道、故障注入或整机认证通过报告。

最新软件续更：SE对象APDU、双head/提交锚点、持久指纹重试、共享SAES/RNG与FPC GCM标签验证适配；100项本地测试通过。详见[持久状态实现](crates/se-state/README.md)、[密码专项复查](docs/H573_CRYPTO_REVIEW.md)和[CI/构建记录](docs/r04-build-report.json)。实片资格、完整认证运行时和管理通道仍未完成；当前8次失败锁定的恢复/可用性须在制造前评审。

充电：**Apple20W USB-C适配器的5V档，输入预算约0.50W；电池侧100mA目标、4.2V上限，约0.42W上限**，随电量与系统负载下降；不请求9V、不使用20W。选500mAh受保护电池包、独立贴芯真实NTC；需开启设备运行充电服务，不要求解锁秘密。电芯及保护包寿命、温升和实际充电时间待实测。

目标协议包括FIDO2/WebAuthn/Passkey、U2F、hmac-secret、凭据管理、加密notes/recovery codes，分别验证macOS/iOS及各扩展。功能列表不代表已经实现。三把FIDO密钥分别注册，恢复码另留独立离线备份。

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo check -p device-policy --target thumbv8m.main-none-eabihf --locked
python3 hardware/r04/tools/manifest.py --check
# KiCad9/pcbnew/OpenSCAD/CairoSVG环境；验证现存布线并导出，不重建PCB：
sh hardware/r04/tools/export.sh
# 当前应明确报告未完成的交付条件并返回2：
python3 scripts/check-production-ready.py
```

[验证记录](docs/VALIDATION.md)区分原生软件CI与新硬件检查。[R04硬件说明](hardware/r04/README.md)含板层、天线、机械坐标、外接件、工具版本和模拟限制；[制造任务书](manufacturing/HANDOFF.md)含三档预算口径与大陆厂家入口。未代表用户联系厂商或下单。

运行源码采用[MIT](LICENSE)，硬件采用[CERN-OHL-S-2.0](hardware/LICENSE-CERN-OHL-S-2.0.txt)。第三方库/芯片ROM保留各自许可及信任边界。[参考锁定版本](references.lock.tsv)包含Solo2、Google OpenSK/Wasefire、Nitrokey/Trussed等，不把它们的认证转移给本设备。`hardware/r03/`和更早目录为历史资料，不能与R04混用。
