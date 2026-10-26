#!/bin/bash
# Споделена настройка на средата за YottaDB скриптовете.
# Не пуска journal recovery — безопасно за source от много скриптове.
#
# Използване: . /usr/local/bin/ydb-env.sh

YDB_DIST=/opt/yottadb/current
GBL_DIR=/data/r2.06_x86_64/g
SRC_DIR=/data/r2.06_x86_64/o/utf8
OBJ_DIR=/data/r2.06_x86_64/r

export ydb_dist="$YDB_DIST"
export gtm_dist="$YDB_DIST"
export ydb_dir=/data
export ydb_rel=r2.06_x86_64
export ydb_gbldir="$GBL_DIR/yottadb.gld"
export ydb_repl_instance="$GBL_DIR/yottadb.repl"
export ydb_chset=UTF-8
export gtm_chset=UTF-8
export ydb_routines="$SRC_DIR*($OBJ_DIR /data/r) $YDB_DIST/plugin/o/utf8/_ydbaim.so $YDB_DIST/plugin/o/utf8/_ydbgui.so $YDB_DIST/plugin/o/utf8/_ydbmwebserver.so $YDB_DIST/plugin/o/utf8/_ydbocto.so $YDB_DIST/plugin/o/utf8/_ydbposix.so $YDB_DIST/utf8/libyottadbutil.so"
export gtmroutines="$ydb_routines"
export ydb_tmp=/tmp/yottadb/r2.06_x86_64
export gtm_tmp=/tmp/yottadb/r2.06_x86_64
export ydb_log=/tmp/yottadb/r2.06_x86_64
export gtm_log=/tmp/yottadb/r2.06_x86_64
export ydb_retention=42
export gtm_retention=42
export ydb_readline=1
export ydb_xc_libcurl="$YDB_DIST/plugin/libcurl.xc"
export ydb_xc_ydbposix="$YDB_DIST/plugin/ydbposix.xc"
export ydb_etrap='Write:(0=$STACK) "Error occurred: ",$ZStatus,!'
export gtm_etrap="$ydb_etrap"
export ydb_procstuckexec="$YDB_DIST/yottadb -run %YDBPROCSTUCKEXEC"
export gtm_procstuckexec="$ydb_procstuckexec"
export ydb_icu_version=76
export gtm_icu_version=76
export LD_LIBRARY_PATH="$YDB_DIST:$YDB_DIST/utf8${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$YDB_DIST:$YDB_DIST/plugin/bin:$PATH"

mkdir -p /tmp/yottadb/r2.06_x86_64 "$SRC_DIR" "$OBJ_DIR" /data/logs
