#!/bin/sh
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"

# If your Steam library is somewhere else, correct this path.
export HL_ROOT="$HOME/.steam/steam/steamapps/common/Half-Life"

# Debug or release bxt-rs:
export BXT_RS="$SCRIPT_DIR/target/i686-unknown-linux-gnu/debug/libbxt_rs.so"
# export BXT_RS="$SCRIPT_DIR/target/i686-unknown-linux-gnu/release/libbxt_rs.so"

# If you want regular Bunnymod XT, uncomment and correct the path:
# export BXT="/replace/this/with/path/to/libBunnymodXT.so"

# Check that the Half-Life folder exists.
if [ ! -d "$HL_ROOT" ]; then
    echo "Half-Life folder does not exist at $HL_ROOT"
    exit 1
fi

# Check that bxt-rs exists.
if [ ! -f "$BXT_RS" ]; then
    echo "bxt-rs does not exist at $BXT_RS"
    exit 1
fi

export LD_PRELOAD="$BXT_RS"

if [ "$BXT" ]; then
    # Check that Bunnymod XT exists.
    if [ ! -f "$BXT" ]; then
        echo "Bunnymod XT does not exist at $BXT"
        exit 1
    fi

    export LD_PRELOAD="$LD_PRELOAD:$BXT"
fi

# Run Half-Life.
export LD_LIBRARY_PATH="$HL_ROOT:$LD_LIBRARY_PATH"
export SteamEnv=1 # Fix the annoying locale error.

# Set bxt-rs debug variables.
# export BXT_RS_VULKAN_DEBUG=1
# export BXT_RS_PROFILE=1
# export BXT_RS_PROFILE_TRACY=1
# export BXT_RS_VERBOSE=1

# Allow running the ffmpeg binary from the HL folder.
# export PATH="$HL_ROOT:$PATH"

# Set to disable BXT and bxt-rs.
# export LD_PRELOAD=""

cd "$HL_ROOT" || exit 1
exec ~/.steam/bin/steam-runtime/run.sh ./hl_linux -steam "$@"