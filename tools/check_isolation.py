"""Check the original inventory and bytes; a missing baseline is an error."""
import argparse
import json
from pathlib import Path
from capture_isolation import repository_root, verify


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--protected-root", required=True, type=Path)
    args = parser.parse_args()
    data = json.loads(args.baseline.read_text(encoding="utf-8"))
    errors = verify(repository_root(args.protected_root), data)
    print(json.dumps({"status": "FAIL" if errors else "PASS", "errors": errors,
                      "protectedCount": len(data["protectedFiles"])}))
    raise SystemExit(1 if errors else 0)


if __name__ == "__main__":
    main()
