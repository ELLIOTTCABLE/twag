use axum::{Router, routing::get};
use sqlx::{Error, PgPool, Pool, Postgres};

async fn initialize_connection(database_url: &str) -> Result<Pool<Postgres>, Error> {
   println!("Connecting to database at: {}", database_url);
   let pool = PgPool::connect(database_url).await?;

   sqlx::query("SELECT 1").fetch_one(&pool).await?;

   Ok(pool)
}

#[tokio::main]
async fn main() {
   tracing_subscriber::fmt::init();

   if dotenvy::from_filename(".env.local").is_err() {
      dotenvy::dotenv().ok();
   }

   let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");

   let pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let app = Router::new().route("/", get(|| async { "Hello, World!" }));

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   axum::serve(listener, app).await.unwrap();
}
