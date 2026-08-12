== Phases ==

> 分析器如何根据本规则估算 `current_step`、`until_boss_step` 和
> `has_step_estimate`，见 [回合启发式估计模型](step-estimator.md)。该文档同时记录了
> 当前参数、置信条件、已知限制和后续调参方法。

The game is split into 5 phases, with the final boss at the end:

* Primal
* Penumbral
* Antumbral
* Umbral
* Eclipse
* Eye of the Eclipse
[[File:Difficultywheelnew.png|The Phase Wheel as seen in-game. It spins to show you what the current phase is after every shop.|center|frameless]]

== Timescales ==

* Spending 15-20 minutes in a phase advances the clock the least, at 24%.
** Due to this, it is actually possible to have a 5-round Primal Phase.
* Spending around 5 minutes in a phase advances the clock the most, at 33-34%.

== The Phase Modifier ==
The Phase Modifier for Ecliptica is variable that helps decide what enemies to spawn, and what boss you will face.

A wheel will appear after every shop, that spins to show you where you are in terms of phase.

The ranges are tabulated below:
{| class="wikitable mw-collapsible" style="background:#000000; color:#ffffff; border:1px solid #444;"
|+Phase Ranges
! style="background:#000000; color:#ffffff;" | Range
! style="background:#000000; color:#ffffff;" | Phase
|-
|0 < x < 0.2
|Primal
|-
|0.2 < x < 0.4
|Penumbral
|-
|0.4 < x <0.6
|Antumbral
|-
|0.6 < x < 0.8
|Umbral
|-
|0.8 < x < 1
|Eclipse
|-
|1
|Eye of the Eclipse
|}
As you play, the value (x), will increase depending on how quick you manage to finish the waves of enemies. Being slower means you will be stuck in that phase for a round longer.
