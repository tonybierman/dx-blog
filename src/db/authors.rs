use crate::model::AuthorProfile;
use arium_dioxus::pool::Pool;

pub async fn get_author_profile_db(
    pool: &Pool,
    username: &str,
) -> Result<Option<AuthorProfile>, sqlx::Error> {
    sqlx::query_as::<_, AuthorProfile>(
        r#"
        SELECT u.id AS user_id, u.username,
               COALESCE(u.display_name, u.username) AS display_name,
               u.avatar_url, up.bio, up.social_links
        FROM users u
        LEFT JOIN user_profiles up ON up.user_id = u.id
        WHERE u.username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}
