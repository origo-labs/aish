# shellcheck shell=bash

ai() {
  command aish-run -- "$@"
}

if [ "${AISH_ENABLE_SHIMS:-1}" = "1" ]; then
  eval "$(command aish-run --print-shims 2>/dev/null)"
fi
