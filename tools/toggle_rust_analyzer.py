import json
from argparse import ArgumentParser, Namespace
from enum import StrEnum
from pathlib import Path
from typing import Any

# Rust analyzer doesn't respect multiple targets/toolchains
# So this script can change the .vscode settings for each project you are working on


class Project(StrEnum):
    COMMON = "ruuvi-common/Cargo.toml"
    GATEWAY = "ruuvi-gateway/Cargo.toml"
    LISTENER = "ruuvi-listener/Cargo.toml"


def parse_args() -> Namespace:
    p = ArgumentParser(description="VSCode Rust Analyzer Workspace Toggler")
    p.add_argument("workspace", type=lambda x: Project[x.upper()])
    return p.parse_args()


DEFAULT = {
    "rust-analyzer.cargo.allTargets": False,
    "rust-analyzer.server.extraEnv": {"RUSTUP_TOOLCHAIN": "stable"},
}


def build_vscode_settings(
    old_settings: dict[str, Any], project: Project
) -> dict[str, Any]:
    match project:
        case Project.COMMON | Project.GATEWAY:
            check_toolchain = cargo_toolchain = "stable"
            old_settings.pop("rust-analyzer.cargo.target", None)
        case Project.LISTENER:
            check_toolchain = "esp"
            cargo_toolchain = "esp"
            old_settings["rust-analyzer.cargo.target"] = "xtensa-esp32s3-none-elf"

    old_settings["rust-analyzer.linkedProjects"] = [str(project)]
    old_settings["rust-analyzer.check.extraEnv"] = {"RUSTUP_TOOLCHAIN": check_toolchain}
    old_settings["rust-analyzer.cargo.extraEnv"] = {"RUSTUP_TOOLCHAIN": cargo_toolchain}

    return {**old_settings, **DEFAULT}


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    settings_path = root / ".vscode" / "settings.json"
    data = json.loads(settings_path.read_text())
    new_settings = build_vscode_settings(data, args.workspace)
    settings_path.write_text(json.dumps(new_settings, indent=4))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
