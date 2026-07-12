const COMMITTED_OPENAPI: &str = include_str!("../../../packages/contracts/openapi.json");

#[test]
fn generated_openapi_matches_committed_contract_byte_for_byte() {
    let generated = match api::openapi_json() {
        Ok(document) => document,
        Err(error) => panic!("generating OpenAPI document failed: {error:#}"),
    };

    assert!(
        generated.as_bytes() == COMMITTED_OPENAPI.as_bytes(),
        "generated OpenAPI differs from packages/contracts/openapi.json; run `cargo run -p api --bin openapi`"
    );
}
