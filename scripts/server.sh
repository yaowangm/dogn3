#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

pid_file=${DOGN3_PID_FILE:-"$repo_root/target/dogn3.pid"}
log_file=${DOGN3_LOG_FILE:-"$repo_root/target/dogn3.log"}

usage() {
  cat >&2 <<EOF
Usage:
  $0 start
  $0 stop
  $0 restart
  $0 status

Environment:
  DATABASE_URL              PostgreSQL connection URL. Default: postgres:///dogn
  BIND_ADDR                 Server bind address. Default: 127.0.0.1:3000
  DATABASE_MAX_CONNECTIONS  PostgreSQL pool size. Default: 5
  BOARD_PAGE_SIZE           Default root post trees per board page. Default: 50
  CACHE_ENABLED             Enable Redis cache layer. Default: true
  REDIS_URL                 Redis connection URL. Default: redis://127.0.0.1:6379
  REDIS_KEY_PREFIX          Redis key prefix. Default: dogn3
  REDIS_DEFAULT_TTL_SECONDS Redis default cache TTL. Default: 300
  SITE_NAME                 Site display name. Default: Dogn
  IMAGE_DIRECTORY           Local post image directory. Default: images
  SESSION_TTL_SECONDS       Session lifetime in seconds. Default: 43200
  SESSION_COOKIE_SECURE     Send session cookie only over HTTPS. Default: false
  RUST_LOG                  Rust tracing filter. Default: dogn3=debug,tower_http=debug
  DOGN3_PID_FILE            PID file path. Default: target/dogn3.pid
  DOGN3_LOG_FILE            Log file path. Default: target/dogn3.log
EOF
}

load_env() {
  if [[ -f .env ]]; then
    set -a
    # shellcheck disable=SC1091
    source .env
    set +a
  fi

  export DATABASE_URL=${DATABASE_URL:-postgres:///dogn}
  export BIND_ADDR=${BIND_ADDR:-127.0.0.1:3000}
  export DATABASE_MAX_CONNECTIONS=${DATABASE_MAX_CONNECTIONS:-5}
  export BOARD_PAGE_SIZE=${BOARD_PAGE_SIZE:-50}
  export CACHE_ENABLED=${CACHE_ENABLED:-true}
  export REDIS_URL=${REDIS_URL:-redis://127.0.0.1:6379}
  export REDIS_KEY_PREFIX=${REDIS_KEY_PREFIX:-dogn3}
  export REDIS_DEFAULT_TTL_SECONDS=${REDIS_DEFAULT_TTL_SECONDS:-300}
  export SITE_NAME=${SITE_NAME:-Dogn}
  export IMAGE_DIRECTORY=${IMAGE_DIRECTORY:-images}
  export SESSION_TTL_SECONDS=${SESSION_TTL_SECONDS:-43200}
  export SESSION_COOKIE_SECURE=${SESSION_COOKIE_SECURE:-false}
  export RUST_LOG=${RUST_LOG:-dogn3=debug,tower_http=debug}
}

is_running() {
  [[ -f "$pid_file" ]] || return 1

  local pid
  pid=$(cat "$pid_file")
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  kill -0 "$pid" >/dev/null 2>&1
}

bind_port() {
  printf '%s\n' "${BIND_ADDR##*:}"
}

listener_pid() {
  local port
  port=$(bind_port)

  if command -v lsof >/dev/null 2>&1; then
    lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -n 1
  elif command -v fuser >/dev/null 2>&1; then
    fuser "$port"/tcp 2>/dev/null | awk '{print $1}'
  fi
}

process_command() {
  local pid=$1
  ps -p "$pid" -o command= 2>/dev/null || true
}

is_dogn3_process() {
  local pid=$1
  local command
  command=$(process_command "$pid")
  [[ "$command" == *"/target/debug/dogn3"* || "$command" == *"target/debug/dogn3"* ]]
}

remember_running_listener() {
  local pid
  pid=$(listener_pid || true)
  [[ -n "$pid" ]] || return 1
  is_dogn3_process "$pid" || return 1
  echo "$pid" >"$pid_file"
}

start_server() {
  load_env
  mkdir -p "$(dirname "$pid_file")" "$(dirname "$log_file")"

  if ! is_running && [[ -f "$pid_file" ]]; then
    rm -f "$pid_file"
  fi

  if ! is_running && remember_running_listener; then
    echo "dogn3 is already running with PID $(cat "$pid_file")"
    echo "URL: http://${BIND_ADDR}"
    return 0
  fi

  if is_running; then
    echo "dogn3 is already running with PID $(cat "$pid_file")"
    echo "URL: http://${BIND_ADDR}"
    return 0
  fi

  local listener
  listener=$(listener_pid || true)
  if [[ -n "$listener" ]]; then
    echo "Port $(bind_port) is already in use by PID $listener:" >&2
    process_command "$listener" >&2
    echo "Stop that process or change BIND_ADDR before starting dogn3." >&2
    exit 1
  fi

  echo "Starting dogn3 at http://${BIND_ADDR}"
  echo "Log: $log_file"

  cargo build
  nohup "$repo_root/target/debug/dogn3" >"$log_file" 2>&1 &
  echo "$!" >"$pid_file"

  sleep 1

  if is_running; then
    echo "dogn3 started with PID $(cat "$pid_file")"
  else
    echo "dogn3 failed to start. Last log lines:" >&2
    tail -40 "$log_file" >&2 || true
    rm -f "$pid_file"
    exit 1
  fi
}

stop_server() {
  load_env

  if ! is_running && remember_running_listener; then
    :
  fi

  if ! is_running; then
    echo "dogn3 is not running"
    rm -f "$pid_file"
    return 0
  fi

  local pid
  pid=$(cat "$pid_file")
  echo "Stopping dogn3 with PID $pid"
  kill "$pid"

  for _ in {1..30}; do
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      rm -f "$pid_file"
      echo "dogn3 stopped"
      return 0
    fi
    sleep 1
  done

  echo "dogn3 did not stop after 30 seconds; sending SIGKILL"
  kill -9 "$pid" >/dev/null 2>&1 || true
  rm -f "$pid_file"
}

status_server() {
  load_env

  if ! is_running && remember_running_listener; then
    :
  fi

  if is_running; then
    echo "dogn3 is running"
    echo "PID: $(cat "$pid_file")"
    echo "URL: http://${BIND_ADDR}"
    echo "Log: $log_file"
  else
    echo "dogn3 is not running"
    if [[ -f "$pid_file" ]]; then
      echo "Stale PID file: $pid_file"
      rm -f "$pid_file"
    fi
  fi
}

command=${1:-}

case "$command" in
  start)
    start_server
    ;;
  stop)
    stop_server
    ;;
  restart)
    stop_server
    start_server
    ;;
  status)
    status_server
    ;;
  --help|-h|"")
    usage
    [[ -n "$command" ]] || exit 2
    ;;
  *)
    echo "Unknown command: $command" >&2
    usage
    exit 2
    ;;
esac
