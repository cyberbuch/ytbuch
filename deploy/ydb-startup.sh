#!/bin/bash
# Стартиране на Octo (rocto) и YottaDB GUI в контейнера ytbuch-yottadb.
#
# НЕ използваме ydb_env_set директно, защото той пуска mupip journal -recover,
# който при грешка (напр. прекъснат контейнер) дава fatal error и скриптът спира
# преди да стартира rocto. Това довежда до Internal Server Error в приложението.
#
# Вместо това: споделена настройка на средата (ydb-env.sh) + journal recovery с
# продължаване при грешка + supervisor цикъл, който рестартира rocto при срив.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR=/data/logs
GBL_DIR=/data/r2.06_x86_64/g
YDB_DIST=/opt/yottadb/current

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') $*" >>"$LOG_DIR/startup.log"
}

mkdir -p "$LOG_DIR"

# Споделена настройка на средата (без journal recovery)
. "$SCRIPT_DIR/ydb-env.sh"

log "=== YottaDB startup beginning ==="
log "ydb_dist=$ydb_dist ydb_gbldir=$ydb_gbldir"

# ---------------------------------------------------------------------------
# Journal recovery — продължаваме дори при грешка
# ---------------------------------------------------------------------------
log "Running journal recovery (non-fatal)..."
# Всичките три журнала: липсата на yottadb.mjl (DEFAULT) оставя базата
# с флаг „Must be recovered“ и mupip backup проваля с REQRECOV.
if "$YDB_DIST/mupip" journal -recover -backward "$GBL_DIR/yottadb.mjl,$GBL_DIR/%ydbaim.mjl,$GBL_DIR/%ydbocto.mjl" >>"$LOG_DIR/startup.log" 2>&1; then
    log "Journal recovery: OK"
else
    log "WARNING: Journal recovery failed (exit $?), continuing anyway..."
fi

# ---------------------------------------------------------------------------
# Създаване на Octo база при нужда
# ---------------------------------------------------------------------------
if [ ! -f "$GBL_DIR/%ydbocto.dat" ]; then
    log "Octo database file missing, creating..."
    "$YDB_DIST/mupip" create >>"$LOG_DIR/startup.log" 2>&1 || log "WARNING: mupip create failed"
fi

# ---------------------------------------------------------------------------
# Потребител ydb в Octo (парола ydbrocks, само за dev). Идемпотентно.
# ---------------------------------------------------------------------------
log "Checking Octo user 'ydb'..."
if ! "$YDB_DIST/yottadb" -run %ydboctoAdmin show users </dev/null 2>/dev/null | grep -qw ydb; then
    log "Creating Octo user 'ydb'..."
    printf "ydbrocks\nydbrocks\n" | "$YDB_DIST/yottadb" -run %ydboctoAdmin add user ydb -w -a >>"$LOG_DIR/octo-admin.log" 2>&1 \
        && log "Octo user 'ydb' created" \
        || log "WARNING: Failed to create Octo user (may already exist)"
else
    log "Octo user 'ydb' already exists"
fi

# ---------------------------------------------------------------------------
# Supervisor цикъл за rocto — рестартира при срив
# ---------------------------------------------------------------------------
start_rocto() {
    log "Starting rocto..."
    rocto -aw >>"$LOG_DIR/rocto.log" 2>&1 &
    ROCTO_PID=$!
    log "rocto started with PID $ROCTO_PID"

    # Изчакване rocto да отвори порт 1337
    local i
    for i in $(seq 1 30); do
        if bash -c 'echo >/dev/tcp/127.0.0.1/1337' 2>/dev/null; then
            log "rocto is ready on port 1337 (after ${i}s)"
            return 0
        fi
        # Ако процесът умре — прекъсваме цикъла
        if ! kill -0 "$ROCTO_PID" 2>/dev/null; then
            log "rocto process died during startup"
            return 1
        fi
        sleep 1
    done
    log "WARNING: rocto did not open port 1337 within 30s"
    return 1
}

# Стартираме ydbgui веднъж (не е критичен)
log "Starting ydbgui..."
"$YDB_DIST/yottadb" -run %ydbgui --readwrite >>"$LOG_DIR/%ydbgui.log" 2>&1 &
GUI_PID=$!
log "ydbgui started with PID $GUI_PID"

# Supervisor цикъл: стартираме rocto и го следим
while :; do
    start_rocto
    if [ $? -eq 0 ]; then
        # Следим дали rocto е жив
        wait "$ROCTO_PID" 2>/dev/null
        EXIT_CODE=$?
        log "rocto exited with code $EXIT_CODE, restarting in 3s..."
    else
        log "rocto failed to start, retrying in 5s..."
        sleep 2
    fi
    sleep 3
done
