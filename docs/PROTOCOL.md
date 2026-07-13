# 协议边界

协议定义在 `crates/riichi-proto/src/messages.rs`，当前通过 Rust 类型和 Serde 表达客户端/服务端边界。

## 客户端到服务端

- `ClientMessage::TurnAction`：当前玩家的摸切、打牌、立直、暗杠等行动。
- `ClientMessage::CallResponse`：响应窗口中的荣和、吃、碰、杠或 Pass。

行动是否可执行由 engine 决定；客户端提交的消息不是授权，也不能绕过合法性检查。

## 服务端到客户端

`ServerMessage` 传递状态更新、事件、行动请求、鸣牌请求、回合结果和分析信息。`GameStateView`、`PlayerView` 等视图只暴露当前玩家可见的信息，不应包含其他玩家的隐藏手牌。

分析信息包括牌效、听牌和弃牌建议，属于辅助展示数据，不改变对局结果。服务端和客户端应允许未来增加事件类型；未知事件不能导致已完成对局被错误回滚。

## 演进约束

修改消息时同步检查：序列化派生、客户端 view 转换、服务端 protocol 转换、UI 分支和协议测试。能兼容旧字段时优先新增可选字段；改变枚举语义或结算顺序时必须同步更新 [`docs/RULES.md`](RULES.md)。
