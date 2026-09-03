// The workspace build script is intentionally kept dependency-free. The binary
// crate owns the platform C++ build in app/build.rs; this marker keeps the
// repository layout compatible with distributors that expect a root build.rs.
fn main() {}
