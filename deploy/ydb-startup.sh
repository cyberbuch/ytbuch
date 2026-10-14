#!/bin/bash
# Стартиране на Octo (rocto) и YottaDB GUI в контейнера ytbuch-yottadb.
# Замества stock /docker-debian-startup.sh на образа: той оставя $ydb_gbldir
# празен, когато няма интерактивен терминал, rocto не влиза в PATH и базата
# не тръгва — приложението дава Internal Server Error след рестарт на машината.
# Затова ydb_gbldir и PATH се задават изрично след ydb_env_set.
. /opt/yottadb/current/ydb_env_set
export ydb_gbldir=/data/r2.06_x86_64/g/yottadb.gld
export PATH=$PATH:/opt/yottadb/current/plugin/octo/bin
mkdir -p /data/logs

# Потребител ydb в Octo (парола ydbrocks, само за dev). Добавянето е идемпотентно.
if ! yottadb -run %ydboctoAdmin show users </dev/null 2>/dev/null | grep -qw ydb; then
    printf "ydbrocks\nydbrocks" | yottadb -run %ydboctoAdmin add user ydb -w -a >>/data/logs/octo-admin.log 2>&1
fi

rocto -aw >>/data/logs/rocto.log 2>&1 &
yottadb -run %ydbgui --readwrite >>/data/logs/%ydbgui.log 2>&1 &
