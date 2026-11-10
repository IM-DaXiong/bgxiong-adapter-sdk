"""Build a frozen SDK checkout and stage the source and product runtime together."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from export_sdk import ExportError, digest, json_bytes, load_manifest, no_links, require, safe_sdk_path, sha

SDK = Path(__file__).resolve().parents[1]
CLI = "bgx-adapter-v2"
WORKER = "bgx-adapter-v2-worker"
SDK_LAYOUT = "adapter-sdk-v2"
BROWSER_BUILD = "tools/form-reference"
DIST = f"{BROWSER_BUILD}/dist/"


def git(*args):
    return subprocess.check_output(["git", "-C", str(SDK), *args])


def measure(root, paths):
    rows = []
    seen = set()
    for path in sorted(paths):
        if path != ".gitattributes":
            safe_sdk_path(path)
        require(path.casefold() not in seen, "DUPLICATE_BUILD_INPUT: " + path)
        seen.add(path.casefold())
        file = no_links(root / path, True)
        rows.append({"path": path, "sha256": digest(file), "bytes": file.stat().st_size})
    return rows


def checkout():
    require(Path(git("rev-parse", "--show-toplevel").decode().strip()) == SDK,
            "EXPECTED_STANDALONE_SDK_CHECKOUT")
    paths = [os.fsdecode(path) for path in git("ls-files", "-z", "--cached", "--others",
                                             "--exclude-standard").split(b"\0") if path]
    return (git("rev-parse", "HEAD").decode().strip(),
            bool(git("status", "--porcelain=v1", "-z", "--untracked-files=all")),
            measure(SDK, paths))


def copy_inputs(rows, destination):
    for row in rows:
        target = destination / row["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(no_links(SDK / row["path"], True), target)
        require(digest(target) == row["sha256"], "BUILD_SOURCE_CHANGED: " + row["path"])


def run(args, cwd, env, *, capture=False):
    if capture:
        return subprocess.check_output(args, cwd=cwd, env=env, text=True)
    subprocess.run(args, cwd=cwd, env=env, check=True)


def build(profile, output):
    require(output.is_absolute() and ".." not in output.parts, "ABSOLUTE_OUTPUT_DIRECTORY_REQUIRED")
    parent = no_links(output.parent)
    output = parent / output.name
    require(not os.path.lexists(output), "BUILD_OUTPUT_ALREADY_EXISTS")

    identity = checkout()
    commit, dirty, inputs = identity
    env = os.environ.copy()
    env.pop("RUSTUP_TOOLCHAIN", None)

    with tempfile.TemporaryDirectory(prefix=".sdk-build-", dir=parent) as temp:
        stage = Path(temp)
        env["CARGO_TARGET_DIR"] = str(stage / "target")
        source = stage / "source"
        runtime = stage / "runtime"
        copy_inputs(inputs, source)

        npm = "npm.cmd" if os.name == "nt" else "npm"
        form = source / BROWSER_BUILD
        run([npm, "ci", "--engine-strict", "--ignore-scripts"], form, env)
        run([npm, "run", "build"], form, env)
        shutil.rmtree(form / "node_modules")

        manifest = load_manifest(source)
        for item in manifest["files"]:
            require(digest(no_links(source / item["source"], True)) == item["sha256"],
                    "BUILD_RELEASE_MANIFEST_MISMATCH: " + item["source"])
        generated = [item["source"] for item in manifest["files"] if item["source"].startswith(DIST)]
        actual = sorted(path.relative_to(source).as_posix() for path in (source / DIST).rglob("*")
                        if path.is_file())
        require(sorted(generated) == actual, "BUILD_GENERATED_INVENTORY_MISMATCH")
        source_rows = measure(source, [row["path"] for row in inputs] + generated)

        compiler = run(["rustc", "-vV"], source, env, capture=True).strip()
        platform = next(line.removeprefix("host: ") for line in compiler.splitlines()
                        if line.startswith("host: "))
        metadata = json.loads(run(["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
                                  source, env, capture=True))
        args = ["cargo", "build", "--locked",
                "--target", platform,
                "-p", "bgx-adapter-v2-tools", "--bin", CLI,
                "-p", "bgx-adapter-v2-runtime", "--bin", WORKER]
        if profile == "release":
            args.append("--release")
        run(args, source, env)

        require(measure(source, [row["path"] for row in source_rows]) == source_rows,
                "BUILD_STAGED_SOURCE_CHANGED")
        require(checkout() == identity, "BUILD_CHECKOUT_CHANGED")
        binaries = Path(metadata["target_directory"]) / platform / profile
        suffix = ".exe" if os.name == "nt" else ""
        cli = f"{CLI}{suffix}"
        worker = f"{WORKER}{suffix}"
        retained = f"{CLI}-retained{suffix}"
        runtime_inputs = {
            cli: binaries / cli,
            f"{SDK_LAYOUT}/{worker}": binaries / worker,
            f"{SDK_LAYOUT}/{retained}": binaries / cli,
            f"{SDK_LAYOUT}/protocol/schema-policy.json": source / "protocol/schema-policy.json",
            f"{SDK_LAYOUT}/protocol/host-feature-catalog.json": source / "protocol/host-feature-catalog.json",
        }
        for name, original in runtime_inputs.items():
            path = runtime / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original, path)
        versions = {p["name"]: p["version"] for p in metadata["packages"]}
        receipt = {
            "schemaVersion": 1, "kind": "SDK_BUILD_RECEIPT_V1",
            "sourceCommit": commit, "dirty": dirty,
            "checkoutDigest": sha(json_bytes(inputs)), "sourceDigest": sha(json_bytes(source_rows)),
            "profile": profile, "platform": platform, "rustc": compiler,
            "sdkRelease": manifest["sdkRelease"], "versions": versions,
            "sourceFiles": source_rows, "runtimeFiles": measure(runtime, runtime_inputs),
            "retainedBinarySource": cli, "validation": "BUILD_ONLY",
        }
        receipt_path = stage / "build-receipt.json"
        receipt_path.write_bytes(json_bytes(receipt))
        output.mkdir(exist_ok=False)
        try:
            source.rename(output / "source")
            runtime.rename(output / "runtime")
            receipt_path.rename(output / receipt_path.name)
        except BaseException:
            shutil.rmtree(no_links(output))
            raise
    print(json.dumps({"outputDirectory": str(output), "sourceCommit": commit,
                      "receiptSha256": digest(output / receipt_path.name)}))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=("debug", "release"), required=True)
    parser.add_argument("--output-directory", type=Path, required=True)
    args = parser.parse_args()
    try:
        build(args.profile, args.output_directory)
    except (ExportError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
