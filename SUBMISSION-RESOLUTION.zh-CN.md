# 在线核验未知受理请求

SDK 2.2.0 的 `submission-resolution-v1` 是可声明的语义能力。协议仍为 2，WIT 仍为 2.0.0，六操作不变；使用 `probe` 的两个明确 kind，不改变普通连通性或 metadata-only operation_preflight。

## 提交前冻结

具有该 feature 的异步操作在实际 submit 之前，参考宿主自动调用 `submission_identity`。输入包含原 `requestId`、完整冻结请求 `requestDigest`、checkpoint 的字符串 `expectedRevision`、原包身份、binding 和 configuration。适配器返回同身份及 `status: ready`、`submissionScope: {storeId, instanceId}`。

`storeId` 标识持久受理记录域，不能随进程重启改变。`instanceId` 是首次服务实例，作为原受理来源保留。SDK 用已有 CAS 写入 prepared checkpoint 的 submissionScope，然后才提交 `acceptance_unknown` 并启动实际 submit。嵌套探针继承总 deadline 和取消，但自身资源清理不会取消父提交。

适配器读取原 `host-checkpoint.read` 获取 requestDigest 和 submissionScope。真正的 POST 必须核 expectedStoreId；若存储已替换，拒绝提交，不能创建同 ID 的另一份任务。服务端应在返回 accepted 之前持久保存原请求、受理号、恢复令牌所引用的资料及重启所需输入。

## 恢复

宿主使用原包/绑定和原 checkpoint request-id 调用 `probe`，kind 为 `submission_resolution`，输入同一 requestDigest/expectedRevision/submissionScope。结果只能是：

- `accepted`：携带标准 `accepted` 操作回执。即使服务端已经完成，仍给出可查询原成果的引用，随后按标准 query 获取媒体。SDK 校验原操作的 resume schema 后 CAS 更新原 checkpoint。
- `rejected`：携带 `rejection: {tombstoneId, fencesLateSubmit: true}`。服务端必须在同一持久受理记录域中原子确认不存在已受理任务，并写入 tombstone，保证并发或迟到的原 POST 永远被拒。SDK 保存这份证据；不会自动提交新的付费请求。
- `unknown`：保留原未知状态。超时、404、换库、存储丢失、不知道旧请求是否受理，都不能转换为 rejected。

结果必须逐项回显 kind/requestId/requestDigest/expectedRevision 和原 submissionScope。重启后的服务 instanceId 可以不同，但 storeId 必须仍是原持久域。旧包未声明 feature、旧 checkpoint 没有 scope，均不通过新服务的空库“解锁”。已确认的旧任务继续用原 query 合同。

完整 JSON：[submission-resolution.schema.json](schemas/submission-resolution.schema.json)。既有离线 operator-reviewed `prepare-resolution` 继续独立存在；在线探针不伪造人工审核凭证。

## 独立会话只读检查

`bgx-adapter-v2 session-inspect --workspace ABS --request-id ORIGINAL` 只打开已有 SDK 标记工作区和只读 checkpoint store，返回 `{schemaVersion:1, requestId, checkpoint}`。此命令不启动服务、不写数据库、不提交或重试，也不把缺失工作区/存储当作远端未受理。产品使用该公开投影续办自己的 per-request 语音工作区，不读取 SDK 私有 SQLite 布局。

取消受理后的异步任务时，cancel 是请求；必须继续 query 到实际 terminal 才能释放任务资源。若取消期间服务仍成功完成，成果留在原 checkpoint，用户操作可显示取消，不能删除成果后重做。

普通语音首次提交前使用 `workspace-create --workspace NEW_ABS` 沿 SDK owner 初始化唯一工作区及检查点库。随后 `session-inspect` 仅在原库存在且原请求无行时返回 null；缺失/损坏的原库明确报错，绝不重建或推定未受理。重复创建已占用工作区拒绝。
