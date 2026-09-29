#!/bin/sh
set -e

./{{project-name}} migrate
exec ./{{project-name}} server
