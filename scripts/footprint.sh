#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
demo_ref=".#queryfabric-demo"
load_query='{"sql":"SELECT city, pm25 FROM readings JOIN stations ON readings.station_id = stations.station_id LIMIT 5"}'
pg_user="$(id -un)"
cold_runs=5
idle_runs=5
load_concurrency=8
load_requests_per_worker=20
tmp_root="$(mktemp -d)"
pgdata="$tmp_root/pgdata"
pgsocket="$tmp_root/pgsocket"
pg_started=0
mkdir -p "$pgsocket"

demo_pid=""
demo_port=""
pg_port=""

cleanup() {
  local status=$?
  if [[ -n "$demo_pid" ]] && kill -0 "$demo_pid" 2>/dev/null; then
    kill "$demo_pid" 2>/dev/null || true
    wait "$demo_pid" 2>/dev/null || true
  fi
  if [[ "$pg_started" -eq 1 ]]; then
    pg_ctl -D "$pgdata" -m fast stop >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp_root"
  exit "$status"
}
trap cleanup EXIT

port_is_free() {
  local port=$1
  if timeout 1 bash -lc ":</dev/tcp/127.0.0.1/$port" >/dev/null 2>&1; then
    return 1
  fi
  return 0
}

pick_port() {
  local port
  while :; do
    port="$(shuf -i 20000-40000 -n 1)"
    if port_is_free "$port"; then
      printf '%s\n' "$port"
      return 0
    fi
  done
}

now_ms() {
  local now
  now="$(date +%s%N)"
  printf '%s\n' "$((now / 1000000))"
}

median() {
  sort -n | awk '
    { values[NR] = $1 }
    END {
      if (NR == 0) exit 1
      if (NR % 2 == 1) {
        print values[(NR + 1) / 2]
      } else {
        print (values[NR / 2] + values[NR / 2 + 1]) / 2
      }
    }
  '
}

human_bytes() {
  numfmt --to=iec-i --suffix=B "$1"
}

fmt_kib() {
  awk -v kib="$1" 'BEGIN { printf "%s KiB", kib }'
}

fmt_ms() {
  awk -v ms="$1" 'BEGIN { printf "%s ms", ms }'
}

build_demo() {
  cd "$repo_root"
  nix build "$demo_ref" --no-link --print-out-paths
}

start_postgres() {
  initdb -D "$pgdata" --auth=trust >/dev/null
  local pgport
  pgport="$(pick_port)"
  pg_ctl -D "$pgdata" -o "-k $pgsocket -c listen_addresses='' -p $pgport" -w start >/dev/null
  pg_started=1
  pg_port="$pgport"
}

reset_database() {
  local pgport=$1
  dropdb --if-exists -h "$pgsocket" -p "$pgport" -U "$pg_user" qfbench >/dev/null 2>&1 || true
  createdb -h "$pgsocket" -p "$pgport" -U "$pg_user" qfbench >/dev/null
}

start_demo() {
  local binary=$1
  local db_url=$2
  local listen_port
  listen_port="$(pick_port)"
  env \
    QFDEMO_LISTEN_ADDR="127.0.0.1:$listen_port" \
    QFDEMO_DATABASE_URL="$db_url" \
    QFDEMO_STORE_BACKEND=memory \
    "$binary" >/dev/null 2>"$tmp_root/demo.log" &
  demo_pid=$!
  demo_port="$listen_port"
}

wait_for_healthz() {
  local port=$1
  local deadline_ms=$(( $(now_ms) + 60000 ))
  while :; do
    if curl --fail --silent --max-time 1 "http://127.0.0.1:$port/healthz" >/dev/null; then
      return 0
    fi
    if (( $(now_ms) > deadline_ms )); then
      echo "demo did not become healthy; last logs:" >&2
      sed -n '1,200p' "$tmp_root/demo.log" >&2
      return 1
    fi
    sleep 0.2
  done
}

vmrss_kib() {
  awk '/^VmRSS:/ { print $2; exit }' "/proc/$1/status"
}

measure_binary_size() {
  local binary=$1
  du -h "$binary" | awk '{ print $1 }'
}

