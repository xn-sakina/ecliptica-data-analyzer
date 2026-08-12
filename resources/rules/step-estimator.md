# 回合启发式估计模型

本文档是 `current_step`、`until_boss_step` 和 `has_step_estimate` 的行为规格与调参依据。
以后修改模型时，应先更新本文档中的假设、公式和验证样本，再修改实现及测试。

实现位于 `src/analysis.rs` 的 `StepEstimator`。游戏 phase 的原始规则见
[`steps.md`](steps.md)。

## 1. 指标语义

| 字段 | 语义 |
|---|---|
| `current_step` | 当前/刚结束战斗在**完整一局**中的一基回合序号。不是玩家中途加入后观察到的回合数。 |
| `until_boss_step` | 从当前升级大厅开始，在进入 Jim 最终战之前预计还需完成的普通战斗回合数。值为 `0` 表示预计本次升级后下一战就是 Jim。 |
| `has_step_estimate` | 当前证据是否达到可展示门槛。为 `false` 时，前两个数值都不得作为有效数据展示。 |

这三个字段是估计值，不是游戏日志直接提供的权威回合编号。默认只通过回合战报模板展示；
OSC 渲染还要求当前存在 `round_report`，因此普通实时消息不会误用它们。

推荐模板写法：

```handlebars
{{#if has_step_estimate}}
预估第 {{current_step}} 回合｜距 Jim {{until_boss_step}} 回合
{{/if}}
```

## 2. 已知事实与经验先验

### 游戏规则事实

- phase modifier `x` 从 `0` 向 `1` 推进，`x == 1` 是 Eye of the Eclipse / Jim。
- phase 的每回合增量不是常数，会受到清怪速度影响。
- 慢速游玩可能让同一大阶段多停留一个回合，因此不能只对单个 phase 值做固定整数映射。

### 日志事实

Stage 日志提供时间戳和当前 phase：

```text
ECLIPTICA - now in stage: Stage_LostElysia on phase: 0.6089296 as class: Twinmage
```

估计器使用相邻两条去重后的 Stage 记录：

- `delta_phase = next.phase - previous.phase`
- `cycle_seconds = next.stage_time - previous.stage_time`

`cycle_seconds` 是两次进入战斗之间的完整周期，包含上一场战斗和随后大厅升级时间，
不是玩家启动软件后的在线时长。

### 当前经验先验

- 完整一局从 `phase = 0` 到打完 Jim 通常略多于 2 小时。
- 当前模型以 135 分钟作为完整局中心先验。
- 其中预留 20 分钟给 Jim 最终战；剩余 115 分钟用于约束进入 Jim 前的普通回合数量。
- 这是弱先验，且**最多**占最终步长模型 20% 权重。时间推导结果与房间内实际 phase
  推进冲突时，时间权重会自动降到 0。

重要：玩家中途加入后通常不会再在线 2 小时。模型**不得**把“本地加入后的累计时间”当作
本局已经进行的时间。完整局时长只用于把已观测的平均回合周期转换成合理的整局回合数先验。

## 3. 状态与输入处理

估计器保存：

- 最近最多 24 个有效 `PhaseObservation { second, phase }`；
- 是否亲眼观察到 `phase <= 0.001` 的本局起点；
- 从本局起点起观察到的 Stage 总数。总数单独累计，不受 24 条窗口裁剪影响。

输入规则：

1. 仅接受有限且位于 `[0, 1]` 的 phase。
2. 与上一条 phase 差小于 `0.000001` 的重复 Stage 忽略。
3. 新 phase 比上一条低超过 `0.01` 时，视为新局或缺失房间离开事件，重置估计器。
4. 离开房间、日志切换、截断或分析器全量重置时清空所有估计状态。
5. 新 Stage 和回到 intermission/lobby 时都会重新估算，以便持续纠错。

## 4. 有效样本与置信门槛

相邻 Stage 只有同时满足以下条件才用于学习：

| 条件 | 当前值 | 目的 |
|---|---:|---|
| 回合周期 | `180..=1800` 秒 | 排除重复日志、瞬时切房和异常长中断 |
| phase 增量 | `0.025..=0.16` | 排除重复、倒退和明显异常跳变 |
| 最少有效跨关样本 | 2 | 中途加入后至少完整观察两次 phase 变化才开放估计 |
| 校正后步长标准差 | `<= 0.035` | phase 推进不稳定时隐藏结果，而不是输出伪精确整数 |

不足 2 个有效样本，或样本离散度超过门槛时，`estimate()` 返回空值，
`has_step_estimate` 保持 `false`。

这意味着中途加入后通常需要看到 3 个 Stage，完成 2 次 Stage-to-Stage 转换后，战报才可能显示估计。

