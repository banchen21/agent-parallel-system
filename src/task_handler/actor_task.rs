use actix::prelude::*;
use std::collections::HashMap;
use tokio::time::{Duration, sleep};
use tracing::info;

// ==========================================
// 1. 数据结构与消息定义
// ==========================================

/// 任务定义
#[derive(Clone, Debug)]
pub struct TaskDef {
    pub id: String,
    pub depends_on: Vec<String>, // 依赖的前置任务 ID 列表
    pub payload_duration: u64,   // 模拟任务执行所需的时间（秒）
}

/// 提交整个 DAG 图的消息
#[derive(Message)]
#[rtype(result = "Result<(), String>")]
pub struct SubmitDag {
    pub tasks: Vec<TaskDef>,
}

/// 内部消息：单个任务执行完成
#[derive(Message)]
#[rtype(result = "()")]
struct TaskCompleted {
    pub task_id: String,
}

/// 内部消息：单个任务执行失败
#[derive(Message)]
#[rtype(result = "()")]
struct TaskFailed {
    pub task_id: String,
    pub reason: String,
}

/// 任务当前的状态
#[derive(Debug, PartialEq, Clone)]
enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

// ==========================================
// 2. DAG 调度器 Actor
// ==========================================

pub struct DagOrchestrator {
    // 任务的元数据存储
    tasks: HashMap<String, TaskDef>,
    // 任务的当前状态
    status: HashMap<String, TaskStatus>,
    // 邻接表：Key 是前置任务，Value 是依赖它的下游任务列表 (A -> [B, C])
    downstream: HashMap<String, Vec<String>>,
    // 入度表：记录每个任务还有几个前置依赖未完成
    indegree: HashMap<String, usize>,
}

impl DagOrchestrator {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            status: HashMap::new(),
            downstream: HashMap::new(),
            indegree: HashMap::new(),
        }
    }

    /// 执行具体任务的逻辑 (模拟)
    fn spawn_task(&mut self, task_id: String, duration: u64, addr: Addr<Self>) {
        // 更新状态为运行中
        self.status.insert(task_id.clone(), TaskStatus::Running);
        info!("🚀 任务 [{}] 开始执行...", task_id);

        // 使用 tokio::spawn 扔到后台异步执行，不阻塞 Actor
        tokio::spawn(async move {
            // 模拟真实的任务处理耗时 (例如网络请求、DB读写、复杂计算)
            sleep(Duration::from_secs(duration)).await;

            // 任务完成后，给 Actor 发送完成消息
            addr.do_send(TaskCompleted { task_id });
        });
    }

    /// 检查 DAG 是否全部执行完毕
    fn check_workflow_completed(&self) {
        let all_done = self.status.values().all(|s| *s == TaskStatus::Completed);
        if all_done && !self.tasks.is_empty() {
            info!("🎉🎉 所有 DAG 任务执行完毕！🎉🎉");
        }
    }
}

impl Actor for DagOrchestrator {
    type Context = Context<Self>;
}

// ==========================================
// 3. 处理提交 DAG 的消息
// ==========================================
impl Handler<SubmitDag> for DagOrchestrator {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: SubmitDag, ctx: &mut Self::Context) -> Self::Result {
        info!("📦 收到新的 DAG 任务流，正在解析图结构...");

        // 1. 初始化图结构
        for task in &msg.tasks {
            self.tasks.insert(task.id.clone(), task.clone());
            self.status.insert(task.id.clone(), TaskStatus::Pending);
            self.indegree.insert(task.id.clone(), task.depends_on.len());
            self.downstream
                .entry(task.id.clone())
                .or_insert_with(Vec::new);
        }

        // 2. 构建邻接表 (Downstream)
        for task in &msg.tasks {
            for dep in &task.depends_on {
                if !self.tasks.contains_key(dep) {
                    return Err(format!("任务 {} 的依赖 {} 不存在于图中!", task.id, dep));
                }
                // 将当前任务加入其依赖项的下游列表中
                self.downstream.get_mut(dep).unwrap().push(task.id.clone());
            }
        }

        // 3. 寻找所有入度为 0 的任务（即没有依赖的根任务）
        let mut ready_tasks = Vec::new();
        for (id, &degree) in &self.indegree {
            if degree == 0 {
                ready_tasks.push(id.clone());
            }
        }

        if ready_tasks.is_empty() && !msg.tasks.is_empty() {
            return Err("DAG 解析失败：图中存在循环依赖 (Cycle)！无法找到起点。".to_string());
        }

        // 4. 并发启动所有就绪的任务
        let addr = ctx.address();
        for task_id in ready_tasks {
            let duration = self.tasks.get(&task_id).unwrap().payload_duration;
            self.spawn_task(task_id, duration, addr.clone());
        }

        Ok(())
    }
}

// ==========================================
// 4. 处理任务完成的消息
// ==========================================
impl Handler<TaskCompleted> for DagOrchestrator {
    type Result = ();

    fn handle(&mut self, msg: TaskCompleted, ctx: &mut Self::Context) -> Self::Result {
        let task_id = msg.task_id;
        info!("✅ 任务 [{}] 执行完成！", task_id);

        // 1. 更新当前任务状态
        self.status.insert(task_id.clone(), TaskStatus::Completed);

        // 2. 查找依赖于该任务的下游任务
        if let Some(downstream_tasks) = self.downstream.get(&task_id).cloned() {
            let addr = ctx.address();

            for next_task in downstream_tasks {
                // 3. 将下游任务的入度减 1
                if let Some(degree) = self.indegree.get_mut(&next_task) {
                    *degree -= 1;

                    // 4. 如果入度降为 0，说明它的所有前置任务都已完成，立即启动它！
                    if *degree == 0 {
                        info!("🔓 任务 [{}] 的前置依赖已全部满足，准备触发...", next_task);
                        let duration = self.tasks.get(&next_task).unwrap().payload_duration;
                        self.spawn_task(next_task, duration, addr.clone());
                    }
                }
            }
        }

        // 5. 检查是否整体结束
        self.check_workflow_completed();
    }
}
