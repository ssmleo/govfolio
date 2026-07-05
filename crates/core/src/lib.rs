pub mod alerts;
pub mod db;
pub mod domain;
pub mod ids;
pub mod query;
pub mod schemas;
pub mod stripe;

#[must_use]
pub fn hello() -> &'static str {
    "govfolio"
}

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_smoke() {
        assert_eq!(super::hello(), "govfolio");
    }
}
