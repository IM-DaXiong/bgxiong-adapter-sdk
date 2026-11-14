# 图像与视频投影

协议仍为2，WIT、执行信封和媒体租约不变。包必须声明requiredFeatures中的`media-projection-v1`；旧宿主在执行前拒绝。禁止optional声明或与`image-projection-v1`同时声明。旧包及冻结任务仍按原声明执行，不转换历史结果。

适用于image.generate和video.generate的非流式操作。输入必须含必填字符串outputProjection，使用非空、无重复的闭合枚举，默认值在枚举内：flat、equirectangular180、equirectangular360。操作必须声明至少一个对应能力种类的必需输出。

成功结果data.mediaArtifacts为每个image/video输出提供唯一的mediaId、role、kind和projection。身份、角色、种类必须与实际outputs完全对应，投影必须与原始冻结请求相同。多个相同字节文件可以有不同媒体身份；不能靠摘要合并回执。audio/file不放入此列表。accepted仍使用现有受理回执；其余未成功状态不得发布视觉成果或投影条目。

真实媒体owner验证尺寸：180度等距柱状图像/视频必须为正方形，360度必须严格2:1，尺寸为正数且乘法不溢出；flat不加投影比例限制。视频使用显式配置的ProbeTool，缺少测量尺寸时全景结果被拒，禁止用请求尺寸填空。平台不得据此改分辨率或采样；不合法即失败。音轨、帧率、时长、色彩和姿态不是本profile新增字段，沿原媒体事实及各操作声明验证。

query、取消、任务恢复、retained inspect/export沿原任务身份与已接受ID；恢复仍启用原profile，不重提交生成。平台模型、Comfy图、球面绘制与裁切算法只属于适配器。

轻量验收复用`crates/tools/tests/fixtures/image_projection_sync.py`。默认模式保留旧图片profile；追加`--media-projection <absolute-ffprobe> <absolute-2:1-video>`时，guest须以media-projection feature构建。脚本使用本机回环HTTP，验证flat/180/360真实PNG、360真实视频、重复字节独立身份、retained导出，以及错误视频比例和缺探测尺寸拒绝。文件须由调用者显式提供；不查找PATH、不下载工具或模型。
