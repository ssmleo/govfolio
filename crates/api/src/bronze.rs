//! Reads a `raw_document`'s archived bytes from wherever `storage_uri` says
//! they live (design §7.3: "our archived copy"). Dispatches on the URI's
//! scheme: local filesystem today (`file://` — the only scheme any adapter
//! currently writes, via `pipeline::adapter::BronzeStore`); object storage
//! (`gs://`) is a documented not-yet-implemented gap, not silently wrong — it
//! fails closed with `503` until the cloud-substrate halt clears
//! (`agents/goals/000-INDEX.md` 020/081).

use crate::error::ApiError;

/// Reads the archived bytes a `raw_document.storage_uri` points at.
///
/// # Errors
/// [`ApiError::Unavailable`] for a scheme this build cannot read yet;
/// [`ApiError::Internal`] on an I/O failure reading a local file.
pub async fn read_document(storage_uri: &str) -> Result<Vec<u8>, ApiError> {
    let Some(path) = storage_uri.strip_prefix("file://") else {
        return Err(ApiError::Unavailable {
            code: "storage_backend_unavailable",
            message: format!(
                "no storage backend implemented yet for {storage_uri} \
                 (object storage arrives once the cloud-substrate halt clears)"
            ),
        });
    };
    tokio::fs::read(path)
        .await
        .map_err(|e| ApiError::from(anyhow::anyhow!("reading bronze document at {path}: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_bytes_from_a_file_scheme_uri() {
        let path =
            std::env::temp_dir().join(format!("govfolio-bronze-test-{}.bin", ulid::Ulid::new()));
        tokio::fs::write(&path, b"hello bronze").await.unwrap();
        let uri = format!("file://{}", path.display());

        let bytes = read_document(&uri).await.unwrap();

        assert_eq!(bytes, b"hello bronze");
        tokio::fs::remove_file(&path).await.unwrap();
    }

    #[tokio::test]
    async fn unsupported_scheme_fails_closed_with_503_not_a_panic() {
        let err = read_document("gs://bucket/object").await.unwrap_err();
        assert!(matches!(err, ApiError::Unavailable { .. }));
    }

    #[tokio::test]
    async fn missing_local_file_is_an_internal_error_not_a_panic() {
        let err = read_document("file:///definitely/not/a/real/path/xyz-govfolio-test")
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Internal(_)));
    }
}