## 5. 当前计算模型

### 5.1 phase 推进模型

模型假定 phase 越高时单回合推进略微加速：

```text
next_phase = min(1, phase + intercept + 0.05 * phase)
```

`0.05` 是 `STEP_PHASE_ACCELERATION`，来自当前完整日志序列的经验拟合，不是游戏官方公式。

对每个有效样本，按照递推式使用前一个回合的 phase 移除加速项，得到本房间基础步长样本：

```text
sample_intercept = delta_phase - 0.05 * previous_phase
```

所有有效样本的平均值为 `mean_intercept`。

### 5.2 完整局时长弱先验

先用本地**已完成跨关样本**求平均完整回合周期：

```text
mean_cycle = average(stage[n + 1].time - stage[n].time)
prior_transitions = round((135min - 20min) / mean_cycle)
prior_transitions = clamp(prior_transitions, 8, 16)
```

然后通过二分搜索求出一个 `prior_intercept`，使上述 phase 推进模型从 `0` 经过
`prior_transitions` 次后到达约 `0.995`。

时间先验不是固定混入。先比较它与 phase 实测模型是否兼容：

```text
difference = abs(mean_intercept - prior_intercept)
compatibility = clamp(1 - difference / 0.025, 0, 1)
time_weight = 0.20 * compatibility

intercept = clamp(
    (1 - time_weight) * mean_intercept + time_weight * prior_intercept,
    0.035, 0.095
)
```

因此：

- 当前房间实际 phase 增量是主要信息；
- 完整局约 2 小时只在它与 phase 轨迹相容时提供最多 20% 的正则化；
- 若极高输出令 Jim 在 2 小时内出现，或慢局在 2 小时后才进入 Jim，时间推导的
  `prior_intercept` 与实测 `mean_intercept` 会产生明显分歧，时间权重随之下降；
- 当分歧达到 `0.025` 时，时间完全退出，回合数由稳定的 phase 增量轨迹决定；
- 慢局若在各 phase 区间实际多打一回合，会留下更密的 phase 轨迹，模型可以据此估计更多回合；
- 中途加入后的本地在线总时长没有进入公式。

### 5.3 `current_step`

如果观察到本局 `phase <= 0.001` 起点：

```text
current_step = 从该起点起去重后的 Stage 总数
```

此时回合序号是直接计数；即使 phase 预测仍是启发式，`current_step` 本身比中途加入场景可靠。

如果中途加入、没有观察到起点：

1. 从 `phase = 0` 开始反复应用当前推进模型；
2. 最多枚举 24 个回合；
3. 选择模拟 phase 与当前真实 phase 距离最小的回合序号。

这得到的是完整一局中的估计位置，不是 `observations.len()`。

### 5.4 `until_boss_step`

从当前 phase 开始反复应用推进模型，直到 phase 到达 `0.995`，得到
`transitions_to_final`。随后：

```text
until_boss_step = max(0, transitions_to_final - 1)
```

减一是因为最后一次转换本身会进入 Jim 战，不属于“Jim 之前还要打的普通回合”。因此在大厅：

- `until_boss_step == 1`：预计还要打一场普通战，之后再回大厅准备 Jim；
- `until_boss_step == 0`：预计当前升级结束后直接进入 Jim。

## 6. 中途加入与持续纠错

中途加入时没有历史回合数，处理原则如下：

1. 第一条 Stage 只提供当前 phase 位置，证据不足，不展示。
2. 第二条 Stage 只形成 1 个跨关样本，仍不展示。
3. 第三条 Stage 形成至少 2 个有效样本；若离散度合格，开放 `has_step_estimate`。
4. 后续每个 Stage 都增加 phase 增量和回合周期证据，重新计算基础步长、绝对回合和剩余回合。
5. 新观测与旧模型不一致时，数值允许上调或下调；这是预期的纠错行为。
6. 样本突然变得不稳定时，新估计可能不可用。展示层必须始终服从 `has_step_estimate`，不能缓存旧整数继续显示。

## 7. 当前验证基线

测试必须至少覆盖以下行为：

