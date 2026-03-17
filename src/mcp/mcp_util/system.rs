use serde_json::{json, Value};
use sysinfo::System;

use crate::mcp::mcp_util::common::{arg_bool, arg_u64};
use crate::mcp::model::McpError;

pub async fn execute_system_info(arguments: &Value) -> Result<Value, McpError> {
    let include_processes = arg_bool(arguments, "include_processes", false);
    let process_limit = arg_u64(arguments, "process_limit").unwrap_or(10).clamp(1, 100) as usize;

    let mut sys = System::new_all();
    sys.refresh_all();

    let mut processes = Vec::new();
    if include_processes {
        let mut sorted: Vec<_> = sys.processes().iter().collect();
        sorted.sort_by(|a, b| {
            b.1.cpu_usage()
                .partial_cmp(&a.1.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (pid, proc_) in sorted.into_iter().take(process_limit) {
            processes.push(json!({
                "pid": pid.to_string(),
                "name": proc_.name(),
                "cpu": proc_.cpu_usage(),
                "memory_bytes": proc_.memory()
            }));
        }
    }

    Ok(json!({
        "os_name": System::name(),
        "kernel_version": System::kernel_version(),
        "os_version": System::os_version(),
        "host_name": System::host_name(),
        "cpu_core_count": sys.cpus().len(),
        "memory_total_bytes": sys.total_memory(),
        "memory_used_bytes": sys.used_memory(),
        "swap_total_bytes": sys.total_swap(),
        "swap_used_bytes": sys.used_swap(),
        "uptime_secs": System::uptime(),
        "processes": processes
    }))
}
