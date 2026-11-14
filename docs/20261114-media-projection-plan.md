# 180度图片与全景视频投影合同

状态：实施前文件级计划及自审。基线de24abf3/SDK2.3.0已由消费者完成实际EXE文件上传与场景继承验收，本阶段不改消费者。

## 实物和边界

新增协商feature `media-projection-v1`，沿协议2、现有image.generate/video.generate、六操作、媒体租约、检查点与retained恢复。声明flat/equirectangular180/equirectangular360；结果逐项绑定mediaId、role、kind、projection，拒绝重复、遗漏、错身份和请求投影不符。180度正方形、360度2:1由真实解码/探测尺寸验证；视频缺探测尺寸显式拒绝，不以文件后缀或请求值代替实测。

已有image-projection-v1及冻结包保留原语义；新包不得同时声明两个投影feature，不自动转换历史请求/结果。音频、模型文件与平台的工作流/姿态/模型选择不进入本profile。帧率/音轨沿已有媒体事实和应用验收，不制造未经测量的新字段。

## 文件范围和实施顺序

1. `profiles/media-projection-v1/`：projection/output/input example schema、中英文语义及反例。`protocol/host-feature-catalog.json`登记必须协商的能力。
2. `crates/validator/src/media_projection.rs`负责图视频公共校验；`context.rs`、`input.rs`、`result.rs`、`capabilities.rs`、`package.rs`、`schema_registry.rs`接入原装配，不新造执行owner。已有image profile不放宽枚举。
3. `crates/reference-host/src/session.rs`在原submit/query/recovery上下文启用profile，使用同一broker的尺寸；`crates/tools/src/retained.rs`在原成果读取中核结果，不另提交任务。
4. 扩展现有validator结果/registry行为与CLI真实媒体fixture，覆盖180图、360视频、缺尺寸、错媒体身份、旧宿主拒绝、原ID恢复。先最小链，再检查模板/生成镜像/发行精确文件清单。
5. SDK正常升版2.4.0，按现有工具构建、提交推送并冻结摘要。之后消费者单独接入，不在同一验证过程修改本SDK基础。

## 验收与停止条件

本阶段上限3小时，定向修正最多2轮；真实旧功能回归或合同缺口保存现场，报告受影响阶段，按用户持续修复授权给出最小修正，不重跑全仓或改变断言求绿。验收不下载大模型、不调用真实付费服务。SDK通过只表示通用协议/媒体链通过，产品UI、Comfy推理与资源机验收分别记录。

自审：协议唯一、feature拒绝发生在执行前、原请求身份保全、尺寸来自媒体owner、平台语义不进入SDK、旧包不改写、消费者冻结、无静默降采样或音轨删除。无需用户再次批准，继续按已授权范围实施。

## 初次定向验收

2026-11-14：结果目标3项、能力协商2项通过，包含原image-projection-v1回归。独立CLI/worker（release构建2分31秒）、真实WASM guest和回环HTTP验证flat/180/360 PNG的结果/检查点/独立身份/导出字节；随后360视频及错误比例、缺实际探测尺寸拒绝通过。视频为调用者现有FFmpeg生成的64×32、24fps、半秒、1886字节纯色文件，不是模型推理。

首轮视频请求被binding合同拒绝，根因为复用脚本固定capabilitySlot=image；按真实能力声明改为video后只补视频链，没有重复已通过三条图片链。原失败回执保留，未放宽SDK校验。SDK升版与冻结构建、消费者接入仍待完成；当前不能记产品视频可用。
