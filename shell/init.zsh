# zsh init for aish

ai() {
  command aish-run -- "$@"
}

if [[ "${AISH_ENABLE_SHIMS:-1}" == "1" ]]; then
  eval "$(command aish-run --print-shims-active 2>/dev/null)"
fi
