#!/bin/sh
set -e

check_copyright_notices() {
    exitcode=0
    for file in $1; do
        if (head -n1 "$file" | grep -q "// Copyright .* Ulvetanna Inc."); then
            echo "$file: ERROR - Copyright notice is using Ulvetanna instead of Irreducible"
            exitcode=1
        elif ! (head -n1 "$file" | grep -q "// Copyright "); then
            echo "$file: ERROR - Copyright notice missing on first line"
            exitcode=1
        elif ! (head -n1 "$file" | grep -q "2025"); then
            echo "$file: ERROR - Copyright notice does not contain the year 2025"
            exitcode=1
        fi
    done
    exit $exitcode
}

check_copyright_notices "$(
    find perfetto-sys -type f \( -name '*.rs' -o -name '*.h' -o -name '*.hpp' -o -name '*.c' -o -name '*.cc' -o -name '*.cpp' \) -not -path 'perfetto-sys/cpp/perfetto/*';
    find src -type f -name '*.rs'
)"