- 中途加入后不足 2 个有效跨关样本时 flag 为 false；
- 中途加入 phase 约 `0.6089 → 0.7092 → 0.8047` 后能够收敛；
- 估计不能用本地加入时间冒充完整局已用时间；
- 观察到 `phase = 0` 时 `current_step` 使用精确 Stage 计数；
- 类真实完整序列在 `phase ≈ 0.8047` 的大厅估计 `until_boss_step = 1`；
- 类真实完整序列在 `phase ≈ 0.9133` 的大厅估计 `until_boss_step = 0`；
- 同一 phase 轨迹在中途加入且每回合分别为 4、10、25 分钟时，不能被时间先验推成不同回合；
- 88 分钟和 165 分钟到达 Jim 准备大厅的完整路径都必须保留相同 Jim 边界；
- 慢速、低推进的 16 回合轨迹必须在完整加入与中途加入时都恢复 `15/1 → 16/0`；
- 时间先验与 phase 模型分歧达到 `0.025` 时，时间权重必须为 0；
- flag 为 false 时模板整段隐藏；
- flag 为 true 时数值 `0` 必须正常渲染，不能被当作缺失值；
- 新局 phase 倒退、离开房间和日志重置不得沿用旧估计。

对应测试目前集中在：

- `analysis::tests::step_estimate_waits_for_enough_late_join_evidence_and_then_corrects`
- `analysis::tests::full_run_prior_never_treats_late_join_time_as_elapsed_run_time`
- `analysis::tests::extreme_cycle_time_does_not_override_phase_path_for_late_join`
- `analysis::tests::extreme_full_run_duration_keeps_jim_boundary_stable`
- `analysis::tests::slow_run_can_add_rounds_without_time_prior_erasing_them`
- `analysis::tests::incompatible_time_prior_loses_all_weight`
- `analysis::tests::observed_phase_zero_makes_current_step_an_exact_ordinal`
- `analysis::tests::complete_fixture_like_run_reaches_one_then_zero_before_jim`
- `osc::tests::report_step_estimate_is_hidden_until_flagged_and_renders_zero`

## 8. 已知限制

- 该递推式是基于现有日志的经验模型，不是游戏源码公式。
- 规则允许慢速玩家在一个大阶段多打一回合；临界点附近估计可能随新证据修正。
- 完整局 135 分钟和 Jim 20 分钟是当前产品先验，不适用于所有队伍和未来版本。
- Stage-to-Stage 周期包含大厅时间。长时间挂机仍可能扭曲时间先验；只要它与 phase 实测明显
  冲突便会自动降权，但小幅且未越过兼容阈值的偏差仍可能影响临界点整数。
- 当前置信 flag 是阈值门控，不是经过校准的概率值。
- 目前输出单个整数，没有暴露候选区间。如果后续数据表明误差常为 ±1，应优先增加区间或置信等级，而不是继续伪装成绝对精确值。
- `until_boss_step` 表达的是模型预期；未来回合速度尚未发生，因此它本质上不能成为权威值。

## 9. 后续改进与调参流程

修改任何常量、公式或 flag 门槛前，应按以下流程执行：

1. 从每份日志提取按 Session/新局分组的 `(stage_time, phase)` 序列。
2. 标注是否从 `phase = 0` 开始、真实 Jim Stage、真实最终战前大厅位置。
3. 使用“按整局留出”验证，不能随机拆同一局的相邻回合，否则会高估泛化能力。
4. 分别评估完整加入和中途加入场景。
5. 至少记录：
   - `current_step` 精确命中率；
   - `current_step` 的 ±1 命中率；
   - `until_boss_step` 精确命中率；
   - 最终战前 `1 → 0` 边界准确率；
   - flag 覆盖率；
   - flag 为 true 时的错误率；
   - 中途加入后需要多少个回合才首次开放 flag。
6. 目标优先级应是降低“flag=true 但整数错误”的比例，其次才是提高 flag 覆盖率。
7. 新模型必须保留上述验证基线，并添加导致本次调参的真实回归样本。
8. 若修改字段语义、模板可见性或 `0` 的含义，必须同步更新双语变量帮助和默认战报预设。

推荐的下一步模型升级方向：

- 输出候选回合的分布或 `[min, max]` 区间；
- 将战斗耗时与大厅耗时拆开建模；
- 按 phase 大阶段分别学习步长；
- 引入最近样本加权，使玩家当前速度变化更快反映到预测；
- 用真实日志回放离线标定 `has_step_estimate` 的概率阈值；
- 将 135/20 分钟先验做成带版本的数据配置，而不是永久硬编码。

## 10. 行为契约

无论未来替换成线性回归、贝叶斯模型还是查表模型，都必须遵守：

1. 中途加入后的在线时长不等于完整局已用时。
2. `current_step` 是完整一局中的一基回合序号。
3. 大厅中的 `until_boss_step == 0` 表示下一战预计为 Jim。
4. 证据不足时必须关闭 `has_step_estimate`，不能用默认 `0` 冒充有效估计。
5. 新日志证据可以纠正之前的估计。
6. 房间/日志生命周期切换后不能泄漏旧局状态。
7. 展示和模板必须以 flag 为准，并正确保留有效的数值零。
