#!/bin/bash
cargo bundle --release
open target/release/bundle/dmg
