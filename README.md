rocket-static-files
===================

Serves static files with far-future cache headers, and version specific URLs.

## Usage

### Dependencies

Add the following dependencies:

```toml
[dependencies]
rocket-static-files = "0.1"

[build-dependencies]
rocket-static-files = { version = "0.1", features = [ "gen" ] }
```

### Build Script

To generate the hashes, add the following to your `build.rs` (this assumes your static files are located at `$CARGO_MANIFEST_DIR/static`):

```rust
use std::path::PathBuf;

fn main() {
    let mut static_root =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    static_root.push("static");

    let mut out_path = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    out_path.push("static_file_hashes.rs");

    rocket_static_files::generate(&out_path, &static_root).unwrap();
}
```

### Fairing

```rust
use rocket_static_files::StaticFiles;

include!(concat!(env!("OUT_DIR"), "/static_file_hashes.rs"));

fn main() {
    rocket::ignite()
        .attach(StaticFiles::fairing(&STATIC_FILE_HASHES)
        .launch();
}
```

### `Rocket.toml`

Add a section like this:

```toml
[global.static_files]
serve_from = "./static"         # Relative to Rocket.toml
path_prefix = "/static"         # Where to serve the files: http://127.0.0.1:8000/static
```
