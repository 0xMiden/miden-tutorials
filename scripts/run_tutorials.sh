#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="$ROOT_DIR/web-client"
RUST_DIR="$ROOT_DIR/rust-client"
RUNS_DIR="$RUST_DIR/.tutorial-runs"

WEB_EXAMPLES=(
  createMintConsume
  multiSendWithDelegatedProver
  incrementCounterContract
  unauthenticatedNoteTransfer
  foreignProcedureInvocation
)

WEB_SKIPPED=(
  incrementCounterContract
  foreignProcedureInvocation
)

RUST_EXAMPLES=(
  counter_contract_deploy
  counter_contract_fpi
  counter_contract_increment
  create_mint_consume_send
  delegated_prover
  hash_preimage_note
  mapping_example
  network_notes_counter_contract
  note_creation_in_masm
  oracle_data_query
  unauthenticated_note_transfer
)

RUST_SKIPPED=(
  counter_contract_fpi
  counter_contract_increment
  oracle_data_query
)

usage() {
  cat <<'EOF'
Usage: yarn tutorials [--web[=name]] [--rust[=name]] [--list]

Defaults to running all web and rust tutorials.

Examples:
  yarn tutorials
  yarn tutorials --web
  yarn tutorials --rust
  yarn tutorials --web=createMintConsume
  yarn tutorials --rust=counter_contract_deploy
EOF
}

contains() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    if [[ "$item" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

add_web_names() {
  local value="$1"
  local part
  IFS=',' read -r -a parts <<< "$value"
  for part in "${parts[@]}"; do
    if [[ -n "$part" ]]; then
      web_names+=("$part")
    fi
  done
}

add_rust_names() {
  local value="$1"
  local part
  IFS=',' read -r -a parts <<< "$value"
  for part in "${parts[@]}"; do
    if [[ -n "$part" ]]; then
      rust_names+=("$part")
    fi
  done
}

run_web=0
run_rust=0
saw_selector=0
web_names=()
rust_names=()
failures=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --web)
      saw_selector=1
      run_web=1
      if [[ $# -gt 1 && "${2:-}" != --* ]]; then
        add_web_names "$2"
        shift
      fi
      ;;
    --web=*)
      saw_selector=1
      run_web=1
      add_web_names "${1#--web=}"
      ;;
    --rust)
      saw_selector=1
      run_rust=1
      if [[ $# -gt 1 && "${2:-}" != --* ]]; then
        add_rust_names "$2"
        shift
      fi
      ;;
    --rust=*)
      saw_selector=1
      run_rust=1
      add_rust_names "${1#--rust=}"
      ;;
    --list)
      echo "Web tutorials (default):"
      for name in "${WEB_EXAMPLES[@]}"; do
        if ! contains "$name" "${WEB_SKIPPED[@]}"; then
          printf "  %s\n" "$name"
        fi
      done
      if [[ ${#WEB_SKIPPED[@]} -gt 0 ]]; then
        echo ""
        echo "Web tutorials (skipped by default):"
        printf "  %s\n" "${WEB_SKIPPED[@]}"
      fi
      echo ""
      echo "Rust tutorials (default):"
      for name in "${RUST_EXAMPLES[@]}"; do
        if ! contains "$name" "${RUST_SKIPPED[@]}"; then
          printf "  %s\n" "$name"
        fi
      done
      if [[ ${#RUST_SKIPPED[@]} -gt 0 ]]; then
        echo ""
        echo "Rust tutorials (skipped by default):"
        printf "  %s\n" "${RUST_SKIPPED[@]}"
      fi
      exit 0
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

if [[ "$saw_selector" -eq 0 ]]; then
  run_web=1
  run_rust=1
fi

if [[ "$run_web" -eq 1 ]]; then
  if [[ ${#web_names[@]} -eq 0 ]]; then
    web_names=()
    for name in "${WEB_EXAMPLES[@]}"; do
      if ! contains "$name" "${WEB_SKIPPED[@]}"; then
        web_names+=("$name")
      fi
    done
  fi

  for name in "${web_names[@]}"; do
    if ! contains "$name" "${WEB_EXAMPLES[@]}"; then
      echo "Unknown web tutorial: $name" >&2
      echo "Available web tutorials: ${WEB_EXAMPLES[*]}" >&2
      exit 1
    fi
    if contains "$name" "${WEB_SKIPPED[@]}"; then
      echo "Note: $name is skipped by default but will run because it was explicitly requested."
    fi
  done

  web_pattern="$(IFS='|'; echo "${web_names[*]}")"
  echo "Running web tutorials: ${web_names[*]}"
  if ! yarn --cwd "$WEB_DIR" playwright test --grep "$web_pattern"; then
    failures+=("web")
  fi
fi

if [[ "$run_rust" -eq 1 ]]; then
  if [[ ${#rust_names[@]} -eq 0 ]]; then
    rust_names=()
    for name in "${RUST_EXAMPLES[@]}"; do
      if ! contains "$name" "${RUST_SKIPPED[@]}"; then
        rust_names+=("$name")
      fi
    done
  fi

  for name in "${rust_names[@]}"; do
    if ! contains "$name" "${RUST_EXAMPLES[@]}"; then
      echo "Unknown rust tutorial: $name" >&2
      echo "Available rust tutorials: ${RUST_EXAMPLES[*]}" >&2
      exit 1
    fi
    if contains "$name" "${RUST_SKIPPED[@]}"; then
      echo "Note: $name is skipped by default but will run because it was explicitly requested."
    fi
  done

  echo "Cleaning rust build artifacts before running tutorials..."
  cargo clean --manifest-path "$RUST_DIR/Cargo.toml"

  mkdir -p "$RUNS_DIR"
  if [[ -e "$RUNS_DIR/masm" && ! -L "$RUNS_DIR/masm" ]]; then
    echo "Expected $RUNS_DIR/masm to be a symlink." >&2
    exit 1
  fi
  if [[ ! -e "$RUNS_DIR/masm" ]]; then
    ln -s ../../masm "$RUNS_DIR/masm"
  fi

  rust_retries="${TUTORIAL_RETRIES:-3}"

  for name in "${rust_names[@]}"; do
    attempt=1
    while true; do
      run_stamp="$(date +%Y%m%d-%H%M%S)-$$-attempt${attempt}"
      run_dir="$RUNS_DIR/${name}-${run_stamp}"
      mkdir -p "$run_dir"
      echo "Running rust tutorial: $name (attempt $attempt/$rust_retries)"
      echo "Run directory: $run_dir"
      set +e
      (
        cd "$run_dir"
        RUST_BACKTRACE=1 cargo run --manifest-path "$RUST_DIR/Cargo.toml" --bin "$name"
      ) 2>&1 | tee "$run_dir/output.log"
      status=${PIPESTATUS[0]}
      set -e
      echo "Output log: $run_dir/output.log"

      if [[ "$status" -eq 0 ]]; then
        break
      fi

      if [[ "$attempt" -ge "$rust_retries" ]]; then
        failures+=("rust:${name}")
        break
      fi

      echo "Retrying rust tutorial: $name in 10s..."
      sleep 10
      attempt=$((attempt + 1))
    done
  done
fi

if [[ ${#failures[@]} -gt 0 ]]; then
  echo ""
  echo "Failures:"
  printf "  %s\n" "${failures[@]}"
  exit 1
fi
