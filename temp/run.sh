#!/bin/bash
rm lupus
cargo build 
cp ../target/debug/lupus . && ./lupus $1
