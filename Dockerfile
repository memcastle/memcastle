# syntax=docker/dockerfile:1
# Consume existing build outputs, from the checkout locally or downloaded release artifacts in CI.
FROM debian:stable-slim

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 memcastle \
    && useradd --uid 10001 --gid 10001 --home-dir /data --no-create-home memcastle \
    && install -d -o 10001 -g 10001 /data /data/config /data/data /data/state

ARG BIN=target/debug/memcastle
ARG WEB=web/dist
COPY --chmod=755 ${BIN} /usr/local/bin/memcastle
COPY ${WEB} /usr/local/share/memcastle/web/
COPY skills/ /usr/local/share/memcastle/skills/

ENV HOME=/data \
    XDG_CONFIG_HOME=/data/config \
    XDG_DATA_HOME=/data/data \
    XDG_STATE_HOME=/data/state \
    MEMCASTLE_BIND=0.0.0.0
USER 10001:10001
WORKDIR /data
EXPOSE 8420
STOPSIGNAL SIGTERM
# The CLI discovers the daemon through its registry, including bind/port overrides.
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD ["memcastle", "status"]
ENTRYPOINT ["/usr/local/bin/memcastle"]
CMD ["serve"]
