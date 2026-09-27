# syntax=docker/dockerfile:1
# dotconfig CI image: the toolchain .github/workflows/ci.yml installs, plus the
# working tree at /src. Run by the Tekton pipeline in manifests/tekton/ci.yaml
# via `just ci-tekton` (scripts/nu/ci-tekton.nu), which builds it from a
# working-tree tarball (gitignored files never enter the build context).
#
# Tool versions mirror ci.yml's env / install-action pins — bump both together.
# Every download is sha256-verified per architecture (amd64, arm64).

FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a AS base

FROM base AS tools
ARG TARGETARCH
ARG NU_VERSION=0.115.1
ARG SHELLCHECK_VERSION=0.11.0
ARG TAPLO_VERSION=0.10.0
ARG GITLEAKS_VERSION=8.30.1
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
WORKDIR /dl
RUN case "$TARGETARCH" in \
        amd64) arch=x86_64; gl=x64 \
            nu_sha=d11d825241f6504a3617c535fa725a9dd6d009c86d7b19fb3168b47635b9d8b0 \
            sc_sha=b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6 \
            taplo_sha=8fe196b894ccf9072f98d4e1013a180306e17d244830b03986ee5e8eabeb6156 \
            gl_sha=551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb ;; \
        arm64) arch=aarch64; gl=arm64 \
            nu_sha=5c4a5bca0af5b070e903a68fa014cc24e6419d0ac9cec03a2948494b2d310e08 \
            sc_sha=68a8133197a50beb8803f8d42f9908d1af1c5540d4bb05fdfca8c1fa47decefc \
            taplo_sha=033681d01eec8376c3fd38fa3703c79316f5e14bb013d859943b60a07bccdcc3 \
            gl_sha=e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080 ;; \
        *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && fetch() { curl -fsSLo "$1" "$2" && echo "$3  $1" | sha256sum -c -; } \
    && fetch nu.tgz "https://github.com/nushell/nushell/releases/download/${NU_VERSION}/nu-${NU_VERSION}-${arch}-unknown-linux-gnu.tar.gz" "$nu_sha" \
    && fetch shellcheck.tgz "https://github.com/koalaman/shellcheck/releases/download/v${SHELLCHECK_VERSION}/shellcheck-v${SHELLCHECK_VERSION}.linux.${arch}.tar.gz" "$sc_sha" \
    && fetch taplo.gz "https://github.com/tamasfe/taplo/releases/download/${TAPLO_VERSION}/taplo-linux-${arch}.gz" "$taplo_sha" \
    && fetch gitleaks.tgz "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_${gl}.tar.gz" "$gl_sha" \
    && mkdir -p /out \
    && tar -xzf nu.tgz --strip-components=1 -C /out "nu-${NU_VERSION}-${arch}-unknown-linux-gnu/nu" \
    && tar -xzf shellcheck.tgz --strip-components=1 -C /out "shellcheck-v${SHELLCHECK_VERSION}/shellcheck" \
    && gunzip -c taplo.gz > /out/taplo \
    && tar -xzf gitleaks.tgz -C /out gitleaks \
    && chmod 0755 /out/*

FROM base
# git: gitleaks history scan; zsh: `zsh -n generated.zsh`; bash 5 for scripts/ci.sh.
RUN apt-get update \
    && apt-get install -y --no-install-recommends git zsh \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 --user-group ci
COPY --from=tools /out/ /usr/local/bin/
# Numeric, so the pod's runAsNonRoot can verify it.
USER 1000:1000
# CI=true: scripts/ci.sh only writes nu stubs under $HOME on a CI runner.
ENV HOME=/home/ci CI=true
WORKDIR /src
COPY --chown=1000:1000 . /src
