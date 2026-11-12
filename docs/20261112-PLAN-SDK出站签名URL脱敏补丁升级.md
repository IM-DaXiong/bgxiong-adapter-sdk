# SDK 出站签名 URL 脱敏补丁升级计划

- 日期：2026-11-12（Asia/Shanghai）。
- 状态：用户已授权审核通过后升级。审核通过，开始执行，结果见§5。
- 基线：SDK `050b7f1adb98d1d292ba57928ed891d824f8ab28`，sdkRelease 2.1.3，reference-host/tools 2.1.3，协议2、WIT 2.0.0。
- 范围：SDK通用HTTP审计，不包含Kling平台分支、视频schema/角色UI、旧库迁移或新增协议。本文是SDK缺口修订的唯一任务正文；调用方计划只链接本文。

## 1. 已证实缺陷与复现

真实 `HttpBroker::execute` → `before_send::redact_url` → `BeforeSendSink::persist`，对未在CredentialVault登记的签名query值只做vault字符串替换，因此原值进入持久化前的record。供应商返回的短期下载签名并不是用户API Key，通常不在vault内。

使用本地127.0.0.1随机端口和合成签名，无真实Key、无用户媒体、无云端请求。两个探针实际结果：普通查询PASS；`signed_download_query_must_not_enter_persisted_audit` FAIL，断言“SDK persisted an unregistered signed-query value”。测试证明HTTP请求保留签名，且捕获到一条实际before-send记录；失败发生在持久记录脱敏断言。后续HTTP audit断言因首个失败未执行，不能写已验。

原始复现保存在同级适配器仓 `packages/cloud-adapters/kling4-omni/tests/sdk-transport/sdk_transport.rs`；独立测试清单同目录Cargo.toml/Cargo.lock。该测试使用SDK本仓crate，不mock redact_url。SDK后续自身回归必须迁入本仓已有http_stream.rs，外部适配器测试只作为接入验收，不能成为SDK构建前置。

最小输入：`GET /video?X-Amz-Signature=synthetic-unregistered-signature-7f82&expires=123`。合成签名只用于fixture。原测试运行2项：1通过、1失败、0忽略，退出101；原始本机日志在调用方工作区logs/kling4-20261112/sdk-transport.log。SDK源码本轮未改。

## 2. 根因、边界与修复合同

- 已核源码：`crates/reference-host/src/before_send.rs::redact_url`（约71行）只对query键和值调用vault.redact；`http.rs`约352–368行将其结果交给sink；`policy.rs::HttpAudit::new`只记录origin元数据，源码未发现同类query持久化。
- 拟定行为：**只改变审计副本**。保留scheme/host/port/path与query键、顺序和重复次数，所有query值一律用固定`[REDACTED]`替换（空值也替换）；不按signature/token等供应商关键词判断。继续vault脱敏query键及path中已知凭据；移除fragment，user/password不得进入审计。实际网络请求raw URL完全不变。
- URL解析失败不得回退原文持久化：返回固定`[INVALID_URL]`，不含raw。正常HttpBroker应先拒非法URL，该分支仍做单测。
- 不改body/text通用遍历，不借本任务宣称所有文本里的任意秘密都能被自动识别。正文中嵌URL的全局策略不在本次已证实缺陷范围；若探针产生新证据，记录独立缺口，不随意删除业务prompt审计。
- 不改变BeforeSendSink先持久化后出网顺序、拒绝即零请求、取消/截止时间/撤权、媒体句柄描述、HTTP权限或真实失败含义。
- 审计消费者不应依赖query值重建业务请求；任务恢复必须用原冻结请求/任务ID。query值全面隐藏的可观测性变化须写入CHANGELOG和集成说明。

## 3. 文件级原子步骤（均NOT_STARTED）

### U01 固定新批次与复现基线

1. 从SDK实际Git根发现同级开发根，读AGENTS；fresh fetch并核主仓与SDK最新源码，正常合并，记录HEAD/status/锁与机器角色；不重置他人改动。
2. 在`crates/reference-host/tests/http_stream.rs`复用现broker/AuditGate，移入最小签名query回归（新测试函数名固定`before_send_redacts_unregistered_query_values_without_changing_request`）；只依赖本仓crate和本地server。
3. 在修复前运行该单用例，保存失败计数/exit/源码SHA。不添加ignore，不用should_panic将缺陷伪装PASS。

输入为已确认的SDK 2.1.3失败；输出为SDK自包含最小复现。若新批已修复，记录真实差异和行为结果，禁止重复写另一套修复。

### U02 修复审计副本

1. 唯一生产源码改动：`crates/reference-host/src/before_send.rs::redact_url`，按§2构造脱敏副本；签名值不可参与持久摘要、错误消息或调试输出。
2. 同文件新增内联测试，覆盖无query、单值、多值/重复键、空值、percent编码、无效URL、fragment和userinfo。无需新增模块或依赖。
3. 复验U01，服务器收到原始query但sink里找不到合成签名；不要修改实际request builder或加入Kling字面量。

失败只在本轮一次正式定向修正额度内处理；仍失败停止受影响阶段，不扩为全SDK重写。

### U03 保留出站生命周期合同

1. 在已有`crates/reference-host/tests/http_stream.rs`扩真实请求断言，记录HTTP audit没有签名；覆盖before-send拒绝时HTTP请求数仍为0。
2. 复用现有`before_send_ack_precedes_audio_http_and_rejection_sends_nothing`和本次新用例；保留body=media只审计mediaId/sha256/byteLength的现合同。
3. 无关生成、上传、声音业务不新增测试矩阵；不运行主仓check:affected或发行预检。

### U04 版本与冻结输入

