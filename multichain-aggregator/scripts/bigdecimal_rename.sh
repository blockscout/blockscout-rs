#!/bin/bash

find . -type f -name "*.rs" | while read -r file; do
  grep -n '#\[sea_orm(column_type = "Decimal(Some' "$file" | cut -d: -f1 | while read -r lineno; do
    nextline=$((lineno + 1))
    # Print the original line for logging
    orig_line=$(sed -n "${nextline}p" "$file")
    if echo "$orig_line" | grep -q '\bDecimal\b'; then
      echo "File: $file, Line: $nextline"
      echo "Before: $orig_line"
      # Use Perl for in-place replacement with word boundary
      perl -i -pe "s/\\bDecimal\\b/BigDecimal/g if \$. == $nextline" "$file"
      # Print the new line for logging
      new_line=$(sed -n "${nextline}p" "$file")
      echo "After:  $new_line"
      echo "-----------------------------"
    fi
  done
done

