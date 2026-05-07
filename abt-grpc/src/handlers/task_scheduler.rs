//! Task Scheduler gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_task_scheduler_service_server::AbtTaskSchedulerService as GrpcTaskSchedulerService,
    *,
};
use crate::handlers::GrpcResult;
use crate::server::AppState;

pub struct TaskSchedulerHandler;

impl TaskSchedulerHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskSchedulerHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl GrpcTaskSchedulerService for TaskSchedulerHandler {
    async fn list_tasks(
        &self,
        _request: Request<Empty>,
    ) -> GrpcResult<ListTasksResponse> {
        let state = AppState::get().await;
        let statuses = state.task_scheduler().list_statuses().await;

        Ok(Response::new(ListTasksResponse {
            tasks: statuses.into_iter().map(status_to_proto).collect(),
        }))
    }

    async fn trigger_task(
        &self,
        request: Request<TriggerTaskRequest>,
    ) -> GrpcResult<TriggerTaskResponse> {
        let req = request.into_inner();
        if req.name.is_empty() {
            return Err(error::validation("name", "任务名称不能为空"));
        }

        let state = AppState::get().await;
        let result = state
            .task_scheduler()
            .trigger(&req.name)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(TriggerTaskResponse {
            processed: result.processed as u64,
            succeeded: result.succeeded as u64,
            message: result.message,
        }))
    }
}

fn status_to_proto(s: abt::TaskStatus) -> TaskStatusProto {
    TaskStatusProto {
        name: s.name,
        is_running: s.is_running,
        last_run_at: s.last_run_at,
        last_elapsed_ms: s.last_elapsed_ms,
        last_result: s.last_result,
        last_error: s.last_error,
        total_runs: s.total_runs,
        interval_secs: s.interval_secs,
    }
}
