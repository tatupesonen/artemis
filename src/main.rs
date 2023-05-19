use std::time::Duration;

use anyhow::bail;
use anyhow::Context;
use axum::extract::Path;
use axum::extract::State;
use axum::http::{HeaderValue, Method};
use axum::response::IntoResponse;
use axum::routing::*;
use axum::Json;
use axum::Router;

use chrono::NaiveDateTime;
use dotenvy::dotenv;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::AllowOrigin;
use tower_http::cors::CorsLayer;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "artemis=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    dotenv().ok();
    // DB pool.
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set.")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Clone because we need it also in the worker.

    debug!("Running migrations...");
    sqlx::migrate!().run(&pool).await?;

    let app_state = AppState { db: pool.clone() };

    let router = Router::new()
        .route("/feeds", get(handle_feeds_get))
        .route("/feeds", post(handle_feed_add))
        .route("/feeds/:id", get(get_feed_entries_for_feed))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::any())
                .allow_methods([Method::GET, Method::POST]),
        )
        .with_state(app_state);

    // Every 10 seconds, run a task for every feed that checks for new posts.
    tokio::task::spawn(async move {
        let db = pool.clone();
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            let _ = update_feeds(&db).await;
        }
    });

    let bind_addr = "0.0.0.0:3000";
    debug!("Listening on {bind_addr}");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(router.into_make_service())
        .await?;
    Ok(())
}

async fn update_feeds(db: &PgPool) -> anyhow::Result<()> {
    // Get feeds
    let feeds = sqlx::query_as!(Feed, r#"select * from feeds;"#)
        .fetch_all(db)
        .await?;

    // Spawn a update feed task for each feed
    for feed in feeds.into_iter() {
        let local_pool = db.clone();
        debug!("Spawned job for {}", feed.url);
        tokio::task::spawn(async move {
            let _ = persist_posts_from_feed(&feed, &local_pool).await;
        });
    }

    Ok(())
}

struct AppError(anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

#[derive(Deserialize)]
struct AddFeedBody {
    url: String,
    name: String,
}

#[derive(Serialize, sqlx::FromRow, Clone, Debug)]
struct Feed {
    id: i32,
    url: String,
    name: String,
}

async fn handle_feed_add(
    State(state): State<AppState>,
    Json(body): Json<AddFeedBody>,
) -> Result<impl IntoResponse, AppError> {
    // Verify the feed works and it's parseable.
    let rss = reqwest::get(&body.url).await?.bytes().await?;

    let _channel = rss::Channel::read_from(&rss[..])?;
    let created = sqlx::query_as!(
        Feed,
        // language=PostgreSQL
        r#"
				  INSERT INTO feeds (url, name) values ($1, $2) returning id, url, name;
			"#,
        body.url,
        body.name,
    )
    .fetch_one(&state.db)
    .await?;

    dbg!(&created);
    let created_clone = created.clone();
    tokio::task::spawn(async move {
        let db = state.db.clone();
        match persist_posts_from_feed(&created_clone, &db).await {
            Ok(_) => info!("Done updating for {}", &created_clone.url),
            Err(e) => error!("Unable to persist posts for {}: {}", created_clone.url, e),
        }
    });

    Ok(Json(created))
}

async fn persist_posts_from_feed(feed: &Feed, db: &PgPool) -> anyhow::Result<()> {
    // Get posts for feed.
    let xml = reqwest::get(&feed.url).await?.bytes().await?;
    let channel = rss::Channel::read_from(&xml[..])?;
    for item in channel.items.iter() {
        let date = item.pub_date.as_ref();
        let date = date
            .map(|e| chrono::DateTime::parse_from_rfc2822(&e).ok())
            .flatten();
        let date = date.map(|e| e.naive_utc());

        let guid = item.guid.as_ref().map(|e| e.value.clone());

        let result = sqlx::query_scalar!(
            r#"
				insert into feed_entries (title, link, pub_date, guid, feed_id) values ($1, $2, $3, $4, $5) returning title;
			"#,
            item.title,
            item.link,
            date,
            guid,
            &feed.id,
        )
        .fetch_one(db)
        .await;
        if let Ok(res) = result {
            info!("New item: {:?}", res);
        }
    }
    Ok(())
}

async fn handle_feeds_get(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let data = sqlx::query_as!(Feed, "select * from feeds;")
        .fetch_all(&state.db)
        .await?;

    Ok(Json(data))
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct FeedEntry {
    id: i32,
    title: Option<String>,
    link: Option<String>,
    pub_date: Option<NaiveDateTime>,
    guid: Option<String>,
    feed_id: Option<i32>,
}

async fn get_feed_entries_for_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let data = sqlx::query_as!(
        FeedEntry,
        "select * from feed_entries where feed_id = $1 order by pub_date desc LIMIT 50;",
        feed_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(data))
}
