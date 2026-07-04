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
