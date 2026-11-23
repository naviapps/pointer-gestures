"""Report the test C compiler's ABI layouts for the managed checks."""

import subprocess

from sdk_tooling.adapters import load


def report_layout(ffi, work, *, compiler, env, windows=False):
    executable = load("c_compiler").compile_c(
        ffi / "tests/c/layout.c",
        output=work / ("layout.exe" if windows else "layout"),
        compiler=compiler,
        cwd=work,
        env=env,
        include_dirs=[ffi / "include"],
        flags=[("/D" if windows else "-D") + "POINTER_GESTURES_REPORT_LAYOUT"],
        standard="c11",
        style="msvc" if windows else "clang",
    )
    if windows:
        load("windows").require_pe_machine(executable, "x64")
    report = work / "layout.txt"
    report.write_bytes(subprocess.check_output([str(executable)], cwd=work, env=env))
    return report
