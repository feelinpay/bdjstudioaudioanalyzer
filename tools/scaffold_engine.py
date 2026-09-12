import os

engine_dir = 'engine'
os.makedirs(engine_dir, exist_ok=True)

cargo_workspace = """[workspace]
resolver = "2"
members = [
    "bdja_core",
    "bdja_decode",
    "bdja_dsp",
    "bdja_verdict",
    "bdja_scan",
    "bdja_store",
    "bdja_worker",
    "bdja_ipc",
    "bdja_ffi",
    "bdja_cli",
]

[workspace.package]
version = "1.0.0"
edition = "2021"
authors = ["BDJ Studio <dev@bdjstudio.com>"]
license = "Proprietary"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
strip = true

[profile.bench]
opt-level = 3
lto = "thin"
codegen-units = 1
"""
with open(os.path.join(engine_dir, 'Cargo.toml'), 'w', encoding='utf-8') as f:
    f.write(cargo_workspace)

crates = [
    ('bdja_core', 'lib', """[package]
name = "bdja_core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
"""),
    ('bdja_decode', 'lib', """[package]
name = "bdja_decode"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
symphonia = { version = "0.5", default-features = false, features = ["all-codecs", "all-formats"] }
thiserror = "2.0"
tracing = "0.1"
"""),
    ('bdja_dsp', 'lib', """[package]
name = "bdja_dsp"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
realfft = "3.4"
rustfft = "6.2"
rubato = "0.15"
thiserror = "2.0"
"""),
    ('bdja_verdict', 'lib', """[package]
name = "bdja_verdict"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
"""),
    ('bdja_scan', 'lib', """[package]
name = "bdja_scan"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
bdja_store = { path = "../bdja_store" }
jwalk = "0.8"
crossbeam-channel = "0.5"
blake3 = "1.5"
thiserror = "2.0"
tracing = "0.1"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Win32_Storage_FileSystem", "Win32_Foundation", "Win32_System_SystemInformation"] }
"""),
    ('bdja_store', 'lib', """[package]
name = "bdja_store"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
rusqlite = { version = "0.32", features = ["bundled"] }
serde_json = "1.0"
thiserror = "2.0"
"""),
    ('bdja_worker', 'bin', """[package]
name = "bdja_worker"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
bdja_decode = { path = "../bdja_decode" }
bdja_dsp = { path = "../bdja_dsp" }
bincode = "1.3"
serde = { version = "1.0", features = ["derive"] }
"""),
    ('bdja_ipc', 'lib', """[package]
name = "bdja_ipc"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
bincode = "1.3"
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
tracing = "0.1"
"""),
    ('bdja_ffi', 'lib', """[package]
name = "bdja_ffi"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "staticlib"]

[lints.rust]
linker_messages = "allow"

[dependencies]
bdja_core = { path = "../bdja_core" }
bdja_scan = { path = "../bdja_scan" }
bdja_store = { path = "../bdja_store" }
bdja_decode = { path = "../bdja_decode" }
bdja_dsp = { path = "../bdja_dsp" }
bdja_verdict = { path = "../bdja_verdict" }
flutter_rust_bridge = "=2.13.0"
thiserror = "2.0"
tracing = "0.1"
parking_lot = "0.12"
"""),
    ('bdja_cli', 'bin', """[package]
name = "bdja_cli"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
bdja_core = { path = "../bdja_core" }
bdja_decode = { path = "../bdja_decode" }
bdja_dsp = { path = "../bdja_dsp" }
bdja_verdict = { path = "../bdja_verdict" }
bdja_scan = { path = "../bdja_scan" }
bdja_store = { path = "../bdja_store" }
""")
]

for name, kind, toml in crates:
    cdir = os.path.join(engine_dir, name)
    sdir = os.path.join(cdir, 'src')
    os.makedirs(sdir, exist_ok=True)
    with open(os.path.join(cdir, 'Cargo.toml'), 'w', encoding='utf-8') as f:
        f.write(toml)
    if kind == 'lib':
        with open(os.path.join(sdir, 'lib.rs'), 'w', encoding='utf-8') as f:
            f.write(f'// {name} library\n')
    else:
        with open(os.path.join(sdir, 'main.rs'), 'w', encoding='utf-8') as f:
            f.write(f'fn main() {{\n    println!("{name} v{{}}", env!("CARGO_PKG_VERSION"));\n}}\n')
print('Crates scaffolded successfully!')
