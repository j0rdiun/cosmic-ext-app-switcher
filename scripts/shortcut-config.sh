#!/usr/bin/env bash
# Safe edits to COSMIC's system_actions RON map, shared by enable.sh and disable.sh.
#
# A malformed system_actions doesn't fail loudly: cosmic-settings-daemon logs a parse
# error and falls back to the packaged system defaults, so every *other* override in the
# file (a custom terminal command, say) silently stops working while our shortcut never
# registers at all. Both helpers therefore rebuild the map from scratch and refuse to
# write anything that isn't brace-delimited.
#
# Degenerate inputs handled: file missing, file empty, and a single-line "{}" (the earlier
# `grep -v … | head -n -1` pipeline turned that one into a body with no opening brace).

# Emit every entry line of $1 except our own two keys and the enclosing braces.
_shortcut_entries() {
    [ -s "$1" ] || return 0
    awk '
        /^[[:space:]]*\{?[[:space:]]*\}?[[:space:]]*$/                { next }
        /^[[:space:]]*(WindowSwitcher|WindowSwitcherPrevious):/       { next }
        { print }
    ' "$1"
}

# _write_map <config> [extra-line...] — rebuild <config> as a valid RON map.
_write_map() {
    local config="$1"; shift
    local body tmp
    body=$(_shortcut_entries "$config")

    tmp=$(mktemp)
    {
        printf '{\n'
        if [ -n "$body" ]; then printf '%s\n' "$body"; fi
        if [ "$#" -gt 0 ]; then printf '%s\n' "$@"; fi
        printf '}\n'
    } > "$tmp"

    if ! head -n1 "$tmp" | grep -q '^{' || ! tail -n1 "$tmp" | grep -q '^}'; then
        rm -f "$tmp"
        echo "Error: refusing to write malformed shortcut config to $config" >&2
        return 1
    fi

    mkdir -p "$(dirname "$config")"
    mv "$tmp" "$config"
    chmod 600 "$config"
}

# shortcut_register <config> <binary>
shortcut_register() {
    _write_map "$1" \
        "    WindowSwitcher: \"$2\"," \
        "    WindowSwitcherPrevious: \"$2 --reverse\","
}

# shortcut_unregister <config>
shortcut_unregister() {
    _write_map "$1"
}
