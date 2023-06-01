FROM rustembedded/cross:x86_64-unknown-linux-musl
RUN dpkg --add-architecture amd64 && \
    apt-get update && \
    apt-get install --assume-yes libssl-dev:amd64 &&  \
    apt-get install --assume-yes libssl-dev &&  \
    apt-get install --assume-yes sqlite3:amd64 &&  \
    apt-get install --assume-yes libsqlite3-dev:amd64