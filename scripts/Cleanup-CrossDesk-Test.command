#!/bin/sh
set -eu
cd "$(dirname "$0")"
echo 'Quit CrossDesk Test first. CrossDesk production data will not be removed.'
"./CrossDesk Test.app/Contents/MacOS/crossdesk-test-daemon" --remove-test-data
echo 'Done. You can now move this extracted test folder to the Trash.'
printf 'Press Enter to close... '
read -r ignored
