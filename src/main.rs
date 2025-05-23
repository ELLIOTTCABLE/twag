use askama::Template;
use axum::{
   Router, extract,
   http::{StatusCode, header},
   response::{IntoResponse, Response},
   routing::{get, post},
};
use lazy_regex::regex_captures;
use notion_client::endpoints::Client as Notion;
use serde::Deserialize;
use serde_hex::{Compact, SerHexOpt};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use tower_http::{
   LatencyUnit,
   trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info, trace, warn};
use url::{Host, Url};

mod models;
use models::Hex14;

async fn initialize_connection(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
   info!(database_url, "Connecting to database");
   let pool = PgPoolOptions::new()
      .min_connections(1)
      .max_connections(5)
      .idle_timeout(std::time::Duration::from_secs(300))
      .connect(database_url)
      .await?;

   sqlx::query("SELECT 1").fetch_one(&pool).await?;

   info!("Postgres connection established");
   Ok(pool)
}

async fn parse_page_id_from_possible_url(input: &str) -> Result<String, String> {
   let database_id = match Url::parse(input) {
      Ok(url) => {
         // Validate the URL is from Notion
         match url.host() {
            Some(Host::Domain("www.notion.so")) => (),
            _ => {
               return Err(format!(
                  "Invalid Notion database ID '{input}': must be either a bare ID or a Notion URL"
               ));
            }
         }

         // Extract the database ID from the URL path
         url.path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|segment| !segment.is_empty())
            .ok_or_else(|| format!("Invalid Notion URL '{input}': missing database ID in path"))?
            .to_string()
      }
      Err(_) => input.to_string(),
   };

   if database_id.len() != 32 || !database_id.chars().all(|c| c.is_ascii_alphanumeric()) {
      return Err(format!("Invalid Notion database ID '{input}'"));
   }

   Ok(database_id)
}

async fn validate_notion_databases(
   client: &Notion,
   things_db: &str,
   containers_db: &str,
) -> Result<(String, String), String> {
   let things_db = parse_page_id_from_possible_url(things_db).await?;
   let containers_db = parse_page_id_from_possible_url(containers_db).await?;
   trace!(things_db, containers_db, "Parsed database IDs");

   match client.databases.retrieve_a_database(&things_db).await {
      Ok(_schema) => trace!("Successfully connected to things database"),
      Err(err) => {
         trace!(err = %err, "Failed to connect to things database");
         return Err(format!("Failed to validate things database {}: {:?}", things_db, err));
      }
   }

   match client.databases.retrieve_a_database(&containers_db).await {
      Ok(_schema) => trace!("Successfully connected to containers database"),
      Err(err) => {
         return Err(format!(
            "Failed to validate containers database {}: {:?}",
            containers_db, err
         ));
      }
   }
   // trace!("Notion database response: {:?}", response);
   Ok((things_db, containers_db))
}

fn init_tracing() {
   use tracing_subscriber::{EnvFilter, fmt};

   let filter = EnvFilter::builder()
      .with_default_directive(match dotenvy::var("RUST_FMT").as_deref() {
         Ok("json") => Level::INFO.into(),
         Ok("pretty") => Level::DEBUG.into(),
         _ => Level::WARN.into(),
      })
      .parse_lossy(dotenvy::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
      .add_directive("hyper::client=info".parse().unwrap())
      .add_directive("hyper::proto=warn".parse().unwrap());

   let format = fmt::format().with_timer(fmt::time::ChronoUtc::rfc_3339());

   match dotenvy::var("RUST_FMT").as_deref() {
      Ok("json") => fmt()
         .with_env_filter(filter)
         .event_format(format.json().with_target(false).with_source_location(true))
         .init(),
      Ok("pretty") => fmt()
         .with_env_filter(filter)
         .event_format(format.pretty().with_source_location(true))
         .init(),
      _ => fmt().with_env_filter(filter).event_format(format).init(),
   };
}

#[allow(dead_code)]
#[derive(Clone)]
struct AppState {
   pool: sqlx::PgPool,
   client: Notion,
}

#[tokio::main]
async fn main() {
   if dotenvy::from_filename(".env").is_err() {
      dotenvy::dotenv().ok();
   }

   init_tracing();

   // TODO: Use a config library
   let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");
   let notion_token = dotenvy::var("NOTION_TOKEN").expect("NOTION_TOKEN must be set");
   let notion_things_db = dotenvy::var("NOTION_THINGS_DB").expect("NOTION_THINGS_DB must be set");
   let notion_containers_db = dotenvy::var("NOTION_CONTAINERS_DB").expect("NOTION_CONTAINERS_DB must be set");

   let pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let client = Notion::new(notion_token.clone(), None).expect("Failed to create Notion client");

   let (_notion_things_db, _notion_containers_db) =
      validate_notion_databases(&client, &notion_things_db, &notion_containers_db)
         .await
         .unwrap();

   let app_state = AppState { pool, client };
   let app = Router::new()
      .route("/", get(|| async { "Hello, World!" }))
      // GET https://xz.ws/tag/create?id=055B88A23C1250&tap_count=00000F
      .route("/tag/create", get(create_tag_page))
      // POST https://xz.ws/tag/create?id=055B88A23C1250&tap_count=00000F: target_url=https://example.com
      .route("/tag/create", post(create_tag))
      // GET https://xz.ws/tag/055B88A23C1250
      // GET https://xz.ws/tag/055B88A23C1250x00000F
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

fn as_html(mut resp: Response) -> Response {
   resp
      .headers_mut()
      .insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());
   resp
}

#[derive(Deserialize)]
struct TagCreateQuery {
   id: Hex14,
   #[serde(with = "SerHexOpt::<Compact>")]
   #[serde(default)]
   tap_count: Option<u32>,
   target_url: Option<String>,
}

#[derive(Deserialize)]
struct TagCreateForm {
   #[serde(with = "SerHexOpt::<Compact>")]
   #[serde(default)]
   tap_count: Option<u32>,
   target_url: Option<String>,
}

#[derive(Template)]
#[template(path = "tag_create.html")]
struct TagCreateTemplate<'a> {
   id: &'a str,
   tap_count: &'a Option<String>,
}