1. 拟议补丁发行 **sdkRelease 2.1.4**；实施前若版本已被其他批占用，核登记后选择下一未用补丁，回写本文再构建，不覆盖已发行字节。
2. 同批修改`crates/reference-host/Cargo.toml`为2.1.4；`crates/tools/Cargo.toml`为2.1.4以标识消费新host的CLI/worker构建；`release-manifest.json::sdkRelease`为2.1.4；`Cargo.lock`通过Cargo正常更新两个workspace包版本，不升级无关注册表依赖。
3. `Cargo.toml`workspace协议版本、sdk/rust、ports/runtime/transport/validator/media组件版本均保持不变（除非新证据证明其代码也需改动）；WIT/schema/六操作不升ABI。
4. `CHANGELOG.md`增加真实修复和未验范围；`HOST-INTEGRATION.zh-CN.md`登记审计query值不可用于业务恢复。生成清单用现`tools/export_sdk.py --refresh-manifest --sdk <SDK绝对根>`刷新，不手填产物hash。`release-lock.json`/`RELEASE-STATUS.json`仅由真实产物出口生成，不提前写READY。

### U05 定向验证与构建

1. 在S执行：`cargo test -p bgx-adapter-v2-reference-host --locked --offline --test http_stream before_send -- --list`，确认命中后执行去掉`-- --list`的同命令；新增内联测试运行`cargo test -p bgx-adapter-v2-reference-host --locked --offline --lib before_send`。记录非零测试数，不能以0 tests通过。
2. 只对CLI/worker重建所需依赖，采用现有导出/来源回执工具；首次构建前核工具真实参数，禁止临时绕过作者校验。工具schema/模板未变，不手改vendor镜像；若发行清单校验要求再生模板，只用`python tools/sync_templates.py --write`并核diff只含来源变更。
3. 保留旧2.1.3产物身份，2.1.4写新目录；真实源码SHA、组件版本、CLI/worker摘要和SDK归档摘要共同冻结。编译中不得切输入。

### U06 消费方联合验证

1. 调用方从本批SDK最新源码准备或复用同SHA/摘要的产物，更新其既有来源锁/构建回执；不维护消费副本源码。消费失败不回退2.1.3冒充完成。
2. 本次适配器重跑既有签名query探针：旧失败必须通过，普通查询仍通过；另完成尚未执行的大正文媒体/权限/重定向探针，不能用修复签名一项代替全部T04。
3. 通过真实worker的host-channel before-send路径验证宿主持久记录不出现合成签名；记录运行CLI/worker/宿主实际身份。只通过reference-host测试不能声明真实主程序已生效。
4. 不改历史冻结任务、包身份或数据库。若宿主消费字段出现真实不兼容，先定位实际owner再列文件，本次不预设主程序业务修复。

### U07 分发与跨机可获取

1. 按项目规则处理本次已授权提交并正常推送核远程包含；原失败用例和修复同批保留，不提交用户数据/Key/机器标记。
2. 发布完整SDK源码/运行工具归档和SHA256，来源锁指向可下载资源；从公开远程下载到全新目录解包、核全文件摘要及CLI/worker身份。不以本机存在当作远程可获取。
3. 分别记录源码、二进制、主程序集成、实际安装验收。调用方真实应用安装/GUI未做则写NOT_RUN，不代签另一机器。

### U08 交接与结束

1. 更新本文每项状态和`docs/INDEX.md`链接；调用方Kling计划只登记SDK任务状态和产物身份，不复制另一份修复任务。
2. U01–U06相关安全与集成均通过后才解除Kling T04的签名审计阻断；视频URL外部来源阻断仍独立存在。
3. 未完成的媒体预算、真实云端、安装与公开下载分别保留。回滚只回本次代码和候选，不删除远端付费任务/成果或覆盖原发行包。

## 4. 依赖、预算、授权和当前结论

顺序U01→U02→U03→U04→U05→U06→U07→U08，共享文件不得并行写。默认单执行者。修复与定向验证阶段建议90分钟（含准备/编译/等待）、一次正式根因修正；构建导出与跨机验证另批固定输入及预算，不能借换批无限重跑同一失败。

用户本轮明确要求**写SDK升级计划**。本次只写此计划与索引，未修改SDK生产源码或版本，未运行SDK修复后测试，未发布SDK补丁。确认缺陷不等于已完成修复；本计划不虚构2.1.4产物或通过回执。

## 5. 审核与实施记录

- 审核通过。缺陷所有者为参考宿主审计URL，实际网络请求由独立的请求构建路径发送。SDK与主仓在开工前完成同步。主仓快进至 `8d4249157b` 并合并两份交接记录。SDK基线 `050b7f1` 与远程齐平，2.1.4未占用。
- 将U04和U05的提交时序明确为先通过行为测试并提交冻结源码，再从干净提交生成可分发回执。公开归档、主仓消费与实际应用安装分别记账。
- 本批实施范围为SDK补丁和调用方联合验证，Kling视频URL来源及其它独立开发步骤按原计划跟进。
- U01 原共享 target 混有 rustc 1.94 缓存。使用独立 rustc 1.95 target 后 SDK 自包含用例按预期失败，服务器已收到完整签名。日志 `sdk-214-red-isolated.log`。
- U02–U03 补丁后 2 个单元测试和 2 个 HTTP 集成测试全部通过，覆盖审计查询脱敏、原始网络请求、拒绝不出网和保留正文。日志 `sdk-214-green.log`。
- U04 已核 2.1.4 为空闲版本。reference-host 和 tools 升补丁，ABI 沿用协议2及 WIT 2.0.0。Cargo 仅更新本批两个组件条目。
