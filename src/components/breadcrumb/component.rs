use dioxus::prelude::*;

use crate::Route;

#[derive(Clone)]
struct BreadcrumbItem {
    label: String,
    route: Option<Route>,
}

impl BreadcrumbItem {
    fn new(label: impl Into<String>, route: Option<Route>) -> Self {
        Self {
            label: label.into(),
            route,
        }
    }
}

fn slug_to_label(slug: &str) -> String {
    slug.replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn home() -> BreadcrumbItem {
    BreadcrumbItem::new("Home", Some(Route::HomePage))
}

fn admin() -> BreadcrumbItem {
    BreadcrumbItem::new("Admin", Some(Route::AdminDashboard))
}

fn route_to_crumbs(route: &Route) -> Vec<BreadcrumbItem> {
    match route {
        // ── Reader ───────────────────────────────────────────────────────────
        Route::HomePage => vec![],
        Route::PostDetail { slug } => {
            vec![home(), BreadcrumbItem::new(slug_to_label(slug), None)]
        }
        Route::CategoryFeed { slug } => vec![
            home(),
            BreadcrumbItem::new("Categories", None),
            BreadcrumbItem::new(slug_to_label(slug), None),
        ],
        Route::TagFeed { slug } => vec![
            home(),
            BreadcrumbItem::new("Tags", None),
            BreadcrumbItem::new(slug_to_label(slug), None),
        ],
        Route::AuthorProfile { slug } => vec![
            home(),
            BreadcrumbItem::new("Authors", None),
            BreadcrumbItem::new(slug_to_label(slug), None),
        ],
        Route::Archive => vec![home(), BreadcrumbItem::new("Archive", None)],
        Route::SearchResults { q } if !q.is_empty() => {
            vec![home(), BreadcrumbItem::new("Search Results", None)]
        }
        Route::SearchResults { .. } => vec![home(), BreadcrumbItem::new("Search", None)],
        Route::Subscribe => vec![home(), BreadcrumbItem::new("Subscribe", None)],
        Route::ConfirmSubscription { .. } => vec![],

        // ── Admin ────────────────────────────────────────────────────────────
        Route::AdminDashboard => vec![BreadcrumbItem::new("Admin", None)],
        Route::AdminPostList => vec![admin(), BreadcrumbItem::new("Posts", None)],
        Route::AdminPostNew => vec![
            admin(),
            BreadcrumbItem::new("Posts", Some(Route::AdminPostList)),
            BreadcrumbItem::new("New Post", None),
        ],
        Route::AdminPostEdit { .. } => vec![
            admin(),
            BreadcrumbItem::new("Posts", Some(Route::AdminPostList)),
            BreadcrumbItem::new("Edit Post", None),
        ],
        Route::AdminMedia => vec![admin(), BreadcrumbItem::new("Media", None)],
        Route::AdminComments => vec![admin(), BreadcrumbItem::new("Comments", None)],
        Route::AdminUsers => vec![admin(), BreadcrumbItem::new("Users", None)],
        Route::AdminSettings => vec![admin(), BreadcrumbItem::new("Settings", None)],
        Route::AdminAppearance => vec![admin(), BreadcrumbItem::new("Appearance", None)],
        Route::AdminTaxonomy => vec![admin(), BreadcrumbItem::new("Taxonomy", None)],
        Route::AdminAnalytics => vec![admin(), BreadcrumbItem::new("Analytics", None)],

        // ── Auth ─────────────────────────────────────────────────────────────
        Route::AccountPage => vec![home(), BreadcrumbItem::new("Account", None)],
        Route::LoginPage => vec![home(), BreadcrumbItem::new("Login", None)],
        Route::RegisterPage => vec![home(), BreadcrumbItem::new("Register", None)],
        Route::ForgotPasswordPage => {
            vec![home(), BreadcrumbItem::new("Forgot Password", None)]
        }
        Route::ResetPasswordPage { .. } => vec![],
        Route::VerifyEmailPage { .. } => vec![],

        // ── Fallbacks ─────────────────────────────────────────────────────────
        Route::ServerError => vec![],
        Route::NotFound { .. } => vec![],
    }
}

#[component]
pub fn Breadcrumb() -> Element {
    let route = use_route::<Route>();
    let crumbs = route_to_crumbs(&route);

    if crumbs.is_empty() {
        return rsx! {};
    }

    rsx! {
        nav { "aria-label": "breadcrumb", class: "mb-4",
            ol { class: "flex flex-wrap items-center text-sm",
                for (i , crumb) in crumbs.iter().enumerate() {
                    li { class: "flex items-center",
                        if i > 0 {
                            span {
                                "aria-hidden": "true",
                                class: "mx-1.5 text-base-content/40 select-none",
                                "›"
                            }
                        }
                        if let Some(r) = &crumb.route {
                            Link {
                                to: r.clone(),
                                class: "text-base-content/60 hover:text-base-content transition-colors",
                                "{crumb.label}"
                            }
                        } else {
                            span {
                                "aria-current": "page",
                                class: "text-base-content font-medium",
                                "{crumb.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}
