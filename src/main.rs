use axum::{
   Router,
   extract::{Path, State},
   http::StatusCode,
   response::{IntoResponse, Response},
   routing::get,
};
use chrono::{DateTime, Utc};
use lazy_regex::regex_captures;
use sqlx::{Error, Pool, Postgres, postgres::PgPoolOptions};
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_http::{
   LatencyUnit,
   trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info, trace, warn};

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
   let pool = PgPoolOptions::new()
      .min_connections(1)
      .max_connections(5)
      .idle_timeout(std::time::Duration::from_secs(300))
      .connect(database_url)
      .await?;

   sqlx::query("SELECT 1").fetch_one(&pool).await?;

   trace!("Connection established");
   Ok(pool)
}

#[derive(Clone)]
struct AppState {
   pool: sqlx::PgPool,
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

   let pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let app_state = AppState { pool };

   let app = Router::new()
      .route("/", get(|| async { "Hello, World!" }))
      // https://xz.ws/tag/055B88A23C1250
      // https://xz.ws/tag/055B88A23C1250x00000F
      .route("/tag/{slug}", get(get_tag_by_id))
      .with_state(app_state)
      .layer(
         TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_request(DefaultOnRequest::new().level(Level::INFO))
            .on_response(
               DefaultOnResponse::new()
                  .level(Level::INFO)
                  .latency_unit(LatencyUnit::Micros),
            ),
      );

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   println!("Listening on http://{}", listener.local_addr().unwrap());
   axum::serve(listener, app).await.unwrap();
}

async fn get_tag_by_id(State(state): State<AppState>, Path(param): Path<String>) -> Result<Response, StatusCode> {
   let Some((_, id, tap_count_str)) = regex_captures!(r"^([0-9A-F]{14})(?:x([0-9A-F]{6}))?$", &param) else {
      warn!("Invalid tag ID format");
      return Err(StatusCode::BAD_REQUEST);
   };

   let tap_count = (!tap_count_str.is_empty())
      .then_some(tap_count_str)
      .and_then(|s| i32::from_str_radix(s, 16).ok());

   let Ok(mut conn) = state.pool.acquire().await else {
      warn!("Failed to acquire database connection");
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
   };

   let tag = sqlx::query!("SELECT * FROM twag_tags WHERE id = $1", id)
      .fetch_optional(&mut *conn)
      .await
      .map_err(|e| {
         warn!("Failed to fetch tag '{id}' from database: {:?}", e);
         StatusCode::INTERNAL_SERVER_ERROR
      })?;

   if tag.is_none() {
      info!("Tag '{id}' not found");
      return Ok(
         axum::response::Redirect::temporary(&format!("/create/{}x{:06X}", id, tap_count.unwrap_or(0))).into_response(),
      );
   }
   let tag = tag.unwrap();

   trace!("Tag found: {:?}", tag);
   Ok(("Tag ID: ".to_string() + id + " Tap Count: " + &tap_count.unwrap_or(0).to_string()).into_response())
}
