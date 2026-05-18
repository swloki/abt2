//! Workflow gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_workflow_service_server::AbtWorkflowService as GrpcWorkflowService, *,
};
use crate::handlers::{dt_to_string, empty_to_none, json_to_string, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

use abt::WorkflowService;

pub struct WorkflowHandler;

impl WorkflowHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WorkflowHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn template_to_proto(t: abt::WorkflowTemplate) -> WorkflowTemplateResponse {
    WorkflowTemplateResponse {
        id: t.id,
        entity_type: t.entity_type,
        name: t.name,
        version: t.version,
        status: t.status,
        graph: json_to_string(t.graph),
        graph_checksum: t.graph_checksum.unwrap_or_default(),
        created_at: dt_to_string(t.created_at),
        updated_at: dt_to_string(t.updated_at),
        trigger_event: t.trigger_event.unwrap_or_default(),
    }
}

fn instance_to_proto(i: abt::WorkflowInstance) -> WorkflowInstanceResponse {
    WorkflowInstanceResponse {
        id: i.id,
        template_id: i.template_id,
        template_version: i.template_version.unwrap_or(0),
        entity_type: i.entity_type,
        entity_id: i.entity_id,
        status: i.status,
        frozen_graph: json_to_string(i.frozen_graph),
        context: json_to_string(i.context),
        suspended_reason: json_to_string(i.suspended_reason),
        initiator_id: i.initiator_id,
        created_at: dt_to_string(i.created_at),
        updated_at: dt_to_string(i.updated_at),
        last_advanced_at: dt_to_string(i.last_advanced_at),
        completed_at: dt_to_string(i.completed_at),
    }
}

fn task_to_proto(t: abt::WorkflowTask) -> WorkflowTaskResponse {
    WorkflowTaskResponse {
        id: t.id,
        instance_id: t.instance_id,
        node_id: t.node_id,
        prev_task_id: t.prev_task_id.unwrap_or(0),
        assignee_id: t.assignee_id.unwrap_or(0),
        status: t.status,
        action: t.action.unwrap_or_default(),
        timeout_action: t.timeout_action.unwrap_or_default(),
        due_at: dt_to_string(t.due_at),
        remind_at: dt_to_string(t.remind_at),
        result: json_to_string(t.result),
        created_at: dt_to_string(t.created_at),
        completed_at: dt_to_string(t.completed_at),
        instance_entity_type: String::new(),
        instance_entity_id: 0,
        template_name: String::new(),
    }
}

#[tonic::async_trait]
impl GrpcWorkflowService for WorkflowHandler {
    async fn create_template(
        &self,
        request: Request<CreateWorkflowTemplateRequest>,
    ) -> GrpcResult<U64Response> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let trigger_event = empty_to_none(req.trigger_event);
        let id = srv
            .create_template(&req.entity_type, &req.name, &req.graph, trigger_event.as_deref())
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_template(
        &self,
        request: Request<UpdateWorkflowTemplateRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let name = empty_to_none(req.name);
        let graph = empty_to_none(req.graph);
        let trigger_event = empty_to_none(req.trigger_event);
        srv.update_template(req.id, name.as_deref(), graph.as_deref(), trigger_event.as_deref())
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_template(
        &self,
        request: Request<GetWorkflowTemplateRequest>,
    ) -> GrpcResult<WorkflowTemplateResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let template = srv
            .get_template(req.id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| tonic::Status::not_found("template not found"))?;

        Ok(Response::new(template_to_proto(template)))
    }

    async fn list_templates(
        &self,
        request: Request<ListWorkflowTemplatesRequest>,
    ) -> GrpcResult<WorkflowTemplateListResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let items = srv
            .list_templates(&req.entity_type)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(WorkflowTemplateListResponse {
            items: items.into_iter().map(template_to_proto).collect(),
        }))
    }

    async fn publish_template(
        &self,
        request: Request<PublishTemplateRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        srv.publish_template(req.id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn archive_template(
        &self,
        request: Request<ArchiveTemplateRequest>,
    ) -> GrpcResult<BoolResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        srv.archive_template(req.id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn start_instance(
        &self,
        request: Request<StartWorkflowInstanceRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let id = srv
            .start_instance(&req.entity_type, req.entity_id, auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn cancel_instance(
        &self,
        request: Request<CancelWorkflowInstanceRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        srv.cancel_instance(req.id, auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_instance(
        &self,
        request: Request<GetWorkflowInstanceRequest>,
    ) -> GrpcResult<WorkflowInstanceResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let instance = srv
            .get_instance(req.id)
            .await
            .map_err(error::err_to_status)?
            .ok_or_else(|| tonic::Status::not_found("instance not found"))?;

        Ok(Response::new(instance_to_proto(instance)))
    }

    async fn list_instances(
        &self,
        request: Request<ListWorkflowInstancesRequest>,
    ) -> GrpcResult<WorkflowInstanceListResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let items = srv
            .list_instances(&req.entity_type, req.entity_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(WorkflowInstanceListResponse {
            items: items.into_iter().map(instance_to_proto).collect(),
        }))
    }

    async fn get_my_tasks(
        &self,
        request: Request<GetMyTasksRequest>,
    ) -> GrpcResult<WorkflowTaskListResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let status = empty_to_none(req.status);
        let status = status.as_deref();

        let items = srv
            .get_my_tasks(auth.user_id, status)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(WorkflowTaskListResponse {
            items: items.into_iter().map(task_to_proto).collect(),
        }))
    }

    async fn approve_task(
        &self,
        request: Request<ApproveTaskRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let comment = empty_to_none(req.comment);
        let comment = comment.as_deref();

        srv.approve_task(req.task_id, auth.user_id, comment)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn reject_task(
        &self,
        request: Request<RejectTaskRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let comment = empty_to_none(req.comment);
        let comment = comment.as_deref();

        srv.reject_task(req.task_id, auth.user_id, comment)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delegate_task(
        &self,
        request: Request<DelegateTaskRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let new_task_id = srv
            .delegate_task(req.task_id, auth.user_id, req.to_user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(U64Response {
            value: new_task_id as u64,
        }))
    }

    async fn retry_auto_task(
        &self,
        request: Request<RetryAutoTaskRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        srv.retry_auto_task(req.instance_id, auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn retry_failed_hook(
        &self,
        request: Request<RetryFailedHookRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        srv.retry_failed_hook(req.instance_id, auth.user_id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn record_entity_change(
        &self,
        request: Request<RecordEntityChangeRequest>,
    ) -> GrpcResult<RecordEntityChangeResponse> {
        let _auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let assignee_ids = srv
            .record_entity_change(
                req.instance_id,
                req.entity_id,
                &req.change_type,
                &req.change_detail,
            )
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(RecordEntityChangeResponse { assignee_ids }))
    }

    async fn list_trigger_events(
        &self,
        request: Request<ListTriggerEventsRequest>,
    ) -> GrpcResult<TriggerEventListResponse> {
        let _auth = extract_auth(&request)?;
        let state = AppState::get().await;
        let srv = state.workflow_service();

        let events = srv
            .list_trigger_events()
            .await
            .map_err(error::err_to_status)?;

        let items = events
            .into_iter()
            .map(|evt| TriggerEventDef {
                name: evt.name.to_string(),
                label: evt.label.to_string(),
                description: evt.description.to_string(),
                bound_template_id: evt.bound_template_id,
                bound_template_name: evt.bound_template_name,
            })
            .collect();

        Ok(Response::new(TriggerEventListResponse { items }))
    }
}
