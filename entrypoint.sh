#!/bin/sh
set -e

# Fix /app/data ownership in case the named volume was created as root
chown -R langtrans:langtrans /app/data

exec gosu langtrans "$@"
