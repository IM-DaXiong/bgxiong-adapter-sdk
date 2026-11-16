# 操作输入 v1

在能力 operation 声明 `inputProfile: operation-input-v1`，并在包的必需特征声明 `operation-input-preflight-v2`。包的 JSON Schema 决定字段、类型、范围、条件与枚举。模型参数字段名由包定义。

控件用 RFC 6901 指针编辑同一个 input。`prompt`、`size`、`aspectRatio`、`duration`、`frameRate`、`seed` 等展示角色决定呈现，陌生角色使用声明的控件。字段的取值、单位和范围由包决定。素材槽按声明的载体形态装入受控素材 ID。资源预算见 SDK schema policy。

宿主按冻结业务记录提供的字段用 `readOnly: true` 声明，在预检前装入实际输入。只读对象的下级字段也拒绝编辑控件。预检和提交照常核对完整输入 schema，包括所有必填的只读字段。

草稿首次建立时可初始化 default。缺失的 const 可以初始化，冲突值显式拒绝。分别保留缺失、null、空串、0、false、空数组。超出 JSON 安全整数范围的整数标识使用十进制字符串。

通过 `probe` 发送冻结的操作选择、非密配置、环境事实版本及就绪状态、input 与素材内容身份。SDK 按规范 UTF-8 JSON 计算小写 SHA-256，摘要排除 `requestDigest`。对象键按 Unicode 码点排序，数组保留顺序，素材元数据表例外并按素材 ID 排序。安全整数去掉小数部分，其它数值使用 SDK 的 JSON 编码。使用 SDK 的摘要函数生成或核验摘要。

包返回 ready 和准备材料，或 rejected 和诊断，两种结果都回传请求摘要。字段诊断指针相对于 input，包业务拒绝使用自己命名空间的诊断码。预检沙箱只开放日志，提交任务、上传、凭据和检查点归执行阶段。包生成的随机值随准备材料固定有效值，准备材料不得保存秘密或可任意打开的本地文件路径。

SDK 核预检结果并封装准备材料摘要。参考 CLI 在本次诊断目录写 `operation-preparation.json`。提交传入同一 input、当前 `environmentRevisionDigest`、受控素材及该封装。输入、账号绑定、包身份、素材事实或环境版本变化时重新预检。缺失或失效的准备结果在创建提交检查点前返回 `PREPARATION_STALE`。查询或取消沿原任务继续，受理未知时不重新提交。

`examples.json` 展示草稿、预检和两种结果。协议沿用 capabilities、probe、upload、submit、query、cancel。视频数字人口播独立 profile 继续使用其自身准备合同。

## 独立验证

1. 用对应包和 worker 执行 SDK `check`，确认真实能力发现返回操作 profile。
2. 在新建开发工作区用确切身份和素材事实调用 `probe`，用明确非法的组合再次预检，核字段诊断。单纯预检不装载厂商凭证或发 HTTP。
3. 用准备回执调用 `submit`。对单列的模拟 fixture 录制真实执行的 probe 与 submit，再用 SDK `replay`、`conform` 校验。查询取消沿原任务身份。真实厂商回执独立留存。
4. 分别修改输入、准备材料、环境版本、包或素材，核旧准备在提交前失效。同一 JSON 交给声明或业务规则不同的包，记录每个包的实际判断。
