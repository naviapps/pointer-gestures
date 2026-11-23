"""Identify local Git or controller-frozen source without a VCS-specific frontend."""

import re
import subprocess

from sdk_tooling import vm_environment as vm


def source_identity(root):
    """Use the controller's frozen source identity when Git metadata is omitted."""
    identity = vm.source_identity(require_fingerprint=False)
    if identity is None:
        identity = (
            subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            None,
        )
    commit, fingerprint = identity
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Invalid source commit identity")
    return commit, fingerprint
