use actix::Addr;
use actix_web::{HttpResponse, Responder, get, web};

use crate::core::actor_system::{GetStats, SysMonitorActor};

// 定义 Handler
#[get("/system_info")]
async fn get_stats_handler(monitor: web::Data<Addr<SysMonitorActor>>) -> impl Responder {
    // 发送消息给 Actor
    match monitor.send(GetStats).await {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
