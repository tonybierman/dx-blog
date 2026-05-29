# Development

The crate compiles to two targets that a plain `cargo check` can't cover at once. Check each:

```sh
cargo check --no-default-features --features server,sqlite              # server
cargo check --no-default-features --features web --target wasm32-unknown-unknown  # client
```

Server-side tests run with:

```sh
cargo test --no-default-features --features server,sqlite
```
