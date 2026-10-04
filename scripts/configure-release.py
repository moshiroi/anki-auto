"""Configure hosted release builds; no local paths are retained in Rust diagnostics."""
import os
import pathlib
import sys

workspace = pathlib.Path.cwd().as_posix()
home = pathlib.Path.home().as_posix()
flags = [f"--remap-path-prefix={workspace}=/src", f"--remap-path-prefix={home}=/build"]
if sys.argv[1].endswith("windows-msvc"):
    # Windows rustc may see either spelling, depending on the tool invoking it.
    flags += [f"--remap-path-prefix={pathlib.Path.cwd()}=/src", f"--remap-path-prefix={pathlib.Path.home()}=/build"]
    flags += ["-C", "target-feature=+crt-static", "-C", "link-arg=/Brepro"]
# Encoded flags preserve paths containing spaces on Windows.
with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as env:
    env.write("CARGO_ENCODED_RUSTFLAGS=" + "\x1f".join(flags) + "\n")
    if not sys.argv[1].endswith("windows-msvc"):
        env.write(f"CFLAGS=-ffile-prefix-map={workspace}=/src -ffile-prefix-map={home}=/build\n")