sample_idle_rss() {
  local binary=$1
  local db_url=$2
  local pgport=$3
  local rss_samples=()
  reset_database "$pgport"
  start_demo "$binary" "$db_url"
  wait_for_healthz "$demo_port"
  curl --fail --silent --max-time 30 \
    -H 'content-type: application/json' \
    -d "$load_query" \
    "http://127.0.0.1:$demo_port/query" >/dev/null
  sleep 40
  for _ in $(seq 1 5); do
    rss_samples+=("$(vmrss_kib "$demo_pid")")
    sleep 1
  done
  printf '%s\n' "$(printf '%s\n' "${rss_samples[@]}" | median)"
  kill "$demo_pid" >/dev/null 2>&1 || true
  wait "$demo_pid" 2>/dev/null || true
  demo_pid=""
}

sample_cold_start_ms() {
  local binary=$1
  local db_url=$2
  local pgport=$3
  local start_ms
  local end_ms
  reset_database "$pgport"
  start_ms="$(now_ms)"
  start_demo "$binary" "$db_url"
  wait_for_healthz "$demo_port"
  end_ms="$(now_ms)"
  kill "$demo_pid" >/dev/null 2>&1 || true
  wait "$demo_pid" 2>/dev/null || true
  demo_pid=""
  printf '%s\n' "$((end_ms - start_ms))"
}

sample_load_peak_rss() {
  local binary=$1
  local db_url=$2
  local pgport=$3
  local peak=0
  local workers=()
  reset_database "$pgport"
  start_demo "$binary" "$db_url"
  wait_for_healthz "$demo_port"

  local _
  for _ in $(seq 1 "$load_concurrency"); do
    (
      local _
      for _ in $(seq 1 "$load_requests_per_worker"); do
        curl --fail --silent --max-time 30 \
          -H 'content-type: application/json' \
          -d "$load_query" \
          "http://127.0.0.1:$demo_port/query" >/dev/null
      done
    ) &
    workers+=("$!")
  done

  while :; do
    local live=0
    local worker_pid
    for worker_pid in "${workers[@]}"; do
      if kill -0 "$worker_pid" 2>/dev/null; then
        live=1
      fi
    done
    if (( live == 0 )); then
      break
    fi
    if kill -0 "$demo_pid" 2>/dev/null; then
      local rss
      rss="$(vmrss_kib "$demo_pid")"
      if [[ -n "$rss" ]] && (( rss > peak )); then
        peak=$rss
      fi
    fi
    sleep 0.05
  done

  for worker_pid in "${workers[@]}"; do
    wait "$worker_pid"
  done
  kill "$demo_pid" >/dev/null 2>&1 || true
  wait "$demo_pid" 2>/dev/null || true
  demo_pid=""
  printf '%s\n' "$peak"
}

demo_out="$(build_demo)"
demo_binary="$demo_out/bin/queryfabric-demo"
closure_size="$(nix path-info -S "$demo_out" | awk 'NR == 1 { print $2; exit }')"
binary_size="$(measure_binary_size "$demo_binary")"

start_postgres
db_url="postgresql://$pg_user@localhost/qfbench?host=$pgsocket&port=$pg_port"

cold_samples=()
idle_samples=()
load_samples=()

for _ in $(seq 1 "$cold_runs"); do
  cold_samples+=("$(sample_cold_start_ms "$demo_binary" "$db_url" "$pg_port")")
done

for _ in $(seq 1 "$idle_runs"); do
  idle_samples+=("$(sample_idle_rss "$demo_binary" "$db_url" "$pg_port")")
done

load_samples+=("$(sample_load_peak_rss "$demo_binary" "$db_url" "$pg_port")")

pg_ctl -D "$pgdata" -m fast stop >/dev/null

printf '| Metric | Value | Notes |\n'
printf '| --- | ---: | --- |\n'
printf "| Release binary size | %s | \`du -h\` on the packaged release binary \`%s/bin/queryfabric-demo\` |\n" "$binary_size" "$demo_out"
printf "| Nix closure size | %s | \`nix path-info -S\` for \`%s\` |\n" "$(human_bytes "$closure_size")" "$demo_out"
printf "| Cold-start median (5 runs) | %s | spawn to first successful \`GET /healthz\` |\n" "$(fmt_ms "$(printf '%s\n' "${cold_samples[@]}" | median)")"
printf "| Idle RSS median (5 runs) | %s | VmRSS after warmup and a 40-second settle window |\n" "$(fmt_kib "$(printf '%s\n' "${idle_samples[@]}" | median)")"
printf "| Under-load peak RSS | %s | peak VmRSS during %s concurrent \`POST /query\` workers |\n" "$(fmt_kib "${load_samples[0]}")" "$load_concurrency"
