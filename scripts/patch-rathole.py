#!/usr/bin/env python3
"""Auditable downstream build-only hardening for rathole 0.5.0.
Remove obsolete build tooling and increase UDP receive buffers without changing the wire protocol.
"""
from pathlib import Path
import sys,re
root=Path(sys.argv[1]);manifest=root/'Cargo.toml';text=manifest.read_text()
text=re.sub(r'\[dependencies\.atty\]\n.*?(?=\n\[)', '',text,flags=re.S)
text=re.sub(r'\[build-dependencies\.vergen\]\n.*?(?=\n\[)', '',text,flags=re.S)
for dependency in ['console-subscriber','tokio-native-tls','tokio-tungstenite','tokio-util','futures-core','futures-sink']:
    text=re.sub(r'\[dependencies\.'+re.escape(dependency)+r'\]\n.*?(?=\n\[)', '',text,flags=re.S)
text=text.replace('[dependencies.clap]\n','[dependencies.clap]\ndefault-features = false\n')
# clap's color feature pulls the unsafe atty implementation. Plain help output needs only std + derive.
text=re.sub(r'(\[dependencies.clap\].*?features = )\[[^]]*\]',r'\1["derive", "std"]',text,flags=re.S)
features='''[features]
default = ["server", "client", "noise", "hot-reload"]
embedded = ["server", "client", "noise", "hot-reload"]
server = []
client = []
noise = ["snowstorm", "base64"]
hot-reload = ["notify"]

'''
text=re.sub(r'\[features\].*?(?=\[workspace\]|\Z)',features,text,flags=re.S)
text=text.replace("default-features = false\ndefault-features = false", "default-features = false")
manifest.write_text(text)
main=root/'src/main.rs';text=main.read_text().replace('atty::is(atty::Stream::Stdout)','std::io::IsTerminal::is_terminal(&std::io::stdout())');main.write_text(text)
(root/'build.rs').write_text(r'''fn main() {
    println!("cargo:rustc-check-cfg=cfg(feature, values(\"console\", \"tls\", \"websocket\"))");
    for (key,value) in [
        ("VERGEN_BUILD_SEMVER", "0.5.0-holepunch.2".to_string()),
        ("VERGEN_BUILD_TIMESTAMP", "reproducible".to_string()),
        ("VERGEN_CARGO_TARGET_TRIPLE", std::env::var("TARGET").unwrap()),
        ("VERGEN_CARGO_PROFILE", std::env::var("PROFILE").unwrap()),
        ("VERGEN_CARGO_FEATURES", "server,client,noise,hot-reload".to_string()),
    ] { println!("cargo:rustc-env={key}={value}"); }
}
''')

# The wire protocol already carries u16 lengths; the upstream 2 KiB receive buffer
# silently truncated larger datagrams before they reached the shared gateway.
constants=root/'src/constants.rs'
text=constants.read_text()
assert 'pub const UDP_BUFFER_SIZE: usize = 2048;' in text or 'pub const UDP_BUFFER_SIZE: usize = 65507;' in text
constants.write_text(text.replace('pub const UDP_BUFFER_SIZE: usize = 2048;', 'pub const UDP_BUFFER_SIZE: usize = 65507;'))
