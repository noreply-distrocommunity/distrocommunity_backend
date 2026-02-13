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
use rand::{thread_rng, Rng, distributions::Alphanumeric};
use lettre::{
    Message,
    Transport,
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    SmtpTransport,
};

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

    axum::serve(listener, app)
        .await?;

    Ok(())
}

async fn register(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterInput>,
) -> (StatusCode, Json<ApiResponse>) {
    let hashed = format!("hashed_{}", payload.password); // ← use proper hashing in production!

    let code: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();

    let result = sqlx::query(
        "INSERT INTO users (email, password, fullname, discord, age, verification_code, is_verified)
         VALUES ($1, $2, $3, $4, $5, $6, false)"
    )
    .bind(&payload.email)
    .bind(&hashed)
    .bind(&payload.fullname)
    .bind(&payload.discord)
    .bind(payload.age)
    .bind(&code)
    .execute(&pool)
    .await;

    if let Err(e) = result {
        eprintln!("Insert failed: {e}");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "Email already exists or database error".into(),
            }),
        );
    }

    // SMTP setup
    let smtp_email = env::var("SMTP_EMAIL").expect("SMTP_EMAIL not set");
    let smtp_pass  = env::var("SMTP_PASS").expect("SMTP_PASS not set");

    let email = Message::builder()
        .from(smtp_email.parse().expect("Invalid SMTP_EMAIL"))
        .to(payload.email.parse().expect("Invalid email"))
        .subject("Dutchville Account Verification")
        .header(ContentType::TEXT_HTML)
        .body(format!(
            "<h1>Welcome {}, your verification code is: <b>{}</b></h1>",
            payload.fullname, code
        ))
        .expect("Failed to build email");

    let creds = Credentials::new(smtp_email, smtp_pass);

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .expect("Invalid relay")
        .credentials(creds)
        .build();

    if let Err(e) = mailer.send(&email) {
        eprintln!("Email send failed: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: "Failed to send verification email".into(),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Check your email for verification code".into(),
        }),
    )
}