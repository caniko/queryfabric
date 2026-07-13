#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

CRATES=(
  "queryfabric-ir"
  "queryfabric-catalog"
  "queryfabric-runtime"
  "queryfabric-opt"
  "queryfabric-dialect-sql"
  "queryfabric-dialect-syql"
  "queryfabric-adapter-clickhouse"
  "queryfabric-adapter-postgres"
  "queryfabric"
)

usage() {
  cat <<'EOF'
Usage:
  scripts/release.sh check
  scripts/release.sh publish --version <x.y.z> [--from <crate>] [--execute]
  scripts/release.sh tag --version <x.y.z>

Notes:
  - Publishing is staged because dependent crates cannot dry-run or publish
    cleanly until earlier crates are visible on crates.io for the target version.
  - Without --execute, the publish command validates only the current publishable
    step (the crate selected by --from, or the first crate by default) and
    allows a dirty worktree for local rehearsal.
  - Resume a partially completed release with --from <crate> after crates.io
    propagation catches up.
EOF
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '==> %s\n' "$*"
}

staged_failure_message() {
  local crate="$1"
  local version="$2"
  cat <<EOF

${crate} ${version} is not independently dry-runnable yet because one or more
earlier QueryFabric crates for that version are not visible on crates.io.

This is the expected staged-publication constraint for this workspace.

If you are rehearsing locally, start from the first unpublished crate:

  scripts/release.sh publish --version ${version}

If you are resuming a real publish, wait for the earlier crate to appear on
crates.io, then rerun this staged step:

  scripts/release.sh publish --version ${version} --from ${crate} --execute
EOF
}

crate_manifest() {
  local crate="$1"
  printf '%s/crates/%s/Cargo.toml' "$ROOT_DIR" "$crate"
}

crate_index() {
  local target="$1"
  local i
  for i in "${!CRATES[@]}"; do
    if [[ "${CRATES[$i]}" == "$target" ]]; then
      printf '%s\n' "$i"
      return 0
    fi
  done
  return 1
}

assert_known_crate() {
  local crate="$1"
  crate_index "$crate" >/dev/null || die "unknown crate '$crate'"
}

require_version_arg() {
  local version="${1:-}"
  [[ -n "$version" ]] || die "--version is required"
}

