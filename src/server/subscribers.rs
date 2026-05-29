//! Newsletter subscription endpoint (public create).

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[post("/api/subscribe", db: DbExtension)]
pub async fn subscribe(email: String) -> Result<()> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 {
        return Err(ServerFnError::new("Please enter a valid email address.").into());
    }
    sqlx::query("INSERT OR IGNORE INTO subscribers (email) VALUES (?)")
        .bind(&email)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}
