import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

from kg_decision import run, MAX_BYTES


class SubprocessTests(unittest.TestCase):
    def exercise(self, body, error=None, timeout=1, model=None):
        with tempfile.TemporaryDirectory(prefix="kg-runner-control-") as root:
            stub = Path(root) / "fake_agy"
            witness = Path(root) / "witness.json"
            stub.write_text(
                "#!" + sys.executable + "\nimport os,json,time\n"
                + "from pathlib import Path\n"
                + "Path(" + repr(str(witness)) + ").write_text(json.dumps({'pid':os.getpid(),'cwd':os.getcwd()}))\n"
                + body
            )
            stub.chmod(0o700)
            if error:
                with self.assertRaisesRegex(ValueError, error):
                    run("synthetic test only", str(stub), timeout, model)
            else:
                result = run("synthetic test only", str(stub), timeout, model)
                self.assertEqual(result["action"], "wander")
            state = json.loads(witness.read_text())
            self.assertFalse(Path(state["cwd"]).exists(), "disposable workspace survived")
            with self.assertRaises(ProcessLookupError):
                os.kill(state["pid"], 0)

    def test_nonzero_exit_is_rejected_and_reaped(self):
        self.exercise("raise SystemExit(7)\n", r"kind=child_exit exit=7 pid=\d+ stdout_bytes=0 stderr_bytes=0")

    def test_stdout_limit_is_rejected_and_reaped(self):
        self.exercise("print('x' * " + str(MAX_BYTES + 1) + ", flush=True)\n", "output too large")

    def test_timeout_reaps_term_ignoring_child_and_workspace(self):
        started = time.monotonic()
        self.exercise("import signal\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\ntime.sleep(60)\n", "timed out")
        self.assertLess(time.monotonic() - started, 10)

    def test_standalone_timeout_reaps_descendant(self):
        with tempfile.TemporaryDirectory(prefix="kg-runner-descendant-") as root:
            root = Path(root)
            witness = root / "descendant.pid"
            stub = root / "fake_agy"
            stub.write_text(
                "#!" + sys.executable + "\n"
                "import subprocess, sys, time\nfrom pathlib import Path\n"
                "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])\n"
                "Path(" + repr(str(witness)) + ").write_text(str(child.pid))\ntime.sleep(60)\n"
            )
            stub.chmod(0o700)
            with self.assertRaisesRegex(ValueError, "timed out"):
                run("synthetic test only", str(stub), 1)
            pid = int(witness.read_text())
            time.sleep(0.1)
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)

    def test_standalone_rejection_reaps_descendant(self):
        with tempfile.TemporaryDirectory(prefix="kg-runner-rejection-") as root:
            root = Path(root)
            witness = root / "descendant.pid"
            stub = root / "fake_agy"
            stub.write_text(
                "#!" + sys.executable + "\n"
                "import subprocess, sys\nfrom pathlib import Path\n"
                "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])\n"
                "Path(" + repr(str(witness)) + ").write_text(str(child.pid))\nprint('not json')\n"
            )
            stub.chmod(0o700)
            with self.assertRaises(ValueError):
                run("synthetic test only", str(stub), 1)
            pid = int(witness.read_text())
            time.sleep(0.1)
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)

    def test_valid_terminal_stream(self):
        items = [
            {"event":"init", "conversation_id":"control", "init":{"agent":"kg-decision"}},
            {"event":"result", "result":{"conversation_id":"control", "status":"SUCCESS",
             "structured_output":{"action":"wander", "parameters":{"mode":"random"}, "rationale":"test"}}}
        ]
        self.exercise("import sys\nassert sys.argv[sys.argv.index('--model')+1] == 'fixture-model'\nprint(" + repr("\n".join(json.dumps(x) for x in items)) + ")\n", model="fixture-model")

    def test_model_auto_is_omitted_and_explicit_model_is_trimmed(self):
        items = [
            {"event":"init", "conversation_id":"control", "init":{"agent":"kg-decision"}},
            {"event":"result", "result":{"conversation_id":"control", "status":"SUCCESS",
             "structured_output":{"action":"wander", "parameters":{"mode":"random"}, "rationale":"test"}}}
        ]
        with tempfile.TemporaryDirectory(prefix="kg-runner-model-") as root:
            root = Path(root)
            stub = root / "fake_agy"
            witness = root / "args.json"
            stub.write_text(
                "#!" + sys.executable + "\n"
                "import json, sys\nfrom pathlib import Path\n"
                "Path(" + repr(str(witness)) + ").write_text(json.dumps(sys.argv[1:]))\n"
                "print(" + repr("\n".join(json.dumps(x) for x in items)) + ")\n"
            )
            stub.chmod(0o700)
            run("synthetic test only", str(stub), 1, " AuTo ")
            args = json.loads(witness.read_text())
            self.assertNotIn("--model", args)
            run("synthetic test only", str(stub), 1, "  fixture-model  ")
            args = json.loads(witness.read_text())
            self.assertEqual(args[args.index("--model") + 1], "fixture-model")


if __name__ == "__main__":
    unittest.main()
