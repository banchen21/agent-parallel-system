use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::{
    core::errors::AppError,
    models::task::{Task, TaskStatus},
};

/// DAG（有向无环图）编排器
pub struct DagOrchestrator {
    // 任务ID到任务的映射
    tasks: HashMap<Uuid, Task>,
    // 任务依赖关系：task_id -> 依赖的任务ID集合
    dependencies: HashMap<Uuid, HashSet<Uuid>>,
    // 反向依赖关系：task_id -> 依赖于此任务的任务ID集合
    reverse_dependencies: HashMap<Uuid, HashSet<Uuid>>,
}

impl DagOrchestrator {
    /// 创建新的DAG编排器
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            dependencies: HashMap::new(),
            reverse_dependencies: HashMap::new(),
        }
    }

    /// 添加任务到DAG
    pub fn add_task(&mut self, task: Task) {
        self.tasks.insert(task.id, task);
    }

    /// 添加任务依赖关系
    pub fn add_dependency(
        &mut self,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    ) -> Result<(), AppError> {
        // 检查任务是否存在
        if !self.tasks.contains_key(&task_id) {
            return Err(AppError::NotFound(format!("任务 {} 不存在", task_id)));
        }
        if !self.tasks.contains_key(&depends_on_task_id) {
            return Err(AppError::NotFound(format!(
                "依赖任务 {} 不存在",
                depends_on_task_id
            )));
        }

        // 检查是否会产生循环依赖
        if self.would_create_cycle(task_id, depends_on_task_id) {
            return Err(AppError::ValidationError(format!(
                "添加依赖 {} -> {} 会产生循环依赖",
                task_id, depends_on_task_id
            )));
        }

        // 添加正向依赖
        self.dependencies
            .entry(task_id)
            .or_insert_with(HashSet::new)
            .insert(depends_on_task_id);

        // 添加反向依赖
        self.reverse_dependencies
            .entry(depends_on_task_id)
            .or_insert_with(HashSet::new)
            .insert(task_id);

        Ok(())
    }

    /// 检查添加依赖是否会产生循环
    fn would_create_cycle(&self, start_task_id: Uuid, new_dependency_id: Uuid) -> bool {
        // 使用BFS检查从新依赖任务是否能到达起始任务
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(new_dependency_id);
        visited.insert(new_dependency_id);

        while let Some(current) = queue.pop_front() {
            if current == start_task_id {
                return true; // 发现循环
            }

            if let Some(deps) = self.dependencies.get(&current) {
                for &next in deps {
                    if !visited.contains(&next) {
                        visited.insert(next);
                        queue.push_back(next);
                    }
                }
            }
        }

        false
    }

    /// 获取任务的所有依赖
    pub fn get_task_dependencies(&self, task_id: Uuid) -> Vec<Uuid> {
        self.dependencies
            .get(&task_id)
            .map(|deps| deps.iter().copied().collect())
            .unwrap_or_default()
    }

    /// 获取任务的所有反向依赖（哪些任务依赖于此任务）
    pub fn get_task_reverse_dependencies(&self, task_id: Uuid) -> Vec<Uuid> {
        self.reverse_dependencies
            .get(&task_id)
            .map(|deps| deps.iter().copied().collect())
            .unwrap_or_default()
    }

    /// 检查任务是否就绪（所有依赖都已完成）
    pub fn is_task_ready(&self, task_id: Uuid) -> Result<bool, AppError> {
        let _task = self
            .tasks
            .get(&task_id)
            .ok_or_else(|| AppError::NotFound(format!("任务 {} 不存在", task_id)))?;

        // 如果没有依赖，任务就是就绪的
        let deps = self.dependencies.get(&task_id);
        if deps.is_none() || deps.unwrap().is_empty() {
            return Ok(true);
        }

        // 检查所有依赖任务是否都已完成
        for &dep_task_id in deps.unwrap() {
            let dep_task = self
                .tasks
                .get(&dep_task_id)
                .ok_or_else(|| AppError::NotFound(format!("依赖任务 {} 不存在", dep_task_id)))?;

            if dep_task.status != "completed" {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// 获取就绪的任务列表（所有依赖都已完成的任务）
    pub fn get_ready_tasks(&self) -> Vec<Uuid> {
        let mut ready_tasks = Vec::new();

        for (&task_id, _) in &self.tasks {
            if let Ok(true) = self.is_task_ready(task_id) {
                ready_tasks.push(task_id);
            }
        }

        ready_tasks
    }

    /// 获取任务的执行顺序（拓扑排序）
    pub fn get_execution_order(&self) -> Result<Vec<Uuid>, AppError> {
        let mut in_degree = HashMap::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::new();

        // 计算每个任务的入度（依赖数量）
        for (&task_id, _) in &self.tasks {
            let degree = self
                .dependencies
                .get(&task_id)
                .map(|deps| deps.len())
                .unwrap_or(0);
            in_degree.insert(task_id, degree);

            if degree == 0 {
                queue.push_back(task_id);
            }
        }

        // 执行拓扑排序
        while let Some(task_id) = queue.pop_front() {
            order.push(task_id);

            // 减少所有依赖于此任务的任务的入度
            if let Some(reverse_deps) = self.reverse_dependencies.get(&task_id) {
                for &dependent_id in reverse_deps {
                    if let Some(degree) = in_degree.get_mut(&dependent_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent_id);
                        }
                    }
                }
            }
        }

        // 检查是否有循环依赖
        if order.len() != self.tasks.len() {
            return Err(AppError::ValidationError(
                "DAG中存在循环依赖，无法确定执行顺序".to_string(),
            ));
        }

        Ok(order)
    }

    /// 获取任务执行路径（关键路径分析）
    pub fn get_critical_path(&self) -> Result<Vec<Uuid>, AppError> {
        // 简单的关键路径算法（假设所有任务执行时间相同）
        let execution_order = self.get_execution_order()?;

        if execution_order.is_empty() {
            return Ok(Vec::new());
        }

        // 计算每个任务的最早开始时间
        let mut earliest_start = HashMap::new();
        for &task_id in &execution_order {
            let mut max_earliest = 0;

            if let Some(deps) = self.dependencies.get(&task_id) {
                for &dep_id in deps {
                    let dep_earliest = *earliest_start.get(&dep_id).unwrap_or(&0);
                    max_earliest = max_earliest.max(dep_earliest + 1); // 假设每个任务执行时间为1
                }
            }

            earliest_start.insert(task_id, max_earliest);
        }

        // 找到最晚完成的任务
        let max_time = *earliest_start.values().max().unwrap_or(&0);
        let mut critical_path = Vec::new();

        // 找出在关键路径上的任务
        for (&task_id, &start_time) in &earliest_start {
            // 简单的启发式：开始时间 + 1（执行时间） == 总时间
            if start_time + 1 == max_time {
                critical_path.push(task_id);
            }
        }

        Ok(critical_path)
    }

    /// 更新任务状态
    pub fn update_task_status(
        &mut self,
        task_id: Uuid,
        status: TaskStatus,
    ) -> Result<(), AppError> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::NotFound(format!("任务 {} 不存在", task_id)))?;

        task.status = status.to_string();
        Ok(())
    }

    /// 获取任务信息
    pub fn get_task(&self, task_id: Uuid) -> Option<&Task> {
        self.tasks.get(&task_id)
    }

    /// 获取所有任务
    pub fn get_all_tasks(&self) -> Vec<&Task> {
        self.tasks.values().collect()
    }

    /// 清除所有数据
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.dependencies.clear();
        self.reverse_dependencies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_task(id: Uuid, status: TaskStatus) -> Task {
        Task {
            id,
            title: "Test Task".to_string(),
            description: Some("Test Description".to_string()),
            status: status.to_string(),
            priority: crate::models::task::TaskPriority::Medium.to_string(),
            parent_task_id: None,
            workspace_id: Uuid::new_v4(),
            assigned_agent_id: None,
            created_by: Uuid::new_v4(),
            requirements: serde_json::json!({}),
            result: None,
            progress: 0,
            started_at: None,
            completed_at: None,
            retry_count: Some(0),
            metadata: serde_json::json!({}),
            execution_context: serde_json::json!({}),
            tags: serde_json::json!([]),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }

    #[test]
    fn test_add_task_and_dependency() {
        let mut dag = DagOrchestrator::new();

        let task1_id = Uuid::new_v4();
        let task2_id = Uuid::new_v4();

        let task1 = create_test_task(task1_id, TaskStatus::Pending);
        let task2 = create_test_task(task2_id, TaskStatus::Pending);

        dag.add_task(task1);
        dag.add_task(task2);

        assert!(dag.add_dependency(task2_id, task1_id).is_ok());
        assert!(dag.is_task_ready(task1_id).unwrap());
        assert!(!dag.is_task_ready(task2_id).unwrap());
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = DagOrchestrator::new();

        let task1_id = Uuid::new_v4();
        let task2_id = Uuid::new_v4();
        let task3_id = Uuid::new_v4();

        let task1 = create_test_task(task1_id, TaskStatus::Pending);
        let task2 = create_test_task(task2_id, TaskStatus::Pending);
        let task3 = create_test_task(task3_id, TaskStatus::Pending);

        dag.add_task(task1);
        dag.add_task(task2);
        dag.add_task(task3);

        // 添加依赖：2 -> 1, 3 -> 2
        assert!(dag.add_dependency(task2_id, task1_id).is_ok());
        assert!(dag.add_dependency(task3_id, task2_id).is_ok());

        // 尝试添加循环依赖：1 -> 3
        assert!(dag.add_dependency(task1_id, task3_id).is_err());
    }

    #[test]
    fn test_topological_sort() {
        let mut dag = DagOrchestrator::new();

        let task1_id = Uuid::new_v4();
        let task2_id = Uuid::new_v4();
        let task3_id = Uuid::new_v4();

        let task1 = create_test_task(task1_id, TaskStatus::Pending);
        let task2 = create_test_task(task2_id, TaskStatus::Pending);
        let task3 = create_test_task(task3_id, TaskStatus::Pending);

        dag.add_task(task1);
        dag.add_task(task2);
        dag.add_task(task3);

        // 添加依赖：2 -> 1, 3 -> 2
        assert!(dag.add_dependency(task2_id, task1_id).is_ok());
        assert!(dag.add_dependency(task3_id, task2_id).is_ok());

        let order = dag.get_execution_order().unwrap();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], task1_id);
        assert_eq!(order[1], task2_id);
        assert_eq!(order[2], task3_id);
    }

    #[test]
    fn test_ready_tasks() {
        let mut dag = DagOrchestrator::new();

        let task1_id = Uuid::new_v4();
        let task2_id = Uuid::new_v4();

        let task1 = create_test_task(task1_id, TaskStatus::Pending);
        let task2 = create_test_task(task2_id, TaskStatus::Pending);

        dag.add_task(task1);
        dag.add_task(task2);

        // 添加依赖：2 -> 1
        assert!(dag.add_dependency(task2_id, task1_id).is_ok());

        let ready_tasks = dag.get_ready_tasks();
        assert_eq!(ready_tasks.len(), 1);
        assert_eq!(ready_tasks[0], task1_id);

        // 完成任务1
        dag.update_task_status(task1_id, TaskStatus::Completed)
            .unwrap();

        let ready_tasks = dag.get_ready_tasks();
        assert_eq!(ready_tasks.len(), 1);
        assert_eq!(ready_tasks[0], task2_id);
    }
}
