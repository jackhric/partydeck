#!/bin/bash
cd "$(dirname "$0")"
./partydeck --fullscreen > log.txt 2>&1
