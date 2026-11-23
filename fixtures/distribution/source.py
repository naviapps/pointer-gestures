"""Identify local Git or controller-frozen source without a VCS-specific frontend."""

import os
import re
import subprocess


def source_identity(root):
    """Use the controller's frozen source identity when Git metadata is omitted."""
    commit = (
        os.environ.get("NAVI_CI_SOURCE_COMMIT")
        or subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    )
    fingerprint = os.environ.get("NAVI_CI_SOURCE_FINGERPRINT")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Invalid source commit identity")
    if fingerprint is not None and not re.fullmatch(r"[0-9a-f]{64}", fingerprint):
        raise ValueError("Invalid frozen workspace fingerprint")
    return commit, fingerprint
