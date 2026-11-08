#!/usr/bin/env bash
# Checks the workflows against the layout Vehicoule's four repositories share (Vehicoule, vehicoule-plugins,
# vehicoule-plugins-extra and nsh-m3e), so that a check reads the same in each:
#
# - Every block of `.github/workflows/ci.yml` between `# common: NAME` and `# end common: NAME` is the same in the four
#   repositories: its hash is the one listed below, and this script is the same file in the four. Changing a common
#   block changes its hash here, and the same change goes to the other three repositories.
# - Every checkout in every workflow says whether it keeps the token on disk (`persist-credentials`).
#
#   scripts/check-workflows.sh [DIR]    check the repository at DIR (default: this script's repository)
set -euo pipefail

root="${1:-$(dirname "$0")/..}"
ci="$root/.github/workflows/ci.yml"

# The common blocks, and the SHA-256 of each, its marker lines left out. A `case` rather than an associative array:
# macOS ships Bash 3.2.
names="changes header lint"
expected() {
    case "$1" in
        changes) echo 18a4e41e06bf95cea585b942bc5412dcc93ef0557e44dd3ab03b8b20515bf20d ;;
        header) echo 692b8d5c25f02cc0688690d4bfbe57f1a0ed1e1bb2aef94d3721b3db5db9d3e0 ;;
        lint) echo 186fa84b4d7e72fc43f483159f8cbce975c0af23db4ae1402dbcfc0276510346 ;;
    esac
}

hash() {
    if command -v sha256sum > /dev/null; then sha256sum; else shasum -a 256; fi | cut -d' ' -f1
}

status=0
fail() {
    echo "error: $*" >&2
    status=1
}

[[ -f "$ci" ]] || { echo "error: no $ci" >&2; exit 1; }

# The lines between a block's markers, which stand alone on their line, whatever their indentation.
block() {
    awk -v name="$1" '
        $0 ~ "^ *# end common: " name "$" { inside = 0 }
        inside { print }
        $0 ~ "^ *# common: " name "$" { inside = 1 }
    ' "$ci"
}

for name in $names; do
    begins=$(grep -cE "^ *# common: $name\$" "$ci" || true)
    ends=$(grep -cE "^ *# end common: $name\$" "$ci" || true)
    if [[ "$begins" != 1 || "$ends" != 1 ]]; then
        fail "$ci: common block '$name' must appear once, between '# common: $name' and '# end common: $name'"
        continue
    fi
    actual=$(block "$name" | hash)
    if [[ "$actual" != "$(expected "$name")" ]]; then
        fail "$ci: common block '$name' differs from the other repositories' (hash $actual, expected $(expected "$name"))"
    fi
done

# A marker naming a block this script does not list is a typo or a block the other repositories lack.
while IFS= read -r name; do
    [[ " $names " == *" $name "* ]] || fail "$ci: '# common: $name' is not a common block"
done < <(sed -nE 's/^ *# common: ([A-Za-z0-9_-]+)$/\1/p' "$ci")

# Each `actions/checkout` step sets `persist-credentials` before the next step or the end of its job.
for workflow in "$root"/.github/workflows/*.yml; do
    while IFS= read -r line; do
        fail "$workflow:$line: the checkout does not set persist-credentials"
    done < <(awk '
        function close_step() { if (open && !set) print start; open = 0; step = 0 }
        /^ *- / {
            close_step()
            step = 1
            indent = match($0, /-/)
        }
        step && !/^ *- / && /^ *[^ #]/ && match($0, /[^ ]/) <= indent { close_step() }
        step && /^ *(- )?uses: actions\/checkout@/ { open = 1; set = 0; start = NR }
        open && /^ *persist-credentials: / { set = 1 }
        END { close_step() }
    ' "$workflow")
done

if [[ "$status" -eq 0 ]]; then
    echo "The workflows follow the shared layout."
fi
exit "$status"
