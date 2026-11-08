# 导入格式 v1

只能在真实设备授权后打开文件和预览私密元数据。当前管理器尚无硬件授权后端，所以保持锁定，不接受真实秘密。以下样例均为公开测试数据。

## TOTP

UTF-8 文本，一行一个 URI。支持 SHA1/SHA256/SHA512、6/8 位、15–120 秒周期；省略参数使用 SHA1/6 位/30 秒。

~~~text
otpauth://totp/Example:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Example
~~~

重复参数、无效 Base32、非法 UTF-8、控制字符/双向文本控制符、冲突 issuer、未知参数均拒绝。默认不猜测纠错。

## Notes 和 recovery codes

~~~json
{
  "version": 1,
  "entries": [
    {"type": "otp", "uri": "otpauth://totp/Example:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Example"},
    {"type": "note", "title": "Example account", "body": "SYNTHETIC TEST NOTE"},
    {"type": "recovery_codes", "title": "Example backup codes", "codes": ["SYNTHETIC-ONE", "SYNTHETIC-TWO"]}
  ]
}
~~~

单文件最多 1 MiB；单批最多 256 条；单条 note 最多 16 KiB；每组 recovery codes 最多 64 个。设备实际容量还需取决于 SE/Flash 和保留空间；桌面格式上限不代表硬件已经支持相同容量。

已实现解析和校验；尚未实现真实设备写入。批量提交必须只包含勾选记录，设备重新校验内容并让用户确认。不可由电脑一句“已解锁”放行。

Google Authenticator otpauth-migration 二维码/protobuf、Aegis、2FAS、Bitwarden、1Password、CSV 和扫描二维码均尚未实现；本版不把通用 JSON 支持冒充这些厂商格式支持。

recovery code 的“已使用”只能是用户标记或可信服务返回，不能凭本地点击就声称网站已消费该码。当前导入模型不伪造使用状态。导入不是安全删除原始导出文件，工具不自动删除用户文件。
