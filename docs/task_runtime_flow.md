# Task Runtime Flow

这份文档按当前项目实际实现整理，覆盖任务提交、工作区绑定、Agent 执行、MCP 重试、审阅决策，以及成功/失败/回退的完整流转。

```mermaid
graph TD
    classDef entry fill:#e8f1ff,stroke:#2a5fb0,stroke-width:2px;
    classDef actor fill:#eaf7ea,stroke:#2f8f46,stroke-width:2px;
    classDef db fill:#fff3e6,stroke:#d17b00,stroke-width:2px,stroke-dasharray: 5 5;
    classDef state fill:#f7ebff,stroke:#7b3fb4,stroke-width:2px;
    classDef fail fill:#ffe9e9,stroke:#c23b3b,stroke-width:2px;

    User[用户 / API 提交任务]:::entry --> Submit[DagOrchestrator.SubmitTask]:::actor
    Submit --> InsertPublished[写入 tasks<br/>status = published]:::db
    InsertPublished --> PickWorkspace{是否找到用户工作区}:::entry

    PickWorkspace -- 否 --> KeepPublished[记录告警并返回<br/>任务保持 published]:::state
    PickWorkspace -- 是 --> CheckAgent[CheckAvailableAgent]:::actor
    CheckAgent --> HasAgent{是否找到可用 Agent}:::entry

    HasAgent -- 否 --> BindWorkspaceOnly[仅写入 workspace_name]:::db
    BindWorkspaceOnly --> WaitAssign[任务保持 published<br/>等待后续分配]:::state

    HasAgent -- 是 --> StartAgent[StartAgent<br/>启动或复用 AgentActor]:::actor
    StartAgent --> MarkAccepted[更新 tasks<br/>assigned_agent_id + status = accepted]:::db

    subgraph Runtime [Agent Runtime]
        TickLoop[run_interval 轮询]:::actor --> ExecuteMcp[发送 ExecuteMcp 给 McpAgentActor]:::actor
        ExecuteMcp --> McpResult{执行结果}:::entry
        McpResult -- 成功 --> MarkSubmitted[更新 tasks<br/>status = submitted]:::db
        McpResult -- 失败但可重试 --> RetryBackoff[记录失败次数并指数退避]:::fail
        McpResult -- 失败且不可重试 --> MarkFailureDirect[更新 tasks<br/>status = completed_failure]:::fail
        RetryBackoff --> TickLoop
    end

    MarkSubmitted --> BeginReview[BeginTaskReview<br/>status: submitted -> under_review]:::db
    BeginReview --> ReviewTask[TaskAgent 审阅执行结果]:::actor
    ReviewTask --> SaveReview[写入 task_reviews]:::db
    SaveReview --> UserDecision{用户是否接收审阅结果}:::entry

    UserDecision -- 接收 --> ReviewApproved{审阅建议是否通过}:::entry
    ReviewApproved -- 是 --> MarkSuccess[更新 tasks<br/>status = completed_success]:::db
    ReviewApproved -- 否 --> MarkFailureReviewed[更新 tasks<br/>status = completed_failure]:::fail

    UserDecision -- 拒绝 --> ResetPublished[清空 assigned_agent_id<br/>删除 task_reviews<br/>status = published]:::db
```

## 关键结论

1. 任务初始一定是 `published`
2. 执行成功后才会进入 `submitted` 和后续审阅
3. 执行失败且不可重试时会直接进入 `completed_failure`
4. 用户拒绝审阅结果不会直接失败，而是回到 `published`
5. submitted 有回补扫描机制，避免任务永久卡住
