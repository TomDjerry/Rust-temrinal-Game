# Game Runtime Refactor Design

## Goal
在不改变现有玩法、存档语义与 UI 行为的前提下，重构 `src/game/mod.rs`，把运行时逻辑拆分为更稳定的领域模块，降低后续功能开发和维护成本。

## Why Now
当前项目功能可用且测试较完整，但核心风险已经集中在 `src/game/mod.rs`：

- 文件体积过大，当前已超过 2600 行
- 单文件同时承担输入分发、AI、背包、任务、存档、UI 快照与测试
- 新功能仍在持续增加，继续堆叠会放大回归风险
- `cargo clippy --all-targets --all-features -- -D warnings` 不能全绿，说明结构和风格整理已到必要时机

这说明当前最需要的不是新增机制，而是先把核心运行时整理为可持续扩展的结构。

## Non-Goals
本轮重构不会做以下事情：

- 不新增玩法或平衡调整
- 不改变输入映射、战斗规则、AI 决策规则、任务规则
- 不改变存档字段语义和兼容策略
- 不引入 ECS、事件总线或其他大规模架构迁移

## Refactor Strategy
采用“中等强度模块化”方案：保留 `Game` 作为运行时聚合根和顶层编排入口，把具体行为按领域拆到同目录子模块中。

### Planned Modules
- `src/game/mod.rs`
  - 保留核心类型定义、模块声明、顶层 `run` 入口
  - 保留必要的跨模块共享常量与主编排逻辑
- `src/game/actions.rs`
  - 输入动作分发
  - UI 模式分支
  - 玩家行动与回合消费入口
- `src/game/inventory.rs`
  - 背包条目操作
  - 装备/卸下
  - Buff 结算
  - 物品使用与拾取辅助逻辑
- `src/game/contracts.rs`
  - 支线合约进度
  - 奖励发放
  - 主线必需任务物辅助逻辑
- `src/game/ai.rs`
  - 怪物回合
  - Alert / Flee / Patrol 行为
  - 噪音响应与移动决策
- `src/game/save.rs`
  - `SaveState`
  - 存档读写与状态转换
- `src/game/snapshot.rs`
  - `UiSnapshot` 所需映射逻辑
  - 背包/装备/合约展示文案组装
- `src/game/util.rs`
  - 公共小工具，如 `strip_bom`

## Testing Strategy
重构采用“行为不变”的 TDD 路线：

1. 先补针对拆分边界的测试或迁移现有测试。
2. 每拆一块模块就运行针对性测试，再跑全量测试。
3. 最终要求：
   - `cargo test`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo build`

测试组织也会同步整理，把 `src/game/mod.rs` 中按领域聚集的测试迁移到更贴近实现的位置，但不追求一次性重写整个测试结构。

## Migration Principles
- 每次提取只移动一个领域，避免同时跨多个责任区改动
- 优先抽离“纯逻辑 / 展示映射 / 工具函数”，降低借用与可见性复杂度
- 通过 `impl Game` 在子模块中扩展方法，避免过早重塑 `Game` 结构
- 先消除重复和 `clippy` 问题，再进行下一轮功能开发

## Success Criteria
重构完成后应满足：

- `src/game/mod.rs` 明显瘦身，不再承载大部分业务细节
- AI、背包、合约、存档、快照逻辑拥有清晰文件边界
- `strip_bom` 等重复逻辑被统一收敛
- `cargo clippy --all-targets --all-features -- -D warnings` 全绿
- 现有功能与测试行为保持不变

