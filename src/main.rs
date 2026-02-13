use axum::{
    routing::post,
    Router,
    Json,
    extract::State,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{env, net::SocketAddr};
use dotenvy::dotenv;
use rand::{thread_rng};
use rand::distributions::Alphanumeric;
use rand::Rng;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use tower_http::cors::{CorsLayer, Any};

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
async fn main() {
    dotenv().ok();

    let database_url =
        env::var("DATABASE_URL")
            .expect("DATABASE_URL not set");

    let pool =
        PgPool::connect(&database_url)
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

    let port =
        env::var("PORT")
            .unwrap_or_else(|_| "10000".to_string());

    let addr =
        SocketAddr::from(([0, 0, 0, 0], port.parse().unwrap()));

    println!("Server running on {}", addr);

    let listener =
        tokio::net::TcpListener::bind(addr)
            .await
            .unwrap();

    axum::serve(listener, app)
        .await
        .unwrap();
}

async fn register(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterInput>,
) -> Json<ApiResponse> {

    // SIMPLE hash muna (temporary para mag compile)
    let hashed = format!("hashed_{}", payload.password);

    // generate 6 letter code
    let code: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();

    // INSERT sa database
    let result = sqlx::query(
        "INSERT INTO users (email, password, fullname, discord, age, verification_code, is_verified)
         VALUES ($1,$2,$3,$4,$5,$6,false)"
    )
    .bind(&payload.email)
    .bind(&hashed)
    .bind(&payload.fullname)
    .bind(&payload.discord)
    .bind(payload.age)
    .bind(&code)
    .execute(&pool)
    .await;

    if result.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Email already exists".into(),
        });
    }

    // kunin SMTP info mula sa .env
    let smtp_email = env::var("SMTP_EMAIL").unwrap();
    let smtp_pass = env::var("SMTP_PASS").unwrap();

    // HTML email body
    let html_body = format!(
        r#"
        <html>
            <body style="font-family: Arial, sans-serif; text-align: center; background-color: #f0f2f5; padding: 30px;">
                <div style="background-color: #ffffff; border-radius: 10px; padding: 40px; display: inline-block;">
                    <h2 style="color: #4A90E2;">Welcome to Dutchville!</h2>
                    <p>Hello, <strong>{}</strong></p>
                    <p>Your verification code is:</p>
                    <h1 style="letter-spacing: 4px; color: #333;">{}</h1>
                    <p>Please enter this code to verify your account.</p>
                    <hr style="margin-top: 30px; margin-bottom: 10px;">
                    <p style="font-size: 12px; color: #888;">This is an automated message, please do not reply.</p>
                </div>
            </body>
        </html>
        "#,
        payload.fullname,
        code
    );

    // gumawa ng email message
    let email = Message::builder()
        .from(smtp_email.parse().unwrap())
        .to(payload.email.parse().unwrap())
        .subject("Dutchville Account Verification")
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(html_body)
        .unwrap();

    // credentials
    let creds = Credentials::new(smtp_email.clone(), smtp_pass);

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    let _ = mailer.send(&email);

    Json(ApiResponse {
        success: true,
        message: "Check your email for verification code".into(),
    })
}
