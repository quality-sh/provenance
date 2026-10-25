"""Package verification must run in Actions on a maintainer host."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

loader = importlib.machinery.SourceFileLoader("agent_cargo", str(Path(__file__).parents[1] / "cargo"))
spec = importlib.util.spec_from_loader(loader.name, loader)
shim = importlib.util.module_from_spec(spec)
loader.exec_module(shim)


class PackageTest(unittest.TestCase):
    def test_package_uses_remote_command_without_native_cargo(self):
        args = ["package", "--workspace", "--locked", "--allow-dirty"]
        with patch.object(shim.sys, "argv", ["cargo", *args]), \
             patch.object(shim, "provenance_root", return_value=Path("/repo")), \
             patch.object(shim, "snapshot", return_value="a" * 40), \
             patch.object(shim, "run_id_for_snapshot", return_value=123) as dispatch, \
             patch.object(shim, "show_result", return_value=0) as result, \
             patch.object(shim, "real_cargo", side_effect=AssertionError("native Cargo was selected")):
            self.assertEqual(shim.main(), 0)
        self.assertEqual(dispatch.call_args.args[3], args)
        self.assertEqual(result.call_args.args[2:4], ("package", True))

    def test_bare_package_refuses_before_snapshot(self):
        with patch.object(shim.sys, "argv", ["cargo", "package"]), \
             patch.object(shim, "provenance_root", return_value=Path("/repo")), \
             patch.object(shim, "snapshot", side_effect=AssertionError("snapshot selected")):
            with self.assertRaisesRegex(RuntimeError, "cargo package --workspace --locked --allow-dirty"):
                shim.main()

    def test_dispatch_uses_the_snapshot_workflow(self):
        with patch.object(shim, "push_snapshot"), \
             patch.object(shim, "command", return_value='{"workflow_run_id": 123}') as command:
            shim.dispatch(Path("/repo"), "a" * 40, "quality-sh/provenance", '["package"]', "package")
        payload = json.loads(command.call_args.kwargs["input_text"])
        self.assertEqual(payload["ref"], "agent-snapshot/" + "a" * 40)


if __name__ == "__main__":
    unittest.main()
