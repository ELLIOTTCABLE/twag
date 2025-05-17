use axum::{Router, routing::get};
use sqlx::{Error, PgPool, Pool, Postgres};
use tracing::{Level, info, trace};

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

   let pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let app = Router::new().route("/", get(|| async { "Hello, World!" }));

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   println!("Listening on http://{}", listener.local_addr().unwrap());
   axum::serve(listener, app).await.unwrap();
}
