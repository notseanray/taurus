#!/bin/bash
mkdir taurus && cd taurus
git clone https://github.com/NotCreative21/taurus.git
mv taurus build
cd build
cargo build --release
cp target/release/taurus ../
./taurus help
