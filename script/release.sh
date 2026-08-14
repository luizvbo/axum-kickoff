#!/bin/sh
set -e

./axum-kickoff migrate
exec ./axum-kickoff server