print_publish_plan() {
  local from="${1:-${CRATES[0]}}"
  local start
  start="$(crate_index "$from")"
  log "publish order from ${from}:"
  local i
  for ((i = start; i < ${#CRATES[@]}; i++)); do
    printf '  - %s\n' "${CRATES[$i]}"
  done
}

run_check() {
  cd "$ROOT_DIR"
  log "cargo fmt --all --check"
  cargo fmt --all --check

  log "cargo fmt (fuzz)"
  cargo fmt --manifest-path fuzz/Cargo.toml --all --check

  log "cargo clippy --workspace --all-targets -- -D warnings"
  cargo clippy --workspace --all-targets -- -D warnings

  log "cargo test --workspace --all-targets --exclude queryfabric-python"
  cargo test --workspace --all-targets --exclude queryfabric-python

  log "cargo fuzz build --sanitizer none parse_sql_no_panic"
  (
    cd fuzz
    cargo fuzz build --sanitizer none parse_sql_no_panic
  )

  log "cargo fuzz build --sanitizer none bind_portable_no_panic"
  (
    cd fuzz
    cargo fuzz build --sanitizer none bind_portable_no_panic
  )

  log "cargo build --manifest-path crates/queryfabric/Cargo.toml --examples"
  cargo build --manifest-path crates/queryfabric/Cargo.toml --examples

  local wheel_dir
  wheel_dir="$(mktemp -d)"
  trap "rm -rf '$wheel_dir'" RETURN
  local venv_dir="${wheel_dir}/venv"

  log "uv venv (queryfabric Python binding)"
  (
    cd packages/queryfabric
    uv venv "$venv_dir"
  )

  log "maturin build (queryfabric Python binding)"
  (
    cd packages/queryfabric
    maturin build --release --features extension-module -o "$wheel_dir"
  )

  log "maturin develop --uv (queryfabric Python binding)"
  (
    cd packages/queryfabric
    export VIRTUAL_ENV="$venv_dir"
    export PATH="${VIRTUAL_ENV}/bin:${PATH}"
    maturin develop --uv --features extension-module
  )

  log "uv run python smoke test (queryfabric Python package)"
  (
    cd packages/queryfabric
    export VIRTUAL_ENV="$venv_dir"
    export PATH="${VIRTUAL_ENV}/bin:${PATH}"
    uv run --active python -c \
      "import queryfabric; parsed = queryfabric.parse_syql('FROM records'); assert parsed.table == 'records'"
  )

  log "uv run pytest (queryfabric Python package)"
  (
    cd packages/queryfabric
    export VIRTUAL_ENV="$venv_dir"
    export PATH="${VIRTUAL_ENV}/bin:${PATH}"
    uv run --active pytest tests
  )
}

wait_for_crates_io() {
  local crate="$1"
  local version="$2"
  local url="https://crates.io/api/v1/crates/${crate}/${version}"
  local attempt

  log "waiting for ${crate} ${version} to appear on crates.io"
  for attempt in $(seq 1 60); do
    if curl --fail --silent --show-error "$url" >/dev/null; then
      log "${crate} ${version} is visible on crates.io"
      return 0
    fi

    printf '   not visible yet (%s/60), sleeping 10s\n' "$attempt"
    sleep 10
  done

  die "${crate} ${version} did not appear on crates.io in time; rerun with --from ${crate} or the next crate once propagation completes"
}

publish_step() {
  local crate="$1"
  local version="$2"
  local execute="$3"
  local manifest
  manifest="$(crate_manifest "$crate")"

  if [[ "$execute" != "yes" ]]; then
    log "validating ${crate} ${version} with cargo publish --dry-run --allow-dirty"
    if ! cargo publish --manifest-path "$manifest" --dry-run --allow-dirty; then
      staged_failure_message "$crate" "$version" >&2
      return 1
    fi

    cat <<EOF

Validated staged publish step for ${crate} ${version}.

This repository cannot promise a full dry-run of every crate before publication:
downstream crates depend on earlier crates being visible on crates.io for the
same version. Publish ${crate} first, wait for index propagation, then resume
with:

  scripts/release.sh publish --version ${version} --from ${crate} --execute

or start at the next crate once ${crate} is visible.
EOF
    return 0
  fi

  log "validating ${crate} ${version} with cargo publish --dry-run"
  if ! cargo publish --manifest-path "$manifest" --dry-run; then
    staged_failure_message "$crate" "$version" >&2
    return 1
  fi

  log "publishing ${crate} ${version}"
  cargo publish --manifest-path "$manifest"
  wait_for_crates_io "$crate" "$version"
}

run_publish() {
  local version=""
  local from="${CRATES[0]}"
  local execute="no"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || die "--version requires a value"
        version="$2"
        shift 2
        ;;
      --from)
        [[ $# -ge 2 ]] || die "--from requires a crate name"
        from="$2"
        shift 2
        ;;
      --execute)
        execute="yes"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown publish argument '$1'"
        ;;
    esac
  done

  require_version_arg "$version"
  assert_known_crate "$from"
  cd "$ROOT_DIR"
  print_publish_plan "$from"

  local start
  start="$(crate_index "$from")"
  local i

  if [[ "$execute" != "yes" ]]; then
    publish_step "${CRATES[$start]}" "$version" "$execute"
    return 0
  fi

  for ((i = start; i < ${#CRATES[@]}; i++)); do
    publish_step "${CRATES[$i]}" "$version" "$execute"
  done
}

run_tag() {
  local version=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || die "--version requires a value"
        version="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown tag argument '$1'"
        ;;
    esac
  done

  require_version_arg "$version"
  cd "$ROOT_DIR"
  log "creating annotated tag v${version}"
  git tag -a "v${version}" -m "queryfabric ${version}"
}

main() {
  local command="${1:-}"
  [[ -n "$command" ]] || {
    usage
    exit 1
  }
  shift || true

  case "$command" in
    check)
      run_check "$@"
      ;;
    publish)
      run_publish "$@"
      ;;
    tag)
      run_tag "$@"
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      die "unknown command '$command'"
      ;;
  esac
}

main "$@"
