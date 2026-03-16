use actix::prelude::*;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

/// 发给订阅方（如 WS 会话）的任务通知事件。
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct TaskQueueEvent {
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct QueuedTaskMessage {
    content: String,
    created_at: DateTime<Utc>,
}

pub struct TaskNotifyQueueActor {
    queue: VecDeque<QueuedTaskMessage>,
    subscribers: HashMap<String, Recipient<TaskQueueEvent>>,
}

impl TaskNotifyQueueActor {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            subscribers: HashMap::new(),
        }
    }
}

impl Actor for TaskNotifyQueueActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("TaskNotifyQueueActor 已启动");

        // 定时从队列取消息并广播给在线订阅者。
        ctx.run_interval(std::time::Duration::from_millis(400), |act, _ctx| {
            let Some(msg) = act.queue.pop_front() else {
                return;
            };

            if act.subscribers.is_empty() {
                return;
            }

            let event = TaskQueueEvent {
                content: msg.content,
                created_at: msg.created_at,
            };
            for recipient in act.subscribers.values() {
                recipient.do_send(event.clone());
            }
        });
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EnqueueTaskNotify {
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl Handler<EnqueueTaskNotify> for TaskNotifyQueueActor {
    type Result = ();

    fn handle(&mut self, msg: EnqueueTaskNotify, _ctx: &mut Self::Context) -> Self::Result {
        self.queue.push_back(QueuedTaskMessage {
            content: msg.content,
            created_at: msg.created_at,
        });
        debug!(queue_size = self.queue.len(), "任务通知已入队");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SubscribeTaskNotify {
    pub session_id: String,
    pub recipient: Recipient<TaskQueueEvent>,
}

impl Handler<SubscribeTaskNotify> for TaskNotifyQueueActor {
    type Result = ();

    fn handle(&mut self, msg: SubscribeTaskNotify, _ctx: &mut Self::Context) -> Self::Result {
        self.subscribers.insert(msg.session_id, msg.recipient);
        debug!(subscribers = self.subscribers.len(), "新增任务通知订阅者");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UnsubscribeTaskNotify {
    pub session_id: String,
}

impl Handler<UnsubscribeTaskNotify> for TaskNotifyQueueActor {
    type Result = ();

    fn handle(&mut self, msg: UnsubscribeTaskNotify, _ctx: &mut Self::Context) -> Self::Result {
        self.subscribers.remove(&msg.session_id);
        debug!(subscribers = self.subscribers.len(), "移除任务通知订阅者");
    }
}