#!/bin/bash
# Remove only the exact REPL envelope that SurrealDB CLI 3.2 adds around
# piped --json --hide-welcome output. Older CLIs already return bare JSON.
# Callers must still validate the returned JSON and CLI exit/stderr status.

normalize_surreal_sql_json() {
  [ "$#" -eq 3 ] || return 2
  local output="$1" namespace="$2" database="$3"
  [ -n "$namespace" ] && [ -n "$database" ] || return 2

  local prompt="$namespace/$database> "
  local trailer=$'\n\n'"$prompt"
  if [[ "$output" == "$prompt"* ]]; then
    # A leading prompt without the matching final prompt is malformed.
    [[ "$output" == *"$trailer" ]] || return 1
    output="${output#"$prompt"}"
    output="${output%"$trailer"}"
  elif [[ "$output" == *"$trailer" ]]; then
    # A trailing prompt without its leading mate is malformed.
    return 1
  fi

  printf '%s' "$output"
}
