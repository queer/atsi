#!/usr/bin/env bash

cargo make release
cp -v target/release/@ ~/bin
