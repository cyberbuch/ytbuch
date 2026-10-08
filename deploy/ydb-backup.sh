#!/bin/bash
# Архив на базата по правилата на YottaDB: MUPIP BACKUP database, онлайн,
# с превключване на журналите след архива (без -nonewjnlfiles).
#   ydb-backup.sh        — един архив
#   ydb-backup.sh loop   — периодично, през YDB_BACKUP_EVERY секунди (6h)
. /opt/yottadb/current/ydb_env_set
log=/data/logs/ydb-backup.log
export ydb_baktmpdir=/backup/.tmp
mkdir -p "$ydb_baktmpdir" /backup

bak() {
    stamp=$(date +%Y%m%d_%H%M%S)
    dir="/backup/ydb_backup_${stamp}"
    mkdir -p "$dir"
    echo "$(date) mupip backup -database '*' -> $dir" >>"$log"
    if "$ydb_dist/mupip" backup -database "*" "$dir/" >>"$log" 2>&1; then
        if tar -czf "${dir}.tar.gz" -C "$dir" . >>"$log" 2>&1; then
            rm -rf "$dir"
            echo "$(date) готов архив ${dir}.tar.gz" >>"$log"
        else
            echo "$(date) ГРЕШКА: tar на $dir се провали" >>"$log"
        fi
    else
        echo "$(date) ГРЕШКА: mupip backup се провали" >>"$log"
        rm -rf "$dir"
    fi
    ls -1t /backup/ydb_backup_*.tar.gz 2>/dev/null | tail -n +9 | xargs -r rm -f
}

case "${1:-}" in
    loop)
        # Първият архив закъснява, за да не се състезава с инициализацията
        # на базата при стартиране на контейнера.
        sleep "${YDB_BACKUP_DELAY:-120}"
        while :; do
            bak
            sleep "${YDB_BACKUP_EVERY:-21600}"
        done
        ;;
    *)
        bak
        ;;
esac
