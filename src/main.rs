use axum::{Router, extract::Path, http::StatusCode, routing::get};
use chrono::{DateTime, Utc};
use lazy_regex::regex_captures;
use sqlx::{Error, PgPool, Pool, Postgres};
use tracing::{info, trace, warn};

#[derive(sqlx::FromRow)]
struct TwagTag {
   id: String,
   target_url: String,
   created_at: DateTime<Utc>,
   updated_at: DateTime<Utc>,
   last_accessed: Option<DateTime<Utc>>,
   access_count: i32,
   last_seen_tap_count: Option<i32>,
}

async fn initialize_connection(database_url: &str) -> Result<Pool<Postgres>, Error> {
   info!(database_url, "Connecting to database");
   let pool = PgPool::connect(database_url).await?;

   sqlx::query("SELECT 1").fetch_one(&pool).await?;

   trace!("Connection established");
   Ok(pool)
}

#[tokio::main]
async fn main() {
   tracing_subscriber::fmt()
      .event_format(tracing_subscriber::fmt::format().with_file(true).with_line_number(true))
      .init();

   if dotenvy::from_filename(".env.local").is_err() {
      dotenvy::dotenv().ok();
   }

   let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");

   let _pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let app = Router::new()
      .route("/", get(|| async { "Hello, World!" }))
      // https://xz.ws/tag/055B88A23C1250
      // https://xz.ws/tag/055B88A23C1250x00000F
      .route("/tag/{slug}", get(get_tag_by_id));

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   println!("Listening on http://{}", listener.local_addr().unwrap());
   axum::serve(listener, app).await.unwrap();
}

#[axum::debug_handler]
async fn get_tag_by_id(Path(param): Path<String>) -> Result<String, StatusCode> {
   if let Some((_, id, tap_count_str)) = regex_captures!(r"^([0-9A-F]{14})(?:x([0-9A-F]{6}))?$", &param) {
      let tap_count = if tap_count_str.is_empty() {
         None
      } else {
         Some(i32::from_str_radix(tap_count_str, 16).unwrap_or(0))
      };
      return Ok("Tag ID: ".to_string() + &id + " Tap Count: " + &tap_count.unwrap_or(0).to_string());
   } else {
      warn!("Invalid tag ID format");
      return Err(StatusCode::BAD_REQUEST);
   }
}
