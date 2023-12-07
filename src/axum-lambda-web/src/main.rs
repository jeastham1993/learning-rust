mod application;

use std::env;

use crate::application::adapters::DynamoDbToDoRepo;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_sdk_dynamodb::Client;
use axum::{extract::Path, extract::State, response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use axum::response::IntoResponse;
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::application::public_types::{CreateToDoCommand, ToDoItem, UpdateToDoCommand};
use crate::application::commands::{create_to_do, update_todo};
use crate::application::domain::AppState;
use crate::application::queries::{list_todos, get_todos};

#[derive(Serialize, Deserialize)]
struct ApiResponse<T> {
    data: T,
    message: String,
}

fn app(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/todo", get(list_todo_endpoint).post(post_todo_endpoint))
        .route("/todo/:id", get(get_todo_endpoint).put(update_todo_endpoint))
        .with_state(app_state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "axum_lambda=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let use_local = &env::var("USE_LOCAL");

    let config: SdkConfig = aws_config::load_defaults(BehaviorVersion::latest()).await;

    let mut dynamodb_client: Client = Client::new(&config);

    let mut table_name = env::var("TABLE_NAME").expect("TABLE_NAME must be set");

    if use_local.is_ok() {
        let dynamodb_local_config = aws_sdk_dynamodb::config::Builder::from(&config)
            .endpoint_url(
                // 8000 is the default dynamodb port
                "http://localhost:8000",
            )
            .region(Region::from_static("us-east-1"))
            .build();
        dynamodb_client = Client::from_conf(dynamodb_local_config);
        table_name = String::from("TODO");
    }

    let shared_state = Arc::new(AppState {
        todo_repo: Arc::new(DynamoDbToDoRepo::new(
            dynamodb_client.clone(),
            table_name.clone(),
        )),
    });

    let app = app(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Value> {
    Json(json!({ "msg": "Healthy" }))
}

async fn list_todo_endpoint(headers: HeaderMap, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match check_user_header(headers) {
        Ok(user_id) => {
            let items = list_todos(&user_id, &state.todo_repo).await.unwrap();

            let response = ApiResponse {
                data: items,
                message: "Success".to_string(),
            };

            (StatusCode::OK, Json(response))
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(ApiResponse {
                data: Vec::new(),
                message: "Please set the 'user-id".to_string()
            }))
        }
    }
}

async fn get_todo_endpoint(Path(id): Path<String>, headers: HeaderMap, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match check_user_header(headers) {
        Ok(user_id) => {
            let todo = get_todos(&user_id, id.as_str(), &state.todo_repo).await.unwrap();

            let response = ApiResponse {
                data: todo,
                message: "Success".to_string(),
            };

            (StatusCode::OK, Json(response))
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(ApiResponse {
                data: ToDoItem{
                    id: String::from(""),
                    title: String::from(""),
                    is_complete: false,
                    completed_on: String::from("")
                },
                message: "Please set the 'user-id".to_string()
            }))
        }
    }
}

async fn post_todo_endpoint(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<CreateToDoCommand>,
)  -> impl IntoResponse {
    match check_user_header(headers) {
        Ok(user_id) => {
            let todo = create_to_do(user_id, input, &state.todo_repo).await.unwrap();

            let response = ApiResponse {
                data: todo,
                message: "Success".to_string(),
            };

            (StatusCode::OK, Json(response))
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(ApiResponse {
                data: ToDoItem{
                    id: String::from(""),
                    title: String::from(""),
                    is_complete: false,
                    completed_on: String::from("")
                },
                message: "Please set the 'user-id".to_string()
            }))
        }
    }
}

async fn update_todo_endpoint(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<UpdateToDoCommand>) -> impl IntoResponse
{
    match check_user_header(headers) {
        Ok(user_id) => {
            let todo = update_todo(user_id, id, input, &state.todo_repo)
                .await.unwrap();

            let response = ApiResponse {
                data: todo,
                message: "Success".to_string(),
            };

            (StatusCode::OK, Json(response))
        },
        Err(_) => {
            (StatusCode::BAD_REQUEST, Json(ApiResponse {
                data: ToDoItem{
                    id: String::from(""),
                    title: String::from(""),
                    is_complete: false,
                    completed_on: String::from("")
                },
                message: "Please set the 'user-id".to_string()
            }))
        }
    }
}

fn check_user_header(headers: HeaderMap) -> Result<String, ()> {
    if let Some(user_id) = headers.get("user-id") {
        return Ok(user_id.to_str().unwrap().to_string());
    } else {
        return Err(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use axum::response::Response;
    use http_body_util::BodyExt;
    use http::Method;
    use tower::ServiceExt;


    struct ApiDriver{
        router: Box<Router>
    }

    impl ApiDriver {
        fn new(router: Box<Router>) -> Self {
            Self {
                router
            }
        }

        async fn list(&self) -> Response {
            self.router.clone()
                .oneshot(
                    Request::builder()
                        .uri("/todo")
                        .header("user-id","jameseastham")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn create(&self, text: &str) -> Response {
            let body = format!("{{\"title\":\"{0}\"}}", text);

            self.router.clone()
                .oneshot(
                    Request::builder()
                        .uri("/todo")
                        .method(Method::POST)
                        .header("user-id","jameseastham")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn update(&self, text: &str, todo_id: &str, set_as_complete: &bool) -> Response {
            let body = format!("{{\"title\":\"{0}\", \"to_do_id\":\"{1}\", \"set_as_complete\":{2}}}", text, todo_id, set_as_complete);

            self.router.clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/todo/{0}", todo_id))
                        .method(Method::PUT)
                        .header("user-id","jameseastham")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap()
        }

        async fn get(&self, id: &String) -> Response {
            self.router.clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/todo/{0}", id))
                        .method(Method::GET)
                        .header("user-id","jameseastham")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
        }
    }

    async fn load_test_state() -> Arc<AppState> {
        let config: SdkConfig = aws_config::load_defaults(BehaviorVersion::latest()).await;

        let dynamodb_local_config = aws_sdk_dynamodb::config::Builder::from(&config)
            .endpoint_url(
                // 8000 is the default dynamodb port
                "http://localhost:8000",
            )
            .region(Region::from_static("us-east-1"))
            .build();
        let dynamodb_client = Client::from_conf(dynamodb_local_config);
        let table_name = String::from("TODO");

        Arc::new(AppState {
            todo_repo: Arc::new(DynamoDbToDoRepo::new(
                dynamodb_client.clone(),
                table_name.clone(),
            )),
        })
    }

    #[tokio::test]
    async fn list_todo() {
        let shared_state = load_test_state().await;

        let app = app(shared_state);

        let driver = ApiDriver::new(Box::new(app));

        let response = driver.list().await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn create_and_retrieve_todo() {
        let shared_state = load_test_state().await;

        let app = app(shared_state);

        let driver = ApiDriver::new(Box::new(app));

        let test_text = "My todo";

        let response = driver.create(&test_text).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());
        let created_todo: ApiResponse<ToDoItem> = serde_json::from_slice(&*body.to_vec()).unwrap();

        assert_eq!(&created_todo.data.title, test_text);

        let get_response = driver.get(&created_todo.data.id).await;

        assert_eq!(get_response.status(), StatusCode::OK);
        let get_body = get_response.into_body().collect().await.unwrap().to_bytes();
        assert!(!get_body.is_empty());
    }

    #[tokio::test]
    async fn update_a_completed_todo_title_should_not_change() {
        let shared_state = load_test_state().await;

        let app = app(shared_state);

        let driver = ApiDriver::new(Box::new(app));

        let test_text = "My todo";

        let response = driver.create(&test_text).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());

        let created_todo: ApiResponse<ToDoItem> = serde_json::from_slice(&*body.to_vec()).unwrap();

        let update_response = driver.update("Updated todo", &created_todo.data.id, &true).await;

        let get_response = driver.get(&created_todo.data.id).await;

        assert_eq!(get_response.status(), StatusCode::OK);
        let get_body = get_response.into_body().collect().await.unwrap().to_bytes();
        assert!(!get_body.is_empty());

        let get_todo: ApiResponse<ToDoItem> = serde_json::from_slice(&*get_body.to_vec()).unwrap();

        assert_eq!(&get_todo.data.title, "My todo");
    }

    #[tokio::test]
    async fn update_a_incomplete_todo_title_should_change() {
        let shared_state = load_test_state().await;

        let app = app(shared_state);

        let driver = ApiDriver::new(Box::new(app));

        let test_text = "My todo";

        let response = driver.create(&test_text).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());

        let created_todo: ApiResponse<ToDoItem> = serde_json::from_slice(&*body.to_vec()).unwrap();

        let update_response = driver.update("Updated todo", &created_todo.data.id, &false).await;
        assert_eq!(update_response.status(), StatusCode::OK);

        let get_response = driver.get(&created_todo.data.id).await;

        assert_eq!(get_response.status(), StatusCode::OK);
        let get_body = get_response.into_body().collect().await.unwrap().to_bytes();
        assert!(!get_body.is_empty());

        let get_todo: ApiResponse<ToDoItem> = serde_json::from_slice(&*get_body.to_vec()).unwrap();

        assert_eq!(&get_todo.data.title, "Updated todo");
    }

    #[tokio::test]
    async fn not_found() {
        let shared_state = load_test_state().await;

        let app = app(shared_state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
    }
}
