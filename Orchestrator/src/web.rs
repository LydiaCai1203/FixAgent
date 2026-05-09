use crate::error::OrchestratorError;
use crate::service::OrchestratorService;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct ListPrsQuery {
    pub project_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ListIssuesQuery {
    pub project_key: Option<String>,
    pub platform: Option<String>,
    pub pr_number: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PrStatsQuery {
    pub project_key: String,
    pub platform: String,
    pub pr_number: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListWorkflowsQuery {
    pub project_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunUntilStableRequest {
    pub repo_dir: Option<String>,
    pub project_key: String,
    pub project_name: String,
    pub pr_url: String,
    pub claimed_by: Option<String>,
    pub max_rounds: Option<i32>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub project_name: String,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: String,
}

pub async fn serve_http(service: OrchestratorService, host: String, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/projects", get(list_projects).post(create_project))
        .route("/prs", get(list_prs))
        .route("/issues", get(list_issues))
        .route("/pr-stats", get(pr_stats))
        .route("/workflows", get(list_workflows).post(start_workflow))
        .route("/workflows/run-until-stable", post(run_until_stable))
        .route("/workflows/{workflow_run_id}", get(workflow_detail))
        .route("/workflows/{workflow_run_id}/rounds", get(workflow_rounds))
        .with_state(service);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn list_projects(
    State(service): State<OrchestratorService>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.list_projects().await?;
    Ok(Json(serde_json::json!(result)))
}

async fn create_project(
    State(service): State<OrchestratorService>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.create_project(request.project_name).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn list_prs(
    State(service): State<OrchestratorService>,
    Query(query): Query<ListPrsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.list_prs(query.project_key).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn list_issues(
    State(service): State<OrchestratorService>,
    Query(query): Query<ListIssuesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service
        .list_issues(query.project_key, query.platform, query.pr_number, query.status)
        .await?;
    Ok(Json(serde_json::json!(result)))
}

async fn pr_stats(
    State(service): State<OrchestratorService>,
    Query(query): Query<PrStatsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service
        .pr_stats(query.project_key, query.platform, query.pr_number)
        .await?;
    Ok(Json(serde_json::json!(result)))
}

async fn list_workflows(
    State(service): State<OrchestratorService>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.list_workflows(query.project_key).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn workflow_detail(
    State(service): State<OrchestratorService>,
    Path(workflow_run_id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.workflow_detail(workflow_run_id).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn workflow_rounds(
    State(service): State<OrchestratorService>,
    Path(workflow_run_id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service.workflow_rounds(workflow_run_id).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn start_workflow(
    State(service): State<OrchestratorService>,
    Json(request): Json<RunUntilStableRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo_dir = PathBuf::from(request.repo_dir.unwrap_or_else(|| ".".to_string()));
    let project_key = request.project_key;
    let project_name = request.project_name;
    let pr_url = request.pr_url;
    let claimed_by = request.claimed_by.unwrap_or_else(|| "orchestrator-api".to_string());
    let max_rounds = request.max_rounds.unwrap_or(5);
    let dry_run = request.dry_run.unwrap_or(false);

    let workflow_run_id = service
        .start_workflow(
            project_key.clone(),
            project_name.clone(),
            pr_url.clone(),
            max_rounds,
        )
        .await?;

    let background_service = service.clone();
    tokio::spawn(async move {
        if let Err(error) = background_service
            .execute_workflow_run(
                workflow_run_id,
                repo_dir,
                project_key,
                project_name,
                pr_url,
                claimed_by,
                max_rounds,
                dry_run,
            )
            .await
        {
            let _ = background_service
                .mark_workflow_failed(workflow_run_id, &error.to_string())
                .await;
        }
    });

    Ok(Json(serde_json::json!({ "workflow_run_id": workflow_run_id })))
}

async fn run_until_stable(
    State(service): State<OrchestratorService>,
    Json(request): Json<RunUntilStableRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = service
        .run_until_stable(
            PathBuf::from(request.repo_dir.unwrap_or_else(|| ".".to_string())),
            request.project_key,
            request.project_name,
            request.pr_url,
            request.claimed_by.unwrap_or_else(|| "orchestrator-api".to_string()),
            request.max_rounds.unwrap_or(5),
            request.dry_run.unwrap_or(false),
        )
        .await?;

    Ok(Json(serde_json::json!(result)))
}

struct ApiError(OrchestratorError);

impl From<OrchestratorError> for ApiError {
    fn from(value: OrchestratorError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            OrchestratorError::Config(_) => StatusCode::BAD_REQUEST,
            OrchestratorError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OrchestratorError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OrchestratorError::Json(_) => StatusCode::BAD_REQUEST,
        };

        let body = Json(ApiErrorBody {
            error: self.0.to_string(),
        });

        (status, body).into_response()
    }
}
