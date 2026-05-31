//! User management, delegated entirely to arium's admin UI (list + detail).

use dioxus::prelude::*;

use crate::components::text::PageTitle;

use super::AdminShell;

#[component]
pub fn AdminUsers() -> Element {
    let mut selected = use_signal::<Option<i64>>(|| None);
    rsx! {
        AdminShell { active: "users".to_string(),
            PageTitle { "User management" }
            if let Some(uid) = selected() {
                arium_dioxus::ui::AdminUserDetail { user_id: uid, on_back: move |_| selected.set(None) }
            } else {
                arium_dioxus::ui::AdminUserList { on_select: move |id: i64| selected.set(Some(id)) }
            }
        }
    }
}
