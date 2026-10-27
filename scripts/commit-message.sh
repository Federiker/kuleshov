#!/usr/bin/env sh
# The commit-msg hook (lefthook.yml): the first line must start with a known [prefix] and some
# words, e.g. "[fix] Keep the stress on compounds". What each prefix means is in the README's
# "Git conventions". Git's own merge, revert and fixup messages are let through.
set -eu

if [ "$#" -ne 1 ]; then
  echo "Usage: sh scripts/commit-message.sh <commit message file>" >&2
  exit 2
fi

prefixes='add|fix|remove|refactor|rename|perf|data|docs|test|fmt|build|deps|security|release|revert|init'
first_line=$(grep -v '^#' "$1" | grep -m 1 . || true)

if printf '%s\n' "$first_line" | grep -Eq "^(\[($prefixes)\] [^ ]|Merge |Revert \"|fixup! |squash! |amend! )"; then
  exit 0
fi

echo 'The commit message should start with a prefix and a space, e.g. "[fix] Keep the stress on compounds".' >&2
echo "Prefixes: [$(printf '%s' "$prefixes" | sed 's/|/] [/g')]" >&2
echo "What each one means: README.md, \"Git conventions\". First line was: \"$first_line\"" >&2
exit 1
