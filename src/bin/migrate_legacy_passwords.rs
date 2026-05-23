use std::{env, process};

use anyhow::{Context, Result, bail};
use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use sqlx::{FromRow, postgres::PgPoolOptions};

const MIGRATED_PASSWORD_SCHEME: &str = "argon2id-md5-v1";
const ARGON2_MEMORY_KIB: u32 = 19 * 1024;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const SCHEMA_SQL: &str = include_str!("../../scripts/migrate_legacy_password_schema.sql");

#[derive(Debug, FromRow)]
struct LegacyCredential {
    id: i32,
    password: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    require_execute_flag()?;

    let database_url = env::var("DATABASE_URL")
        .context("DATABASE_URL is required, for example postgres:///dogn")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .context("failed to connect to PostgreSQL")?;
    let mut transaction = pool.begin().await.context("failed to begin transaction")?;

    sqlx::raw_sql(SCHEMA_SQL)
        .execute(&mut *transaction)
        .await
        .context("failed to apply credential schema changes")?;

    let unsupported_schemes = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM user_info
        WHERE password_scheme IS NOT NULL
          AND password_scheme <> ''
          AND password_scheme NOT IN ('argon2id-md5-v1', 'argon2id-v1')
        "#,
    )
    .fetch_one(&mut *transaction)
    .await
    .context("failed to inspect credential schemes")?;
    if unsupported_schemes != 0 {
        bail!(
            "found {unsupported_schemes} credential(s) with unsupported password_scheme; no changes committed"
        );
    }

    let credentials = sqlx::query_as::<_, LegacyCredential>(
        r#"
        SELECT id, password
        FROM user_info
        WHERE password_scheme IS NULL OR password_scheme = ''
        ORDER BY id
        FOR UPDATE
        "#,
    )
    .fetch_all(&mut *transaction)
    .await
    .context("failed to load unmigrated active credentials")?;

    for credential in &credentials {
        if !is_legacy_md5_hash(&credential.password) {
            bail!(
                "user_info.id={} does not contain a lowercase 32-character MD5 value; no changes committed",
                credential.id
            );
        }
    }

    let argon2 = configured_argon2id()?;
    for credential in &credentials {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = argon2
            .hash_password(credential.password.as_bytes(), &salt)
            .map_err(|error| {
                anyhow::anyhow!("failed to hash user_info.id={}: {error}", credential.id)
            })?
            .to_string();

        sqlx::query(
            r#"
            UPDATE user_info
            SET password = $1,
                password_scheme = $2
            WHERE id = $3
              AND (password_scheme IS NULL OR password_scheme = '')
            "#,
        )
        .bind(password_hash)
        .bind(MIGRATED_PASSWORD_SCHEME)
        .bind(credential.id)
        .execute(&mut *transaction)
        .await
        .with_context(|| {
            format!(
                "failed to update credential for user_info.id={}",
                credential.id
            )
        })?;
    }

    transaction
        .commit()
        .await
        .context("failed to commit credential migration")?;

    println!(
        "Migrated {} active credential(s) to {MIGRATED_PASSWORD_SCHEME}.",
        credentials.len()
    );
    println!(
        "Review info_bak separately; this command intentionally changes only active user_info credentials."
    );
    Ok(())
}

fn require_execute_flag() -> Result<()> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_usage();
        process::exit(0);
    }
    if arguments.as_slice() != ["--execute"] {
        print_usage();
        bail!("refusing to modify credentials without exactly one --execute flag");
    }
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage:\n  DATABASE_URL=postgres:///dogn cargo run --bin migrate_legacy_passwords -- --execute\n\n\
         This command modifies user_info.password and user_info.password_scheme atomically."
    );
}

fn configured_argon2id() -> Result<Argon2<'static>> {
    let parameters = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        None,
    )
    .context("invalid Argon2id configuration")?;

    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, parameters))
}

fn is_legacy_md5_hash(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::is_legacy_md5_hash;

    #[test]
    fn accepts_lowercase_md5_digest() {
        assert!(is_legacy_md5_hash("5f4dcc3b5aa765d61d8327deb882cf99"));
    }

    #[test]
    fn rejects_non_legacy_or_uppercase_credentials() {
        assert!(!is_legacy_md5_hash("$argon2id$v=19$m=19456,t=2,p=1$hash"));
        assert!(!is_legacy_md5_hash("5F4DCC3B5AA765D61D8327DEB882CF99"));
        assert!(!is_legacy_md5_hash(""));
    }
}
