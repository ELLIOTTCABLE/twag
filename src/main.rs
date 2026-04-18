use askama::Template;
use axum::{
   extract,
   http::{header, StatusCode},
   response::{IntoResponse, Response},
   routing::{get, post},
   Router,
};
use lazy_regex::regex_captures;
use notion_client::{
   endpoints::Client as Notion, objects::data_source::DataSource, objects::database::DatabaseProperty,
};
use serde::Deserialize;
use serde_hex::{Compact, SerHexOpt};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tower_http::{
   trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
   LatencyUnit,
};
use tracing::{debug, info, trace, warn, Level};

mod models;
use models::{Hex14, NotionPageId};

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

/// Retrieve the primary data-source for a Notion database.
///
/// Since API version 2025-09-03, database properties live on data-sources
/// rather than on the database object itself. This retrieves the database to
/// find its data-source IDs, then fetches the first data-source.
async fn retrieve_data_source(
   client: &Notion,
   database_id: &NotionPageId,
   data_source_id: &str,
) -> Result<DataSource, String> {
   // FIXME: once `notion-client` fixes Database deserialization for API >=
   //    2025-09-03, discover the data_source_id from the database object
   //    instead of requiring it as a parameter:
   //
   //     let db = client
   //        .databases
   //        .retrieve_a_database(database_id)
   //        .await
   //        .map_err(|err| format!("Failed to retrieve Database {}: {:?}", database_id, err))?;
   //     let ds_ref = db
   //        .data_sources
   //        .first()
   //        .ok_or_else(|| format!("Database {} has no DataSource", database_id))?;
   //     let data_source_id = &ds_ref.id;

   let ds = client
      .data_sources
      .retrieve_a_data_source(data_source_id)
      .await
      .map_err(|err| {
         format!(
            "Failed to retrieve DataSource {} for Database {}: {:?}",
            data_source_id, database_id, err
         )
      })?;

   debug!(?ds, "Retrieved DataSource for Database {}", database_id);
   Ok(ds)
}

fn validate_relation_property(
   data_source: &DataSource,
   property_name: &str,
   expected_target_db: &NotionPageId,
) -> Result<(), String> {
   let property = data_source
      .properties
      .get(property_name)
      .ok_or_else(|| format!("Missing required property '{}' in DataSource", property_name))?;

   match property {
      DatabaseProperty::Relation { relation, .. } => {
         let actual_db_id = relation
            .database_id
            .as_ref()
            .ok_or_else(|| format!("'{}' relation has no database_id", property_name))?;

         if actual_db_id != expected_target_db {
            return Err(format!(
               "'{}' property points to wrong Database: expected {}, got {}",
               property_name, expected_target_db, actual_db_id
            ));
         }

         trace!("Validated '{}' property points to target Database", property_name);
      }
      _ => {
         return Err(format!(
            "'{}' property must be a relation type, found: {:?}",
            property_name, property
         ));
      }
   }
   Ok(())
}

async fn validate_notion_databases(
   client: &Notion,
   things_db: &NotionPageId,
   things_ds_id: &str,
   containers_db: &NotionPageId,
   containers_ds_id: &str,
   things_column_name: &str,
   containers_column_name: &str,
) -> Result<(), String> {
   let things_ds = retrieve_data_source(client, things_db, things_ds_id).await?;
   let containers_ds = retrieve_data_source(client, containers_db, containers_ds_id).await?;

   validate_relation_property(&things_ds, things_column_name, containers_db)?;
   validate_relation_property(&containers_ds, containers_column_name, things_db)?;

   Ok(())
}

fn init_tracing() {
   use tracing_subscriber::{fmt, EnvFilter};

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
   let things_ndb = NotionPageId::new(dotenvy::var("NOTION_THINGS_DB").expect("NOTION_THINGS_DB must be set"))
      .expect("Invalid NOTION_THINGS_DB format");
   let things_column = dotenvy::var("NOTION_THINGS_COLUMN_NAME").expect("NOTION_THINGS_COLUMN_NAME must be set");
   let containers_ndb =
      NotionPageId::new(dotenvy::var("NOTION_CONTAINERS_DB").expect("NOTION_CONTAINERS_DB must be set"))
         .expect("Invalid NOTION_CONTAINERS_DB format");
   let containers_column =
      dotenvy::var("NOTION_CONTAINERS_COLUMN_NAME").expect("NOTION_CONTAINERS_COLUMN_NAME must be set");
   // Data source IDs (required until notion-client supports the 2025-09-03 Database schema)
   let things_ds_id = dotenvy::var("NOTION_THINGS_DS_ID").expect("NOTION_THINGS_DS_ID must be set");
   let containers_ds_id = dotenvy::var("NOTION_CONTAINERS_DS_ID").expect("NOTION_CONTAINERS_DS_ID must be set");

   let pool = initialize_connection(&database_url)
      .await
      .expect("Failed to connect to database");

   let client = Notion::new(notion_token.clone(), None).expect("Failed to create Notion client");

   trace!(%things_ndb, %containers_ndb, "Parsed database IDs");
   validate_notion_databases(
      &client,
      &things_ndb,
      &things_ds_id,
      &containers_ndb,
      &containers_ds_id,
      &things_column,
      &containers_column,
   )
   .await
   .unwrap();
   trace!(things_column, containers_column, "Validated Notion database relations");

   let app_state = AppState {
      pool: pool.clone(),
      client,
   };
   let app = Router::new()
      .route("/", get(|| async { "Hello, World!" }))
      .route("/healthz", get(health_check))
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

   let port = dotenvy::var("PORT").unwrap_or_else(|_| "3000".to_string());
   let addr = format!("0.0.0.0:{}", port);
   let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
   println!("Listening on http://{}", listener.local_addr().unwrap());

   let shutdown_signal = async move {
      use tokio::signal::unix::{signal, SignalKind};
      let mut sigterm = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");
      tokio::select! {
         _ = tokio::signal::ctrl_c() => {},
         _ = sigterm.recv() => {},
      }
      trace!("Shutdown signal received, closing database connections");
      pool.close().await;
   };

   axum::serve(listener, app)
      .with_graceful_shutdown(shutdown_signal)
      .await
      .unwrap();
}

fn as_html(mut resp: Response) -> Response {
   resp
      .headers_mut()
      .insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap());
   resp
}

async fn health_check(extract::State(state): extract::State<AppState>) -> StatusCode {
   match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
      Ok(_) => StatusCode::OK,
      Err(_) => StatusCode::SERVICE_UNAVAILABLE,
   }
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
   target_url: &'a Option<String>,
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
      target_url,
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
