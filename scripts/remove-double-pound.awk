BEGIN {
    true = 1
    false = 0
    in_block = false
}

{
    if (!in_block && $0 ~ /^```/) {
        in_block = true
    } else if (in_block && $0 ~ /^```$/) {
        in_block = false
    }

    if (in_block) {
        sub(/## /, "# ")
    }
    print $0
}

