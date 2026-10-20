#!/bin/bash
# Read-only collection for an already running CrossDesk Test. macOS Bash 3.2.
# No permission requests, TCC database access, app launches, input, or resets.
set -uo pipefail
umask 077
export LC_ALL=en_US.UTF-8

if [[ "$(/usr/bin/uname -s)" != Darwin ]]; then
  printf '此工具只在 Mac 上运行。\n' >&2
  exit 1
fi

tool_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
report_dir="$(/usr/bin/mktemp -d "$tool_dir/CrossDesk-permission-report-XXXXXX")" || exit 1
report="$report_dir/权限检查结果.txt"

# A report may be shared with support. Strip user home paths; retain bundle
# paths, process IDs and signing identities, which are the evidence we need.
redact() {
  /usr/bin/sed -E 's@/Users/[^/[:space:]]+@/Users/<user>@g'
}
section() { printf '\n--- %s ---\n' "$1"; }
read_command() {
  "$@" 2>&1
  local result=$?
  printf '[command exit: %s]\n' "$result"
  return 0
}

collect() {
  printf 'CrossDesk Mac permission inspection v1\n'
  printf 'Read-only. No app started/stopped; no input captured/injected; no permissions changed.\n'
  printf 'Raw permission values must come from the running app, never from this Terminal process.\n'
  section 'System'
  read_command /usr/bin/sw_vers
  read_command /usr/bin/uname -m
  read_command /bin/date -u '+%Y-%m-%dT%H:%M:%SZ'

  section 'Running CrossDesk executables'
  local processes pid parent executable app bundle found=0
  processes="$(/bin/ps -ww -axo pid=,ppid=,comm=)"
  while read -r pid parent executable; do
    case "$executable" in
      */Contents/MacOS/crossdesk-desktop)
        app="${executable%/Contents/MacOS/crossdesk-desktop}"
        bundle="$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$app/Contents/Info.plist" 2>/dev/null)"
        [[ "$bundle" == dev.crossdesk.desktop.test ]] || continue
        found=$((found + 1))
        printf 'app_pid=%s parent_pid=%s\napp=%s\nbundle_id=%s\n' "$pid" "$parent" "$app" "$bundle"
        read_command /bin/ps -ww -p "$pid" -o pid=,ppid=,etime=,comm=
        case "$app" in */AppTranslocation/*) printf 'app_translocated=true\n';; *) printf 'app_translocated=false\n';; esac
        section 'App signature and designated requirement'
        read_command /usr/bin/codesign --display --verbose=4 --requirements :- "$app"
        section 'App signature verification'
        read_command /usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
        section 'Helper signature and designated requirement'
        read_command /usr/bin/codesign --display --verbose=4 --requirements :- "$app/Contents/MacOS/crossdesk-test-daemon"
        section 'Helper process in this app'
        printf '%s\n' "$processes" | /usr/bin/awk -v target="$app/Contents/MacOS/crossdesk-test-daemon" '
          { p=$1; parent=$2; sub(/^[[:space:]]*[0-9]+[[:space:]]+[0-9]+[[:space:]]+/, "");
            if ($0 == target) print "daemon_pid=" p " parent_pid=" parent " executable=" $0; }'
        if [[ -f "${app%/*}/VERSION.txt" ]]; then
          section 'Package label (label only, signature above is authoritative)'
          /usr/bin/head -n 5 "${app%/*}/VERSION.txt"
        fi
        ;;
    esac
  done <<< "$processes"
  printf '\nmatching_running_test_apps=%s\n' "$found"
  if [[ "$found" == 0 ]]; then
    printf 'No running test app was identified. Open the existing CrossDesk Test normally, then run this check again.\n'
  fi

  section 'CrossDesk TCC records from the last 30 minutes'
  # TCC decisions sometimes appear on a separate line from the app name.
  # Retain only CrossDesk lines and lines with the same TCC msgID. Do not save
  # the unfiltered system log. Bounded memory; no unrelated app logs in report.
  /usr/bin/log show --last 30m --style compact --info --predicate 'subsystem == "com.apple.TCC"' 2>/dev/null |
    /usr/bin/awk '
      {
        line=$0; key="";
        if (match(line, /msgID=[[:alnum:]_.-]+/)) key=substr(line,RSTART,RLENGTH);
        relevant=(tolower(line) ~ /crossdesk/);
        if (relevant && key != "") wanted[key]=1;
        slot=NR % 20000; lines[slot]=line; keys[slot]=key; direct[slot]=relevant; seq[slot]=NR;
      }
      END {
        start=(NR>20000 ? NR-19999 : 1); total=0;
        for(i=start;i<=NR;i++) {
          slot=i%20000;
          if(seq[slot]==i && (direct[slot] || (keys[slot]!="" && wanted[keys[slot]]))) {
            selected[total%500]=lines[slot]; total++;
          }
        }
        first=(total>500 ? total-500 : 0);
        for(i=first;i<total;i++) print selected[i%500];
        printf "matching_tcc_lines=%d; emitted=%d\n",total,total-first;
        if(total==0) print "No matching visible TCC records. This is NOT evidence that permission was granted or denied.";
      }'
  local log_status=${PIPESTATUS[0]}
  printf 'system_log_exit=%s\n' "$log_status"
  if [[ "$log_status" != 0 ]]; then
    printf 'System log was unavailable. No additional permissions were requested.\n'
  fi
}

printf '正在检查当前 CrossDesk Test 的程序身份和权限日志，请稍候。\n'
collect | redact > "$report"
result=$?
if [[ "$result" != 0 ]]; then
  printf '检查未完整完成（%s），已保留部分报告。\n' "$result" >&2
fi
printf '\n报告已保存：%s\n' "$report"
printf '请把此 txt 文件发回。它不会自动上传，也没有修改系统开关或配对。\n'
/usr/bin/open -R "$report" >/dev/null 2>&1 || true
if [[ -t 0 ]]; then
  printf '按回车关闭此窗口。'
  read -r ignored
fi
exit "$result"
