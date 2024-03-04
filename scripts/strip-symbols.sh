#!/bin/sh
set -e

# Check if the user provided an argument
if [ $# -lt 1 ]; then
    echo "Usage: $0 <strip_binary_path> [strip_binary_args]"
    exit 1
fi

# Parse the arguments
strip_binary_path="$1"
shift
strip_binary_args="$@"

echo "strip_binary_path: $strip_binary_path"
echo "strip_binary_args: $strip_binary_args"

# Execute the provided command for each .node file found in the directory and its subdirectories
find "./packages" -type f -name "*.node" -print0 | xargs -0 sh -c "set -x; for arg do echo \"Processing file \$arg\"; $strip_binary_path \"$strip_binary_args \$arg\"; done" _
