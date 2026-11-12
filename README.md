# 统一开放适配标准与 SDK 开发者预览版

比格熊官网 [www.bgxiong.com](https://www.bgxiong.com)｜微信公众号：天途影像｜[比格熊开发治理 Skill](https://www.bgxiong.com/goods/9p65SRB2Sl)

软件只提供一套平台无关的接入标准。官方 API、自建服务、云端聚合服务、ComfyUI 工作流，都通过同一个适配器接口接入；“官方”只表示包的作者和发布来源，不代表专用执行通道。用户的智能体只凭公开资料制作包，软件导入后发现其能力、显示参数并执行，无须项目方为每个服务修改主程序。

本仓库是这套标准及其 SDK 的公开开发源：[bgxiong-adapter-sdk](https://github.com/IM-DaXiong/bgxiong-adapter-sdk)。说明文档采用 [MIT](LICENSE)；适配器包和主程序仍使用各自许可。

## 作用与意义

比格熊这类宿主软件需要接入官方 API、自建服务、云端聚合服务和 ComfyUI 工作流等不同来源。如果每多一个平台就改主程序，软件会变成各家协议的集合，作者也无法只凭公开资料独立制作接入包。

本仓库只提供一套平台无关的接入标准，以及按该标准制作、校验和执行适配包的工具。差异留在包内：包声明能力与参数，把公共输入转换成目标服务报文，再把结果和错误转换成标准输出。宿主负责安装与信任、展示参数、校验、执行调度、媒体、任务与恢复，不解释某一家的端点、节点或错误码。因此：

- 同一套适配器接口可以接入不同类型的服务，不必为“官方”另开专用执行通道。
- 用户的智能体可以只凭本仓库公开资料制作包；软件导入后发现能力、显示参数并执行。
- 新增服务、模型、工作流、参数和业务操作，在已支持的公共输入、输出和交互原语内，通过安装适配包完成，无须项目方为每个服务修改主程序。
- 全新的媒体类型、硬件权限或交互原语仍需标准升级；不能声称未来所有未知功能都可以零修改主程序接入。

ComfyUI 适配和视频工作台是下游需求样本，不定义 SDK 边界。桌面客户端源码不在本仓库。

## 谁应该阅读

- **适配包作者和用户智能体**：按公开标准和作者工具独立制包、检查和打包。从 [预览版状态与使用边界](DEVELOPER-PREVIEW.zh-CN.md) 和 [作者指南](AUTHORING.zh-CN.md) 开始。
- **宿主集成方**：按同一接口发现能力、展示参数并执行。见 [参考宿主集成](HOST-INTEGRATION.zh-CN.md) 和 [模块接入清单](HOST-ADOPTION-CHECKLIST.zh-CN.md)。
- **包的使用者**：已发布的第三方包、下载和每包验收在适配器仓，不在本仓库重复一份。见下方「已接入的第三方适配器」。

## 包与宿主的职责

主程序（宿主）负责：包安装和信任、用户授权、配置与凭据保存、能力索引、通用 schema 校验和控件、媒体句柄、预算与取消、任务记录、持久恢复、用户界面和结果保存。宿主不解释厂商端点、节点、错误码或私有状态；也不因某包的发布来源是“官方”而改走另一套执行内核。

适配包负责：声明模型、能力、输入、参数和输出；将公共输入转换成目标服务报文；上传、请求、查询、取消；把厂商结果、真实用量与错误转换为标准结果。适配包不能读写应用数据库、获取画布组件或直接写素材库。平台端点、鉴权报文、工作流节点、参数编码和厂商错误映射只存在于适配包中。

配置型适配和代码型适配是同一包接口的两种制作方式。已有的 worker、沙箱、媒体、凭据、权限、任务、恢复和包安装系统继续作为实现基础，升级后消费同一标准，不新建并列运行内核。

更完整的责任边界见 [开放标准](OPEN-STANDARD.zh-CN.md)。

## 最短入门

1. 先读 [预览版状态与使用边界](DEVELOPER-PREVIEW.zh-CN.md)，确认当前交付是开发者预览，以及已验范围和未验边界。
2. 制作包从 [作者指南](AUTHORING.zh-CN.md) 开始；独立作者任务见 [AUTHOR-TASK.zh-CN.md](AUTHOR-TASK.zh-CN.md)。
3. 若使用应用安装目录中的公开样例：`user-examples/vendor-adapter-v2/` 提供完整公开 SDK；`user-examples/comfy-workflow-authoring-v2/` 提供任务书、工作流材料和 12 套自包含作者包。在独立目录完成 Rust/WASM 编译、标准 check/pack 后，从「设置 → 第三方适配器 → 安装适配包」安装，并在工作流详情关联准确身份、版本、操作和原件。需要宿主配置时展开「高级安装选项」，使用样例生成的完整 `host-config.app.json`。
4. 在本仓库独立 checkout 构建时，使用 `rust-toolchain.toml` 固定的 Rust 与 `wasm32-wasip2`，并准备 Python 3.11+、Node 25+ 和对应本机链接工具。调用仓内脚本时传入尚不存在的绝对输出目录，其父目录须已创建：

```powershell
pwsh -File .\tools\build_sdk.ps1 -Profile debug -OutputDirectory <absolute-new-directory>
```

输出含 `source/`、`runtime/` 和 `build-receipt.json`。回执绑定 SDK 提交、工作区状态、工具链、源码和运行文件摘要，验证范围是构建本身。参考宿主和 CLI 的行为验收要单独运行。当前独立仓迁移尚未切换比格熊主程序消费入口。

## 技术文档入口

建议按这个顺序阅读合同：

1. [开放标准](OPEN-STANDARD.zh-CN.md)：责任边界、配置、能力、参数扩展、执行与版本。
2. [SDK 接口规范](SDK-CONTRACT.zh-CN.md)：包结构、调用字段、Rust/WASM 接口与生命周期。
3. [标准验证和发布条件](CONFORMANCE.zh-CN.md)：什么证据才能证明标准可独立使用。
4. 公开 schema 与样例：[能力声明](schemas/capability-contract.schema.json)、[调用](schemas/operation-payload.schema.json)、[事件](schemas/stream-event.schema.json)、[示例](examples/capabilities.json)。同目录还有信封、响应、结果、媒体、用量和 UI schema。纯 schema 校核不替代真实运行验收。

相关入口：

- [作者指南](AUTHORING.zh-CN.md) 与英文增补说明 [AUTHORING.en.md](AUTHORING.en.md)
- [参考宿主集成](HOST-INTEGRATION.zh-CN.md)
- [独立作者任务](AUTHOR-TASK.zh-CN.md)
- [模块接入清单](HOST-ADOPTION-CHECKLIST.zh-CN.md)
- [请求/回执分派表](operation-contracts.json)（固定六类传输操作）
- [跨领域能力样例](examples/coverage-capabilities.json)
- [变更记录](CHANGELOG.md)、[发行状态](RELEASE-STATUS.json) 与本次整理记录 [docs/INDEX.md](docs/INDEX.md)

设计校核：安装 `tools/requirements-design.txt` 后，运行 `python tools/validate_design.py`（可从任意工作目录执行脚本的实际路径）。该检查只读，不发网络请求、不构建 SDK、不执行模型。三个可编译 Rust/WASM 示例位于 `examples/sync-api`、`examples/cloud-task`、`examples/workflow`。真实执行、完整故障矩阵与候选就绪须分别有验收证据。标准要求完整的外部独立作者验证，设计样例不能替代它。发布候选的许可库存不授予新的所有者许可或外部分发授权。

## 当前范围与边界

当前公开状态以 [预览版说明](DEVELOPER-PREVIEW.zh-CN.md) 和 [RELEASE-STATUS.json](RELEASE-STATUS.json) 为准，不要把规划或局部候选写成已经完成的正式发行。

- 协议、WIT 与 author SDK 身份仍为 **2.0.0**。`RELEASE-STATUS.json` 中的 `sdkRelease` 现为 **2.1.3**，对应工具、profile 与参考宿主等增量；这不是完整 SDK 治理通过。
- 当前交付定位为 **ComfyUI 工作流作者开发包·开发者预览版**。应用安装发行记录为 **0.6.1030**。`developerPreview` 单独记录这次有限交付。
- 当前接入范围是已声明输入输出原语内的图片生成、视频生成和视频编辑。图片、视频生成的已验入口以自由画布工作台及各样例 README 为准；视频编辑以 Wan 样例为准。SDK 通用 schema 不意味着每个产品表单支持任意根字段或媒体组合。模型、节点、素材、鉴权和尺寸/时长要求由具体工作流材料说明。普通工作流能直接用现有工作流管理接入时，优先复用现有管理能力。
- `RELEASE-STATUS.json` 的 `scope` 明确：旧 G00—G06、`sdkReady=false`、`moduleIntegration` 等字段属于独立 SDK 完整候选治理，不是当前应用功能否定清单。尚未完成的完整 SDK 验收仍保持未验。v1 厂商插件原有接口继续见安装目录 `user-examples/vendor-adapter-v1/`；本预览版不承诺所有厂商或业务槽已接入 v2。
- SDK 制作和验证在独立 workspace 或参考宿主完成，不修改主程序构建、安装组件和用户数据；通过独立验收后，各模块再按自己的准备程度接入。未接入模块继续原有完整实现；已接入模块不设置失败回退或旧字段兼容。`SDK_READY` 不等于主程序已支持 v2。
- P01—P13 的代表性编译、安装关联、真实生成和回收结论按原基线复用；0.6.1028 另有新 Z-Image 工作流零外部技术提示独立作者闭环。该新目标包未收进上述 12 套安装样例，也没有据此证明新视频、所有工作流或所有操作系统。0.6.1030 沿用这些已有证据，资料一致性及实际安装检查以该版发行回执为准，不把旧实跑写成本次新实跑。

### Native 媒体消费工具候选

协议、WIT、author SDK 2.0.0 保持不变；native tools/reference-host 的只读 inspect/export 候选见 [受管媒体工具](RETAINED-MEDIA.zh-CN.md)。原已验执行工具独立保留，新工具的通过范围不等于整体发行通过。

### 音频输入（SDK 2.1.3）

独立 crate `bgx-adapter-v2-media` 提供 `inspect_file`，与参考宿主使用同一套实测媒体实现。宿主可以在不依赖参考宿主数据库的情况下消费它。容器感知的音频检查需要显式配置本机 ffprobe。已用真实文件练习的格式包括 WAV/RF64、MP3、FLAC、M4A/AAC/ALAC、ADTS AAC、Vorbis/Opus、ASF/WMA、AIFF、AMR-NB/WB、APE 和音频 WebM/Matroska；内嵌图片不会把音频变成视频。原始字节和严格租约身份保持不变。播放列表，以及包含真实视频流的音频请求会被拒绝。这不证明任何厂商上传端点或模型已经验收。

真实夹具回归：将 `BGX_AUDIO_FIXTURES` 设为 `{name,path,sha256}` 的 JSON 数组（否定用例加 `reject:true`），将 `BGX_REAL_FFPROBE` 设为显式二进制，将 `BGX_AUDIO_FACTS_OUT` 设为新的输出路径；然后运行 `cargo test -p bgx-adapter-v2-reference-host --test media real_common_audio_formats_import_and_rehydrate -- --ignored`。测试会导入无后缀文件，核对宿主检查器与 broker 事实，按有界分块读取，并在重启后恢复。媒体夹具内容不是 SDK 发行的一部分。

### 媒体槽引用载体

`mediaSlots.inputPointer` 指向一个输入字段，其操作输入 schema 声明媒体 ID 字符串、单键 `{ "mediaId": "..." }` 对象，或这些引用的数组。例如，`/source/mediaId` 可以指向字符串，`/referenceMediaIds` 可以指向字符串数组；已有的对象引用仍然有效。它不会改写输入，也不会推断路径。所有形式共用同一套已声明的数量边界、重复拒绝、request.media ID 闭包和 mediaKind 校验。Broker 检查仍然要求匹配包、绑定、请求所有权和实测媒体事实的有效句柄。媒体 ID 从来不是文件路径，也不额外授予文件系统访问权。

## 关于比格熊 / About

比格熊数字导演工作站是装在本机的 AI 导演工具。本仓库只放适配协议、作者工具和参考宿主。桌面客户端源码不在本仓库，公开页面也不提供主程序仓库地址。

- 官网 / 下载：[https://www.bgxiong.com](https://www.bgxiong.com)
- 版本说明：[https://www.bgxiong.com/client/version.html](https://www.bgxiong.com/client/version.html)
- 公开开发文档：[https://github.com/IM-DaXiong/bgxiong-ai-story-docs](https://github.com/IM-DaXiong/bgxiong-ai-story-docs)

## 已接入的第三方适配器

第三方适配器是本 SDK 的消费方：各厂商、云端和工作流包都按这里的协议制作，再由比格熊导入运行。“官方”只说明包的作者和发布来源，不代表这些包拥有专用执行通道。包的源码、下载和每包实际验收都在适配器仓，不在本仓库重复一份。

- 适配器仓：[https://github.com/IM-DaXiong/bgxiong-third-party-adapters](https://github.com/IM-DaXiong/bgxiong-third-party-adapters)
- 当前包和下载从该仓 [README](https://github.com/IM-DaXiong/bgxiong-third-party-adapters/blob/main/README.md) 进入。仓库里写明已经在应用中实测的，包括 Wan 2.2 S2V，以及 MiniMax H3-Image 的安装、文生图、参考图编辑、自由画布、片段画布和同任务恢复。其余包以该仓 README 和对应验收记录里的范围为准，未写实测的不算已成功出片。

## 许可证

本仓库说明文档采用根目录 [LICENSE](LICENSE) 的 MIT 许可。适配器包、主程序和第三方依赖不因此改许可。

比格熊官网 [www.bgxiong.com](https://www.bgxiong.com)｜微信公众号：天途影像｜[比格熊开发治理 Skill](https://www.bgxiong.com/goods/9p65SRB2Sl)
