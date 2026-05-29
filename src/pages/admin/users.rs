//! User management, delegated entirely to arium's admin UI (list + detail).

use dioxus::prelude::*;

use super::AdminShell;

#[component]
pub fn AdminUsers() -> Element {
    let mut selected = use_signal::<Option<i64>>(|| None);
    rsx! {
        AdminShell { active: "users".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "User management" }
            if let Some(uid) = selected() {
                arium_dioxus::ui::AdminUserDetail { user_id: uid, on_back: move |_| selected.set(None) }
            } else {
                arium_dioxus::ui::AdminUserList { on_select: move |id: i64| selected.set(Some(id)) }
            }
        }
    }
}
