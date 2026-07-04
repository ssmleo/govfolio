// sqlx::migrate! embeds migration files at compile time; without this, adding a new
// migration does not rebuild the lib and the embedded migrator silently runs stale.
// (Standard sqlx guidance for stable toolchains.)
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
