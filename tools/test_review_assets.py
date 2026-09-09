"""Check archive identity before any build input is accepted."""
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

from review_assets import extract_verified


class ArchiveTests(unittest.TestCase):
    def archive(self, root, name="index.html", link=False):
        archive = root / "bundle.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            entry = tarfile.TarInfo(name)
            if link:
                entry.type = tarfile.SYMTYPE
                entry.linkname = "/etc/passwd"
                stream.addfile(entry)
            else:
                body = b"<!doctype html>"
                entry.size = len(body)
                stream.addfile(entry, io.BytesIO(body))
        return archive, hashlib.sha256(archive.read_bytes()).hexdigest()

    def test_verifies_bytes_before_creating_destination(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive, digest = self.archive(root)
            output = root / "output"
            with self.assertRaisesRegex(ValueError, "checksum"):
                extract_verified(archive, output, "0" * 64)
            self.assertFalse(output.exists())
            extract_verified(archive, output, digest)
            self.assertEqual((output / "index.html").read_bytes(), b"<!doctype html>")

    def test_refuses_escape_links_and_existing_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name, link in [("../escape", False), ("/escape", False),
                               ("C:/escape", False), ("index.html", True)]:
                archive, digest = self.archive(root, name, link)
                with self.assertRaises(ValueError):
                    extract_verified(archive, root / "output", digest)
                self.assertFalse((root / "output").exists())
            archive, digest = self.archive(root)
            output = root / "existing"
            output.mkdir()
            with self.assertRaises(FileExistsError):
                extract_verified(archive, output, digest)


if __name__ == "__main__":
    unittest.main()
