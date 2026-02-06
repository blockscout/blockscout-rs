#!/bin/sh

# The script helps to rename 'Decimal' to 'BigDecimal' in the SeaORM entities.
#
# SeaORM converts source SQL types like NUMERIC(78,0) to Decimal,
# but it isn't fitting into the Decimal Rust type.
# Such fields should be renamed to BigDecimal by hand in the generated entities.
# This script helps do this automatically.


# Show usage information
show_help() {
  echo "Usage: $0 [directory]"
  echo
  echo "Scans all .rs files in the specified directory (default: current directory),"
  echo "and replaces 'Decimal' with 'BigDecimal' on the line following:"
  echo "    #[sea_orm(column_type = \"Decimal(Some...\""
  echo
  echo "Options:"
  echo "  --help       Show this help message and exit"
}

# Check for --help flag
if [ "$1" = "--help" ]; then
  show_help
  exit 0
fi

# Use current directory if no argument provided
if [ $# -eq 0 ]; then
  DIR="."
elif [ $# -eq 1 ]; then
  DIR="$1"
else
  echo "Error: Too many arguments." >&2
  show_help
  exit 1
fi

# Verify that the directory exists
if [ ! -d "$DIR" ]; then
  echo "Error: '$DIR' is not a directory or does not exist." >&2
  exit 1
fi

# Search for all .rs files under the specified directory
find "$DIR" -type f -name '*.rs' | while IFS= read -r file; do
  #grep -n '#\[sea_orm(column_type = "Decimal(Some' "$file" | cut -d: -f1
  #continue;
  # Find lines with the target SeaORM attribute
  grep -n '#\[sea_orm(column_type = "Decimal(Some' "$file" | cut -d: -f1 | while IFS= read -r lineno; do
    target_line=$((lineno + 1))
    tmpfile="$(mktemp)"

    found_line=$(sed -n "${lineno}p" "$file")

    echo "File: $file, Line: $lineno" > /dev/stderr
    echo "  Pattern found: $found_line" > /dev/stderr

    # Use awk to replace 'Decimal' with 'BigDecimal' on the target line only
    awk -v target="$target_line" -v fname="$file" '
    NR == target {
      orig = $0
      changed = $0
      sub(/Decimal/, "BigDecimal", changed)
      if (orig != changed) {
        print "   Before: " orig > "/dev/stderr"
        print "   After:  " changed > "/dev/stderr"
        print "--------------------------------------------------------" > "/dev/stderr"
      }
      print changed
      next
    }
    { print }
    ' "$file" > "$tmpfile" && mv "$tmpfile" "$file"
  done
done