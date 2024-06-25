#!/bin/sh

if [ ! -f /app/netchat.db ]; then
    touch /app/netchat.db
    echo "Created netchat.db"
else
    echo "netchat.db already exists"
fi

if [ ! -f /app/config.toml ]; then
  cp /app/config.toml.example /app/config.toml
  echo "Created config.toml from config.toml.example"
fi

exec netchat-server
