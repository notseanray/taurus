#!/bin/bash
rm lupus
cargo build && cp ../target/debug/taurus . && ./taurus $1
