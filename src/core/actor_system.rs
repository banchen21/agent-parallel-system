use serde::Serialize;
use sysinfo::{System, Networks, Disks};
use actix::prelude::*;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub used_memory: u64,
    pub total_memory: u64,
    pub disk_usage: u64,
    pub total_disk: u64,
    // 实时网络流量（Byte/s）
    pub net_rx_speed: f64,
    pub net_tx_speed: f64,
}

pub struct SysMonitorActor {
    sys: System,
    networks: Networks,
    disks: Disks,
    // 用于计算网速的状态
    last_check: Instant,
    last_received: u64,
    last_transmitted: u64,
}

impl SysMonitorActor {
    pub fn new() -> Self {
        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        
        // 初始化累加值
        let received = networks.iter().map(|(_, n)| n.received()).sum();
        let transmitted = networks.iter().map(|(_, n)| n.transmitted()).sum();
        
        Self {
            sys: System::new_all(),
            networks,
            disks,
            last_check: Instant::now(),
            last_received: received,
            last_transmitted: transmitted,
        }
    }
}

impl Actor for SysMonitorActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "SystemStats")]
pub struct GetStats;

impl Handler<GetStats> for SysMonitorActor {
    type Result = MessageResult<GetStats>;

    fn handle(&mut self, _: GetStats, _: &mut Self::Context) -> Self::Result {
        let now = Instant::now();
        let duration = now.duration_since(self.last_check).as_secs_f64();
        
        // 刷新所有数据
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);

        // 计算当前累计流量
        let current_rx = self.networks.iter().map(|(_, n)| n.received()).sum::<u64>();
        let current_tx = self.networks.iter().map(|(_, n)| n.transmitted()).sum::<u64>();

        // 计算瞬时速度 (Bytes per second)
        // 使用 saturating_sub 防止网卡重启导致计数器归零造成的溢出
        let rx_speed = (current_rx.saturating_sub(self.last_received)) as f64 / duration;
        let tx_speed = (current_tx.saturating_sub(self.last_transmitted)) as f64 / duration;

        // 更新状态以供下次计算
        self.last_check = now;
        self.last_received = current_rx;
        self.last_transmitted = current_tx;

        // 计算磁盘占用
        let total_disk = self.disks.iter().map(|d| d.total_space()).sum();
        let used_disk = self.disks.iter().map(|d| d.total_space() - d.available_space()).sum();

        let stats = SystemStats {
            cpu_usage: self.sys.global_cpu_usage(),
            used_memory: self.sys.used_memory(),
            total_memory: self.sys.total_memory(),
            disk_usage: used_disk,
            total_disk,
            net_rx_speed: rx_speed,
            net_tx_speed: tx_speed,
        };
        
        MessageResult(stats)
    }
}