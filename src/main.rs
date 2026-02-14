use axum::{
    routing::post,
    Router,
    extract::State,
    Json,
    http::StatusCode,
};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use std::{env, net::SocketAddr};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RegisterInput {
    email: String,
    password: String,
    fullname: String,
    discord: String,
    age: i32,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL not set");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("DB connection failed");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/register", post(register))
        .with_state(pool)
        .layer(cors);

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .expect("PORT is not a valid number");

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("Server running on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn register(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterInput>,
) -> (StatusCode, Json<ApiResponse>) {
    // Direktang ini-store ang password nang walang hashing
    let result = sqlx::query(
        "INSERT INTO users (email, password, fullname, discord, age, is_verified)
         VALUES ($1, $2, $3, $4, $5, false)"
    )
    .bind(&payload.email)
    .bind(&payload.password)          // plain text password
    .bind(&payload.fullname)
    .bind(&payload.discord)
    .bind(payload.age)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: "Registration successful".into(),
            }),
        ),
        Err(e) => {
            eprintln!("Registration failed: {:?}", e);

            let message = if e.to_string().contains("duplicate key") || e.to_string().contains("unique constraint") {
                "Email already exists".to_string()
            } else {
                "Database error during registration".to_string()
            };

            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse {
                    success: false,
                    message,
                }),
            )
        }
    }
}