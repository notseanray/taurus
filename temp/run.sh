#!/bin/bash
cargo build
cp ../target/debug/lupus . && ./lupus
