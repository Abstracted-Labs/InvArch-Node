#!/bin/bash

# Initialize result variables
pallets_result=""
invarch_result=""
tinkernet_result=""

# Function to build and test a given directory
build_and_test() {
  local dir=$1
  local result_var=$2

  cd ./$dir

  echo "$dir: 1/3 Checking format..."
  if RUSTFLAGS=-Awarnings cargo fmt --all -- --check > /dev/null 2>&1; then
    echo "$dir: 1/3 Format Check Ok"
  else
    echo "$dir: 1/3 Format Check Failed"
    echo "$dir: 1/3 Running cargo fmt to fix format..."
    if cargo fmt --all > /dev/null 2>&1; then
      echo "$dir: 1/3 Format Fixed"
    else
      echo "$dir: 1/3 Format Fix Failed"
      eval "$result_var=\"$dir: Format Failed\""
      cd ..
      return
    fi
  fi

  echo "$dir: 2/3 Building..."
  if RUSTFLAGS=-Awarnings cargo build --quiet > /dev/null; then
    echo "$dir: 2/3 Build Ok"
  else
    echo "$dir: 2/3 Build Failed"
    eval "$result_var=\"$dir: Build Failed\""
    cd ..
    return
  fi

  echo "$dir: 3/3 Testing..."
  if RUSTFLAGS=-Awarnings cargo test --quiet > /dev/null; then
    echo "$dir: 3/3 Test Ok"
    eval "$result_var=\"$dir: Ok\""
  else
    echo "$dir: 3/3 Test Failed"
    eval "$result_var=\"$dir: Test Failed\""
  fi

  cd ..
}

# Build and test each project
build_and_test "pallets" "pallets_result"
build_and_test "invarch" "invarch_result"
build_and_test "tinkernet" "tinkernet_result"

# Print results
echo -e "\nResults:"
echo $pallets_result
echo $invarch_result
echo $tinkernet_result