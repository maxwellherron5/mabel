# -------------------------
# General project utilities

alias l := list-unsorted
list-unsorted:
  just --list --unsorted

fmt:
  cargo +nightly fmt --all

fix:
  cargo fix --workspace --allow-staged

set-env:
  export $(grep -v '^#' .env | xargs)

