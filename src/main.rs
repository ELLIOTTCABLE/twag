use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
   if dotenvy::from_filename(".env.local").is_err() {
      dotenvy::dotenv().ok();
   }

   let db_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");
   println!("Connecting to database at: {}", db_url);

   let app = Router::new().route("/", get(|| async { "Hello, World!" }));

   let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
   axum::serve(listener, app).await.unwrap();
}
