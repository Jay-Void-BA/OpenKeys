# 导入格式 v2

只接受 UTF-8 JSON 的 notes 和 recovery codes。必须先完成真实设备指纹验证；提交还需设备 PIN 与实体确认。当前硬件通道尚未实现，管理器保持锁定，不能导入真实秘密。

```json
{
  "version": 2,
  "entries": [
    {"type": "note", "title": "Example account", "body": "SYNTHETIC TEST NOTE"},
    {"type": "recovery_codes", "title": "Example backup codes", "codes": ["SYNTHETIC-ONE", "SYNTHETIC-TWO"]}
  ]
}
```

单文件最多 1MiB；最多 256 条；每 note body 最多 16KiB；每组 recovery codes 最多 64 个、每码最多 256 UTF-8 字节；标题最多 256 字节。拒绝未知/重复 JSON 字段、旧版本、OTP 类型、otpauth URI、非法 UTF-8、误导性控制字符和双向文本控制符。多行 note 允许换行/回车/制表符；它们不得用来模拟设备系统授权画面。

摘要有 v2 域分隔，绑定类型、标题、完整内容、长度、代码边界与条目顺序。它不是认证码或解锁凭据；设备必须对认证解密并重新校验后的实际选中项重算摘要。格式上限不代表目标设备已经具备对应容量。

尚未实现真实设备写入、存储加密、事务提交、备份恢复和厂商导出格式转换。程序不自动删除原始导出文件；本地标记“已使用”也不能声称网站已消费恢复码。
