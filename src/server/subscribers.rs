//! Newsletter subscription with double opt-in. `subscribe` records a pending
//! (unconfirmed) row and emails a confirmation link; `confirm_subscription`
//! flips the `confirmed` flag when that link is followed.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension, MailExtension};

#[post("/api/subscribe", db: DbExtension, mail: MailExtension)]
pub async fn subscribe(email: String) -> Result<()> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 {
        return Err(ServerFnError::new("Please enter a valid email address.").into());
    }

    // Upsert the subscriber row (idempotent on the unique email).
    sqlx::query("INSERT OR IGNORE INTO subscribers (email) VALUES (?)")
        .bind(&email)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    let (sub_id, confirmed): (i64, i64) =
        sqlx::query_as("SELECT id, confirmed FROM subscribers WHERE email = ?")
            .bind(&email)
            .fetch_one(&db.0)
            .await
            .map_err(sfe)?;

    // Already confirmed — nothing to do, and we don't re-mail.
    if confirmed != 0 {
        return Ok(());
    }

    // Issue a fresh token (SQLite's randomblob avoids pulling in a rand crate),
    // replacing any earlier pending token for this subscriber.
    let token: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(sub_id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("INSERT INTO subscriber_tokens (token, subscriber_id) VALUES (?, ?)")
        .bind(&token)
        .bind(sub_id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    // Send the confirmation email. A flaky mailer must not fail the request —
    // mirror arium's auth flows and just log on error.
    let link = format!("{}/subscribe/confirm?token={token}", mail.0.base_url());
    let subject = "Confirm your subscription";
    let text = format!(
        "Thanks for subscribing!\n\nPlease confirm your email by visiting:\n{link}\n\n\
         If you didn't request this, you can ignore this message."
    );
    let html = format!(
        "<p>Thanks for subscribing!</p>\
         <p>Please confirm your email by clicking the link below:</p>\
         <p><a href=\"{link}\">Confirm subscription</a></p>\
         <p style=\"color:#888\">If you didn't request this, you can ignore this message.</p>"
    );
    if let Err(err) = mail.0.send(&email, subject, &text, Some(&html)).await {
        eprintln!("[mail] WARN: failed to send subscription confirmation: {err}");
    }

    Ok(())
}

/// Consume a confirmation token, marking the subscriber confirmed. Returns
/// `true` when a pending token matched, `false` when it was unknown/expired.
#[post("/api/subscribe/confirm", db: DbExtension)]
pub async fn confirm_subscription(token: String) -> Result<bool> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Ok(false);
    }

    let sub_id: Option<i64> =
        sqlx::query_scalar("SELECT subscriber_id FROM subscriber_tokens WHERE token = ?")
            .bind(&token)
            .fetch_optional(&db.0)
            .await
            .map_err(sfe)?;

    let Some(sub_id) = sub_id else {
        return Ok(false);
    };

    sqlx::query("UPDATE subscribers SET confirmed = 1 WHERE id = ?")
        .bind(sub_id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(sub_id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    Ok(true)
}
