#!/bin/sh
# Build LGPL FFmpeg and LAME from the exact source archives shipped with releases.
set -eu
prefix=$1
jobs=${CARGO_BUILD_JOBS:-4}
# Do not let FFmpeg derive its version from the enclosing emul Git checkout.
export GIT_CEILING_DIRECTORIES="$(pwd)"
cd lame-3.100
./configure --prefix="$prefix" --disable-shared --enable-static --disable-frontend --with-pic
make -j"$jobs"
make install
cd ../FFmpeg-n8.0.1
PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure \
  --prefix="$prefix" --disable-shared --enable-static --disable-autodetect \
  --disable-x86asm --disable-debug --disable-doc --disable-ffplay \
  --enable-libmp3lame --enable-pic \
  --extra-cflags="-I$prefix/include" --extra-ldflags="-L$prefix/lib"
make -j"$jobs"
make install
