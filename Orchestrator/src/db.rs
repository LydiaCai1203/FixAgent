use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Run lightweight schema migrations to ensure required columns exist
    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        ALTER TABLE IF EXISTS issues
        ADD COLUMN IF NOT EXISTS suggestion_code TEXT
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        ALTER TABLE IF EXISTS issues
        ADD COLUMN IF NOT EXISTS original_code TEXT
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
