#!/bin/sh
set -e

# Check if the user provided an argument
if [ $# -lt 1 ]; then
    echo "Usage: $0 <output_directory>"
    exit 1
fi

output_directory="$1"

# Execute the provided command for each .node file found in the directory and its subdirectories
find "./packages" -type f -name "*.node" -print0 | xargs -0 sh -c "set -x; for arg do echo \"Processing file \$arg\"; artifact_basename=\"\$(basename -- \$arg)\"; cp \$arg $output_directory/\$artifact_basename; done" _
