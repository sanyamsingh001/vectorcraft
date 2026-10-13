"""Remote-only staged-source gate. This proposal has not been executed."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

UNITS = ("width-shift", "artboard-delete", "screens-tiff", "style-auto-leading")
EXPECTED_ENV = {
    "RUSTUP_TOOLCHAIN": "1.99.0", "CARGO_BUILD_JOBS": "1",
    "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
    "CARGO_INCREMENTAL": "0", "CARGO_TERM_COLOR": "never",
}
FORBIDDEN = (
    "RUST_MIN_STACK", "RUST_TEST_THREADS", "PROPTEST_CASES", "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS", "CRAFT_FONTS_DIR", "CRAFT_FONTS_REQUIRED",
    "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTDOCFLAGS",
    "CARGO_BUILD_RUSTFLAGS", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
)
PACKAGES = (
    "libxkbcommon-dev", "libwayland-dev", "libx11-dev", "libxrandr-dev",
    "libxi-dev", "libgl1-mesa-dev", "fonts-dejavu-core",
)

def require(ok, message):
    if not ok:
        raise RuntimeError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")

def command(args, cwd):
    return subprocess.check_output(args, cwd=cwd, stderr=subprocess.STDOUT)

def git(source, *args):
    return command(["git", "--no-optional-locks", "-c", "core.quotePath=true", *args], source)

def qualify(log, required, steps):
    markers, passing, observed = [], [], {name: [] for name in required}
    summary = "CI summary: all 7 steps passed (" + ", ".join(steps) + ")"
    summaries, current = 0, None
    for line in log.splitlines():
        marker = re.fullmatch(r"=== ci: ([a-z]+) ===", line)
        if marker:
            current = marker.group(1)
            markers.append(current)
        summaries += line == summary
        if current == "test":
            test = re.fullmatch(r"test (\S+) \.\.\. (.+)", line)
            if test and test.group(1) in observed:
                observed[test.group(1)].append(test.group(2))
    passing = [name for name, outcomes in observed.items() if outcomes == ["ok"]]
    return {
        "stage_markers": markers, "all_seven_in_order": markers == steps,
        "all_seven_summary_count": summaries,
        "required_outcomes": observed, "required_pass_count": len(passing),
        "required_exactly_once_ok": len(passing) == len(required),
        "qualified": markers == steps and summaries == 1 and len(passing) == len(required),
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--unit", choices=UNITS, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source, out = args.source.resolve(), args.output.resolve()
    bundle = Path(__file__).resolve().parent
    workspace, temporary = Path(os.environ["GITHUB_WORKSPACE"]).resolve(), Path(os.environ["RUNNER_TEMP"]).resolve()
    target = Path(os.environ["CARGO_TARGET_DIR"]).resolve()
    scratch = temporary / ("r61-" + args.unit + "-tmp")
    require(source == workspace / "source", "unexpected source checkout")
    require(bundle == workspace / "carrier/.github/r61-staged-ci", "unexpected carrier location")
    require(out == temporary / ("r61-" + args.unit + "-evidence"), "unexpected evidence location")
    require(target == temporary / ("r61-" + args.unit + "-target"), "unexpected target location")
    require(not out.exists() and not target.exists() and not scratch.exists(), "evidence/target/tmp must be fresh, without cache")
    require(all(source not in p.parents for p in (out, target, scratch)), "output inside source")
    require(all(os.environ.get(k) == str(scratch) for k in ("TMPDIR", "TMP", "TEMP")), "temporary files must be outside source")
    out.mkdir()
    scratch.mkdir()
    result = {"unit": args.unit, "status": "FAIL", "started": time.time(), "ci_executed": False}
    exit_code = 1
    try:
        require(all(os.environ.get(k) == v for k, v in EXPECTED_ENV.items()), "runner environment differs")
        require(not any(os.environ.get(k) for k in FORBIDDEN), "forbidden build/test/source override")
        require(not any(os.environ.get(k) for k in os.environ if k.startswith(("RUST_TEST_", "PROPTEST_", "NEXTEST_"))), "test override")
        encoded = (bundle / "bindings.json").read_bytes()
        require(sha(encoded) == os.environ["BINDINGS_SHA256"], "bindings changed")
        require(sha(Path(__file__).read_bytes()) == os.environ["VERIFIER_SHA256"], "verifier changed")
        b = json.loads(encoded)
        require(set(b["units"]) == set(UNITS), "matrix differs")
        require(os.environ["GITHUB_REPOSITORY"] == b["repository"], "unexpected repository")
        require(os.environ["GITHUB_EVENT_NAME"] == "push" and os.environ["GITHUB_REF"] == "refs/heads/" + b["branch"], "unexpected carrier event/ref")
        carrier = workspace / "carrier"
        require(git(carrier, "rev-parse", "HEAD").decode().strip() == os.environ["GITHUB_SHA"], "carrier SHA differs")
        require(not git(carrier, "status", "--porcelain", "--untracked-files=all", "--ignored"), "carrier not clean")
        entry = b["units"][args.unit]
        manifest_path, patch_path = bundle / entry["manifest"], bundle / entry["patch"]
        manifest_bytes, patch_bytes = manifest_path.read_bytes(), patch_path.read_bytes()
        require(sha(manifest_bytes) == entry["manifest_sha256"], "frozen manifest changed")
        require(sha(patch_bytes) == entry["patch_sha256"], "frozen patch changed")
        m = json.loads(manifest_bytes)
        require(m["unit"] == args.unit and m["base"] == entry["base"] == b["base"], "base/unit mismatch")
        require(m["tree"] == entry["tree"] and m["diff_sha256"] == entry["patch_sha256"], "frozen identity mismatch")
        require(m["ci_args"] == b["ci_args"] == ["cargo", "xtask", "ci"], "CI command changed")
        require(len(m["required_tests"]) == len(set(m["required_tests"])) == entry["required_test_count"], "test inventory changed")
        require(set(m["parent_source_proofs"]) == set(m["files"]), "source proof inventory mismatch")
        result.update(base=b["base"], expected_tree=m["tree"], carrier_head=os.environ["GITHUB_SHA"],
                      carrier_ref=os.environ["GITHUB_REF"], run_id=os.environ["GITHUB_RUN_ID"],
                      run_attempt=os.environ["GITHUB_RUN_ATTEMPT"], manifest_sha256=sha(manifest_bytes),
                      original_patch_sha256=sha(patch_bytes), bindings_sha256=sha(encoded),
                      verifier_sha256=sha(Path(__file__).read_bytes()), command=b["ci_args"])
        write_json(out / "frozen-manifest.json", m)

        def guard(staged):
            require(git(source, "rev-parse", "HEAD").decode().strip() == b["base"], "source HEAD changed")
            symbolic = subprocess.run(
                ["git", "--no-optional-locks", "symbolic-ref", "--quiet", "HEAD"],
                cwd=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            require(symbolic.returncode == 1, f"source HEAD is not proven detached (symbolic-ref exit {symbolic.returncode})")
            require(not git(source, "ls-files", "--unmerged", "-z"), "unmerged index")
            require(not git(source, "diff", "--no-ext-diff", "--no-textconv", "--exit-code", "--"), "unstaged source")
            require(not git(source, "ls-files", "--others", "--exclude-standard", "-z"), "untracked extra inputs")
            require(not git(source, "ls-files", "--others", "--ignored", "--exclude-standard", "-z"), "ignored extra inputs")
            tree = git(source, "write-tree").decode().strip()
            require(tree == (m["tree"] if staged else b["base_tree"]), "staged tree mismatch")
            names = git(source, "diff", "--cached", "--no-renames", "--name-only", "-z", b["base"], "--").decode().split("\0")[:-1]
            require(sorted(names) == (sorted(m["files"]) if staged else []), "staged path inventory mismatch")
            patch = git(source, "diff", *b["diff_flags"], "--cached", b["base"], "--")
            require(sha(patch) == (entry["full_index_diff_sha256"] if staged else sha(b"")), "full-index patch mismatch")
            require(sha((source / "Cargo.lock").read_bytes()) == m["cargo_lock_sha256"], "lock hash mismatch")
            hashes = {}
            for name, proof in m["parent_source_proofs"].items():
                path = Path(name)
                require(not path.is_absolute() and ".." not in path.parts and not (source / path).is_symlink(), "unexpected source path")
                hashes[name] = sha((source / path).read_bytes())
                require(hashes[name] == proof["candidate_sha256" if staged else "base_sha256"], "source hash mismatch: " + name)
            require(manifest_path.read_bytes() == manifest_bytes and patch_path.read_bytes() == patch_bytes, "carrier input changed")
            require((bundle / "bindings.json").read_bytes() == encoded, "bindings changed during CI")
            require(sha(Path(__file__).read_bytes()) == os.environ["VERIFIER_SHA256"], "verifier changed during CI")
            snapshot = {"head": b["base"], "tree": tree, "files": names,
                        "detached_head": symbolic.returncode == 1, "symbolic_ref_exit_code": symbolic.returncode,
                        "full_index_diff_sha256": sha(patch), "original_patch_sha256": sha(patch_bytes),
                        "lock_sha256": m["cargo_lock_sha256"], "source_sha256": hashes,
                        "unstaged": [], "untracked": [], "ignored_extras": []}
            return snapshot, patch

        original, _ = guard(False)
        write_json(out / "base-source.json", original)
        git(source, "apply", "--check", "--index", str(patch_path))
        git(source, "apply", "--index", str(patch_path))
        before, diff = guard(True)
        write_json(out / "before-source.json", before)
        (out / "full-index.patch").write_bytes(diff)
        target.mkdir()
        rust = command(["rustc", "-vV"], source).decode()
        require(rust.startswith("rustc 1.99.0 "), "unexpected compiler")
        write_json(out / "environment.json", {
            "rust": rust, "cargo": command(["cargo", "-vV"], source).decode(),
            "git": command(["git", "--version"], source).decode(),
            "packages": command(["dpkg-query", "-W", *PACKAGES], source).decode(),
            "settings": {k: os.environ.get(k) for k in (*EXPECTED_ENV, *FORBIDDEN, "CARGO_TARGET_DIR", "TMPDIR", "TMP", "TEMP")},
        })
        result.update(status="RUNNING", ci_attempted=True, ci_started=time.time())
        write_json(out / "result.json", result)
        log_path = out / "ci.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen(b["ci_args"], cwd=source, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            result["ci_executed"] = True
            for line in process.stdout:
                log.write(line)
                log.flush()
                sys.stdout.buffer.write(line)
                sys.stdout.buffer.flush()
            result["ci_exit_code"] = process.wait()
        result["ci_ended"] = time.time()
        log = log_path.read_bytes()
        result["log_sha256"] = sha(log)
        after, _ = guard(True)
        write_json(out / "after-source.json", after)
        result["source_unchanged"] = before == after
        q = qualify(log.decode("utf-8", errors="replace"), m["required_tests"], b["steps"])
        result["qualification"] = q
        passed = result["ci_exit_code"] == 0 and result["source_unchanged"] and q["qualified"]
        result["status"] = "CI_PASS" if passed else "CI_FAIL"
        exit_code = 0 if passed else 1
    except Exception as error:
        result.update(status="FAIL", error=type(error).__name__ + ": " + str(error))
        if isinstance(error, subprocess.CalledProcessError):
            result["command_error_output"] = (error.output or b"").decode("utf-8", errors="replace")[-8000:]
    finally:
        result["ended"] = time.time()
        write_json(out / "result.json", result)
    return exit_code

if __name__ == "__main__":
    sys.exit(main())