async fn create_tag_page(
   extract::State(_state): extract::State<AppState>,
   extract::Query(param): extract::Query<TagCreateQuery>,
) -> Result<Response, StatusCode> {
   let id = &param.id;
   let tap_count = param.tap_count;
   let target_url = &param.target_url;

   // TODO: Redirect to edit if exists

   let page = TagCreateTemplate {
      id,
      tap_count: &tap_count.map(|c| format!("{:06X}", c)),
   };
   let response = page.render().map_err(|e| {
      warn!("Failed to render template: {:?}", e);
      StatusCode::INTERNAL_SERVER_ERROR
   })?;
   Ok(as_html(response.into_response()))
}

async fn create_tag(
   extract::State(state): extract::State<AppState>,
   extract::Query(param): extract::Query<TagCreateQuery>,
   extract::Form(form): extract::Form<TagCreateForm>,
) -> Result<Response, StatusCode> {
   let id = &param.id;
   let tap_count = form.tap_count.or(param.tap_count).unwrap_or(1);
   let target_url = &form.target_url.or(param.target_url);

   if target_url.is_none() {
      warn!("Target URL is missing");
      return Err(StatusCode::BAD_REQUEST);
   }
   let target_url = target_url.as_ref().unwrap();

   info!(
      "Creating tag with ID: {id}, tap_count: {tap_count}, target_url: {:?}",
      target_url
   );

   let Ok(mut conn) = state.pool.acquire().await else {
      warn!("Failed to acquire database connection");
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
   };

   sqlx::query!(
      r#"INSERT INTO twag_tags (id, target_url, access_count) VALUES ($1::hex_14, $2, $3)"#,
      id as &Hex14,
      target_url,
      tap_count as i32,
   )
   .execute(&mut *conn)
   .await
   .map_err(|e| {
      warn!("Failed to create tag in database: {:?}", e);
      StatusCode::INTERNAL_SERVER_ERROR
   })?;

   Ok("Created!".into_response())
}

async fn get_tag_by_id(
   extract::State(state): extract::State<AppState>,
   extract::Path(param): extract::Path<String>,
) -> Result<Response, StatusCode> {
   let Some((_, id_str, tap_count_str)) = regex_captures!(r"^([0-9A-F]{14})(?:x([0-9A-F]{6}))?$", &param) else {
      warn!("Invalid tag ID format");
      return Err(StatusCode::BAD_REQUEST);
   };

   let id: Hex14 = id_str.try_into().map_err(|e| {
      warn!("Failed to parse tag ID: {:?}", e);
      StatusCode::BAD_REQUEST
   })?;

   let tap_count = (!tap_count_str.is_empty())
      .then_some(tap_count_str)
      .and_then(|s| i32::from_str_radix(s, 16).ok());

   let Ok(mut conn) = state.pool.acquire().await else {
      warn!("Failed to acquire database connection");
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
   };

   let tag = sqlx::query!("SELECT * FROM twag_tags WHERE id = $1", &id)
      .fetch_optional(&mut *conn)
      .await
      .map_err(|e| {
         warn!("Failed to fetch tag '{id}' from database: {:?}", e);
         StatusCode::INTERNAL_SERVER_ERROR
      })?;

   if tag.is_none() {
      info!("Tag '{id}' not found, redirecting to /tag/create");
      let create_url = tap_count
         .map(|tap_count| format!("/tag/create?id={id}&tap_count={:06X}", tap_count))
         .unwrap_or_else(|| format!("/tag/create?id={id}"));
      return Ok(axum::response::Redirect::temporary(&create_url).into_response());
   }
   let tag = tag.unwrap();

   trace!(tag = ?tag, "Tag found, redirecting to '{}'", tag.target_url);
   Ok(axum::response::Redirect::permanent(&tag.target_url).into_response())
}
