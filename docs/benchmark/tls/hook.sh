#!/bin/sh
echo "$(date +%s.%N) $*" >> /hook/events.log
