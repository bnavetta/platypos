#!/usr/bin/env bash

git ls-files --exclude-standard -oi --directory > .git/ignores.tmp
rsync -avzhP --exclude-from=.git/ignores.tmp . bennavetta.com:src/platypos
