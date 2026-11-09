#!/bin/sh
# Run from a directory containing the exact FFmpeg and LAME release sources.
set -eu
prefix=$1
jobs=${CARGO_BUILD_JOBS:-4}
export GIT_CEILING_DIRECTORIES="$(pwd)"
case "$(uname -s)" in
  MINGW*|MSYS*)
    # Native MinGW/LLVM output, with no MSYS2 or compiler runtime DLL dependency.
    # MSYS2 itself can run under x64 emulation on ARM; select by toolchain.
    case "${MSYSTEM:-}" in
      CLANGARM64)
        export CC=clang AR=llvm-ar RANLIB=llvm-ranlib
        lame_host=aarch64-w64-mingw32
        set -- --target-os=mingw32 --arch=aarch64 --cc=clang --ar=llvm-ar --nm=llvm-nm --ranlib=llvm-ranlib --strip=llvm-strip ;;
      *) lame_host=x86_64-w64-mingw32; set -- --target-os=mingw32 --arch=x86_64 ;;
    esac
    extra_ldflags="-L$prefix/lib -static"
    ;;
  Darwin*)
    case "$(uname -m)" in
      arm64|aarch64) lame_host=aarch64-apple-darwin; set -- --arch=aarch64 ;;
      *) lame_host=x86_64-apple-darwin; set -- --arch=x86_64 ;;
    esac
    extra_ldflags="-L$prefix/lib" ;;
  *) echo 'Use packaging/linux/build-ffmpeg.sh for Linux.' >&2; exit 1 ;;
esac
cd lame-3.100
./configure --host="$lame_host" --prefix="$prefix" --disable-shared --enable-static --disable-frontend --with-pic
make -j"$jobs"
make install
cd ../FFmpeg-n8.0.1
PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure "$@" \
  --prefix="$prefix" --disable-shared --enable-static --disable-autodetect \
  --disable-x86asm --disable-debug --disable-doc --disable-ffplay \
  --enable-libmp3lame --enable-pic --pkg-config-flags=--static \
  --extra-cflags="-I$prefix/include" --extra-ldflags="$extra_ldflags"
make -j"$jobs"
make install
