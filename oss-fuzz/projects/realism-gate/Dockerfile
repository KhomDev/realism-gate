# OSS-Fuzz build image for realism-gate's cargo-fuzz targets.
#
# Layout follows google/oss-fuzz/projects/<name>/. Nothing here is submitted
# upstream: this copy exists so the configuration is reviewable and testable
# from the repository it builds.
#
# The source is pinned. OSS-Fuzz normally clones the default branch, which
# would make every crash report depend on whatever HEAD was that day. Bump
# RG_COMMIT and the comment together, in a commit that says why.
FROM gcr.io/oss-fuzz-base/base-builder-rust

# The base image's nightly (2025-09-05) is older than the crate's rust-version
# (1.95), so install and select a nightly new enough for the pinned commit.
# rustc 1.102.0-nightly (2026-10-27). Installing here, not in build.sh, keeps
# the compile step free of network access.
RUN rustup toolchain install nightly-2026-11-01 --profile minimal
ENV RUSTUP_TOOLCHAIN=nightly-2026-11-01

# 1.0.4, 2026-11-22: the commit the fuzz/corpus seeds were last checked against.
ARG RG_COMMIT=6019773976706e42e11ceefc2ee1b03496f2df54

RUN git clone https://github.com/KhomDev/realism-gate.git "$SRC/realism-gate" \
    && cd "$SRC/realism-gate" \
    && git checkout --detach "$RG_COMMIT"

WORKDIR $SRC/realism-gate

COPY build.sh $SRC/
