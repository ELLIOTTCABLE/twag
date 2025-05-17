use axum::{Router, routing::get};
use sqlx::PgPool;

#[tokio::main]
async fn main() {
   tracing_subscriber::fmt::init();

   if dotenvy::from_filename(".env.local").is_err() {
      dotenvy::dotenv().ok();
   }

   let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");

   println!("Connecting to database at: {}", database_url);
   let pool = PgPool::connect(&database_url)
      .await
      .expect("Failed to connect to database");

   let row: (i32,) = sqlx::query_as("SELECT 1")
      .fetch_one(&pool)
      .await
      .expect("Failed to execute query");

   println!("Query result: {}", row.0);

   let app = Router::new().route("/", get(|| async { "Hello, World!" }));

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   axum::serve(listener, app).await.unwrap();
}
