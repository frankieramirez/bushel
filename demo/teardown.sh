#!/bin/sh
# Remove only what setup.sh created.
set -u
for c in web cache worker; do
  container stop "$c" 2>/dev/null
  container delete "$c" 2>/dev/null
done
