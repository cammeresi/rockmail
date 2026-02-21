#!/bin/sh
# Count tests by category from `cargo test -- --list` output.

list=$(cargo test --all-targets --all-features -- --list 2>&1)

unit=0
gold=0
regression=0
integration=0
section=""

while IFS= read -r line; do
    case "$line" in
        *Running*src/lib.rs*|*Running*src/bin/*)
            section=unit ;;
        *Running*regressions.rs*)
            section=regression ;;
        *Running*_gold.rs*|*Running*_proptest.rs*)
            section=gold ;;
        *Running*tests/*.rs*)
            section=integration ;;
        *": test")
            case "$section" in
                unit)        unit=$((unit + 1)) ;;
                gold)        gold=$((gold + 1)) ;;
                regression)  regression=$((regression + 1)) ;;
                integration) integration=$((integration + 1)) ;;
            esac ;;
    esac
done <<EOF
$list
EOF

total=$((unit + gold + regression + integration))

printf "unit:        %4d\n" "$unit"
printf "integration: %4d\n" "$integration"
printf "gold:        %4d\n" "$gold"
printf "regression:  %4d\n" "$regression"
printf "total:       %4d\n" "$total"
