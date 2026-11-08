#!/usr/bin/env bash
# Tests scripts/check-workflows.sh: it passes on this repository's workflows, and fails, naming the problem, on a
# changed, missing or unknown common block and on a checkout that does not set persist-credentials.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
check="$here/check-workflows.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0

# A copy of this repository's workflows, changed by the command given, then checked.
case_() {
    local want="$1" expected="$2" change="$3"
    local dir="$work/case"
    rm -rf "$dir"
    mkdir -p "$dir/.github"
    cp -R "$here/../.github/workflows" "$dir/.github/"
    (cd "$dir" && eval "$change")
    local out code=0
    out=$("$check" "$dir" 2>&1) || code=$?
    if [[ "$want" == pass && "$code" -ne 0 ]] || [[ "$want" == fail && "$code" -eq 0 ]]; then
        echo "FAIL: expected $want after: $change" >&2
        echo "$out" >&2
        failures=$((failures + 1))
    elif ! grep -qF -- "$expected" <<< "$out"; then
        echo "FAIL: expected \"$expected\" after: $change" >&2
        echo "$out" >&2
        failures=$((failures + 1))
    fi
}

ci=.github/workflows/ci.yml
case_ pass "follow the shared layout" ":"
case_ fail "common block 'lint' differs" \
    "sed -i.bak '/# common: lint/,/# end common: lint/s/typos-cli --version [0-9.]*/typos-cli --version 0.0.1/' $ci"
case_ fail "common block 'changes' must appear once" "sed -i.bak '/# end common: changes/d' $ci"
case_ fail "common block 'header' must appear once" "printf '# common: header\n# end common: header\n' >> $ci"
case_ fail "'# common: build' is not a common block" "printf '  # common: build\n  # end common: build\n' >> $ci"
case_ fail "the checkout does not set persist-credentials" "cat > .github/workflows/extra.yml <<'YAML'
jobs:
  one:
    steps:
      - name: Check out
        uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          fetch-depth: 2
      - name: Then
        run: 'true'
YAML"
case_ fail "extra.yml:4: the checkout does not set persist-credentials" "cat > .github/workflows/extra.yml <<'YAML'
jobs:
  one:
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
  two:
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          persist-credentials: false
YAML"
case_ pass "follow the shared layout" "cat > .github/workflows/extra.yml <<'YAML'
jobs:
  one:
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          # A bump pushes its branch with this token.
          persist-credentials: true
      - run: 'true'
YAML"

if [[ "$failures" -ne 0 ]]; then
    echo "$failures case(s) failed" >&2
    exit 1
fi
echo "check-workflows.sh passes its cases."
