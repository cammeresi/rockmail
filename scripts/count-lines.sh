#!/bin/sh
# Count lines of Rust code: total, test, and non-test.
#
# "Test" means src/**/tests.rs plus everything under tests/.

t=0
n=0
for f in $(find src tests -name '*.rs'); do
    c=$(wc -l < "$f")
    case $f in
        */tests.rs|tests/*) t=$((t + c)) ;;
        *)                  n=$((n + c)) ;;
    esac
done

echo "total  $((t + n))"
echo "test   $t"
echo "other  $n"
