use std::collections::{BTreeMap, BTreeSet};

use crate::model::Provider;

#[derive(Debug)]
pub(crate) struct SanitizedEnvironment {
    pub(crate) env: Vec<(String, String)>,
    pub(crate) remove_env: Vec<String>,
}

pub(crate) fn sanitize_environment(
    provider: Provider,
    lane_id: &str,
    inherited: &[(String, String)],
) -> SanitizedEnvironment {
    let mut retained = BTreeMap::<String, (String, String)>::new();
    let mut removed = BTreeSet::<String>::new();

    for (key, value) in inherited {
        let normalized = key.to_ascii_uppercase();
        if key_is_valid(key) && is_allowed(provider, &normalized) {
            retained.insert(normalized, (key.clone(), value.clone()));
        } else {
            removed.insert(key.clone());
        }
    }

    replace_explicit(&mut retained, &mut removed, "GOVFOLIO_LANE_ID", lane_id);
    replace_explicit(&mut retained, &mut removed, "NO_COLOR", "1");
    replace_explicit(&mut retained, &mut removed, "GIT_CONFIG_NOSYSTEM", "1");
    replace_explicit(
        &mut retained,
        &mut removed,
        "GIT_CONFIG_GLOBAL",
        null_device(),
    );
    replace_explicit(&mut retained, &mut removed, "GIT_CONFIG_COUNT", "1");
    replace_explicit(
        &mut retained,
        &mut removed,
        "GIT_CONFIG_KEY_0",
        "credential.helper",
    );
    replace_explicit(&mut retained, &mut removed, "GIT_CONFIG_VALUE_0", "");
    replace_explicit(&mut retained, &mut removed, "GIT_TERMINAL_PROMPT", "0");
    replace_explicit(&mut retained, &mut removed, "GCM_INTERACTIVE", "Never");
    let gh_config = std::env::temp_dir()
        .join("govfolio-provider-no-gh")
        .to_string_lossy()
        .into_owned();
    replace_explicit(&mut retained, &mut removed, "GH_CONFIG_DIR", &gh_config);

    SanitizedEnvironment {
        env: retained.into_values().collect(),
        remove_env: removed.into_iter().collect(),
    }
}

fn replace_explicit(
    retained: &mut BTreeMap<String, (String, String)>,
    removed: &mut BTreeSet<String>,
    key: &str,
    value: &str,
) {
    removed.retain(|existing| !existing.eq_ignore_ascii_case(key));
    if let Some((original, _)) = retained.remove(key)
        && original != key
    {
        removed.insert(original);
    }
    retained.insert(key.to_owned(), (key.to_owned(), value.to_owned()));
}

const fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn key_is_valid(key: &str) -> bool {
    !key.is_empty() && !key.contains(['=', '\0'])
}

fn is_allowed(provider: Provider, key: &str) -> bool {
    is_common_key(key) || matches!((provider, key), (Provider::Codex, "CODEX_HOME"))
}

fn is_common_key(key: &str) -> bool {
    matches!(
        key,
        "ALL_PROXY"
            | "CARGO_HOME"
            | "CARGO_TARGET_DIR"
            | "COMSPEC"
            | "DATABASE_URL"
            | "GIT_EXEC_PATH"
            | "GOVFOLIO_BRONZE_ROOT"
            | "HOME"
            | "HOMEDRIVE"
            | "HOMEPATH"
            | "HTTP_PROXY"
            | "HTTPS_PROXY"
            | "LANG"
            | "LC_ALL"
            | "LC_CTYPE"
            | "NODE_EXTRA_CA_CERTS"
            | "NO_PROXY"
            | "PATH"
            | "PATHEXT"
            | "RUSTUP_HOME"
            | "SQLX_OFFLINE"
            | "SSL_CERT_DIR"
            | "SSL_CERT_FILE"
            | "SYSTEMROOT"
            | "TEMP"
            | "TERM"
            | "TMP"
            | "TMPDIR"
            | "USERPROFILE"
            | "WINDIR"
    )
}
