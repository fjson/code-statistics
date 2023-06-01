#!/bin/bash -e
cargo build -r
cross build --target x86_64-unknown-linux-musl --release
upx ./target/x86_64-unknown-linux-musl/release/code-statistics
scp ./target/x86_64-unknown-linux-musl/release/code-statistics root@172.21.56.252:~/jenkins/scripts
ssh root@172.21.56.252 > /dev/null 2>&1 << eeooff
chmod +x ~/jenkins/scripts/code-statistics
exit
eeooff