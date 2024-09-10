#!/bin/bash

export SCRIPT_ROOT=`pwd`
export BIN_OUTPUT=$SCRIPT_ROOT/deb/perfetto/opt/perfetto
export LIB_OUTPUT=$SCRIPT_ROOT/deb/perfetto/lib
mkdir -p $BIN_OUTPUT
mkdir -p $LIB_OUTPUT
git submodule update --init --recursive

cd $SCRIPT_ROOT/cpp/perfetto
sudo tools/install-build-deps
tools/gn gen --args='is_debug=false' out/linux
tools/ninja -C out/linux traced traced_probes perfetto
mv out/linux/traced $BIN_OUTPUT
mv out/linux/traced_probes $BIN_OUTPUT
mv out/linux/perfetto $BIN_OUTPUT
mv out/linux/libperfetto.so $LIB_OUTPUT

cd $SCRIPT_ROOT/deb
dpkg-deb --build perfetto
mv perfetto.deb ../
